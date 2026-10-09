use crate::m1_session::SessionFence;
use crate::mission::{CampaignId, EngagementId};
use crate::{Fail, Res};
use postgres::{Client, IsolationLevel};

pub struct PgSessionFence {
    client: Client,
}

fn store_err(e: &postgres::Error) -> Fail {
    use postgres::error::SqlState;
    match e.code() {
        Some(&SqlState::T_R_SERIALIZATION_FAILURE) | Some(&SqlState::T_R_DEADLOCK_DETECTED) => {
            Fail::Store("serialization_retry")
        }
        Some(&SqlState::UNIQUE_VIOLATION) => Fail::Conflict("duplicate_identity"),
        _ => Fail::Store("storage_error"),
    }
}

impl PgSessionFence {
    pub fn new(client: Client) -> Self {
        Self { client }
    }

    fn lock_key(campaign: CampaignId) -> i64 {
        let bytes = campaign.0.as_bytes();
        i64::from_le_bytes(bytes[0..8].try_into().unwrap())
    }
}

impl SessionFence for PgSessionFence {
    fn prepare_m1_session(&mut self, engagement: EngagementId, campaign: CampaignId) -> Res<u64> {
        let mut tx = self
            .client
            .build_transaction()
            .isolation_level(IsolationLevel::Serializable)
            .start()
            .map_err(|e| store_err(&e))?;
        
        let key = Self::lock_key(campaign);
        let locked: bool = tx
            .query_one("SELECT pg_try_advisory_xact_lock($1)", &[&key])
            .map_err(|e| store_err(&e))?
            .get(0);
        
        if !locked {
            return Err(Fail::Conflict("lock_unavailable"));
        }

        let row = tx
            .query_opt(
                "SELECT generation, state FROM execution.m1_session WHERE campaign_id = $1",
                &[&campaign.0],
            )
            .map_err(|e| store_err(&e))?;

        let next_generation = if let Some(r) = row {
            let state: String = r.get("state");
            if state == "prepared" {
                return Err(Fail::State("already_prepared"));
            }
            let generation: i64 = r.get("generation");
            generation as u64 + 1
        } else {
            1
        };

        tx.execute(
            "INSERT INTO execution.m1_session (engagement_id, campaign_id, generation, state) \
             VALUES ($1, $2, $3, 'prepared') \
             ON CONFLICT (campaign_id) DO UPDATE \
             SET engagement_id = EXCLUDED.engagement_id, \
                 generation = EXCLUDED.generation, \
                 state = EXCLUDED.state, \
                 prepared_at = CURRENT_TIMESTAMP",
            &[&engagement.0, &campaign.0, &(next_generation as i64)],
        )
        .map_err(|e| store_err(&e))?;

        tx.commit().map_err(|_| Fail::Store("commit_unknown"))?;
        Ok(next_generation)
    }

    fn recover_m1_session(&mut self, engagement: EngagementId, campaign: CampaignId) -> Res<u64> {
        let mut tx = self
            .client
            .build_transaction()
            .isolation_level(IsolationLevel::Serializable)
            .start()
            .map_err(|e| store_err(&e))?;
            
        let key = Self::lock_key(campaign);
        let locked: bool = tx
            .query_one("SELECT pg_try_advisory_xact_lock($1)", &[&key])
            .map_err(|e| store_err(&e))?
            .get(0);
            
        if !locked {
            return Err(Fail::Conflict("lock_unavailable"));
        }

        let row = tx
            .query_opt(
                "SELECT generation, state, engagement_id FROM execution.m1_session WHERE campaign_id = $1",
                &[&campaign.0],
            )
            .map_err(|e| store_err(&e))?;

        if let Some(r) = row {
            let state: String = r.get("state");
            let db_eng: uuid::Uuid = r.get("engagement_id");
            if db_eng != engagement.0 {
                return Err(Fail::Input("engagement_mismatch"));
            }
            if state != "prepared" {
                return Err(Fail::State("not_prepared"));
            }
            let generation: i64 = r.get("generation");
            tx.commit().map_err(|_| Fail::Store("commit_unknown"))?;
            Ok(generation as u64)
        } else {
            Err(Fail::State("not_prepared"))
        }
    }

    fn release_m1_session(
        &mut self,
        engagement: EngagementId,
        campaign: CampaignId,
        generation: u64,
    ) -> Res<()> {
        let mut tx = self
            .client
            .build_transaction()
            .isolation_level(IsolationLevel::Serializable)
            .start()
            .map_err(|e| store_err(&e))?;
            
        let key = Self::lock_key(campaign);
        let locked: bool = tx
            .query_one("SELECT pg_try_advisory_xact_lock($1)", &[&key])
            .map_err(|e| store_err(&e))?
            .get(0);
            
        if !locked {
            return Err(Fail::Conflict("lock_unavailable"));
        }

        let row = tx
            .query_opt(
                "SELECT generation, state, engagement_id FROM execution.m1_session WHERE campaign_id = $1",
                &[&campaign.0],
            )
            .map_err(|e| store_err(&e))?;

        if let Some(r) = row {
            let state: String = r.get("state");
            let db_eng: uuid::Uuid = r.get("engagement_id");
            let current_gen: i64 = r.get("generation");

            if db_eng != engagement.0 {
                return Err(Fail::Input("engagement_mismatch"));
            }
            if current_gen as u64 != generation {
                return Err(Fail::Conflict("generation_mismatch"));
            }
            if state != "prepared" {
                return Err(Fail::State("not_prepared"));
            }

            tx.execute(
                "UPDATE execution.m1_session SET state = 'released' WHERE campaign_id = $1",
                &[&campaign.0],
            )
            .map_err(|e| store_err(&e))?;

            tx.commit().map_err(|_| Fail::Store("commit_unknown"))?;
            Ok(())
        } else {
            Err(Fail::State("not_prepared"))
        }
    }
}
