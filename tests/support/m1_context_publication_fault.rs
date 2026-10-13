use postgres::Client;
use serde_json::Value;
use uuid::Uuid;

pub struct PublicationFault {
    admin: Client,
    e: Uuid,
    c: Uuid,
    operation: Uuid,
    event: Uuid,
    goal: Uuid,
    original: Value,
    foreign: Option<(Uuid, Uuid, Value)>,
    finished: bool,
}

impl PublicationFault {
    pub fn capture(e: Uuid, c: Uuid) -> Self {
        let original = crate::context_support::scoped_state(e, c);
        Self {
            admin: crate::session_support::admin_db_client(),
            e,
            c,
            operation: serde_json::from_value(original["publication"]["operation_id"].clone())
                .unwrap(),
            event: serde_json::from_value(original["publication"]["event_id"].clone()).unwrap(),
            goal: serde_json::from_value(original["mission"]["goal_ref"].clone()).unwrap(),
            original,
            foreign: None,
            finished: false,
        }
    }

    pub fn protect_foreign(&mut self, e: Uuid, c: Uuid) {
        self.foreign = Some((e, c, crate::context_support::scoped_state(e, c)));
    }

    pub fn assert_foreign_unchanged(&self) {
        if let Some((e, c, before)) = &self.foreign {
            assert_eq!(*before, crate::context_support::scoped_state(*e, *c));
        }
    }

    pub fn contract(&self) -> Value {
        self.original["publication"]["contract"].clone()
    }

    pub fn set_contract(&mut self, contract: Value) {
        let n = self.admin.execute(
            "UPDATE mission.registration_outbox SET contract=$1 WHERE engagement_id=$2 AND campaign_id=$3 AND operation_id=$4",
            &[&contract, &self.e, &self.c, &self.operation],
        ).unwrap();
        assert_eq!(n, 1);
        let mut expected = self.original.clone();
        expected["publication"]["contract"] = contract;
        self.assert_corrupted(&expected);
    }

    pub fn set_catalog_event(&mut self, event: Uuid) {
        let n = self.admin.execute(
            "UPDATE mission.registration_outbox SET event_id=$1 WHERE engagement_id=$2 AND campaign_id=$3 AND operation_id=$4",
            &[&event, &self.e, &self.c, &self.operation],
        ).unwrap();
        assert_eq!(n, 1);
        let mut expected = self.original.clone();
        expected["publication"]["event_id"] = serde_json::to_value(event).unwrap();
        self.assert_corrupted(&expected);
    }

    pub fn set_owner_goal(&mut self, goal: Uuid) {
        let n = self.admin.execute(
            "UPDATE mission.missions SET goal_ref=$1 WHERE engagement_id=$2 AND campaign_id=$3 AND operation_id=$4",
            &[&goal, &self.e, &self.c, &self.operation],
        ).unwrap();
        assert_eq!(n, 1);
        let mut expected = self.original.clone();
        expected["mission"]["goal_ref"] = serde_json::to_value(goal).unwrap();
        self.assert_corrupted(&expected);
    }

    fn assert_corrupted(&self, expected: &Value) {
        assert_ne!(*expected, self.original);
        assert_eq!(
            *expected,
            crate::context_support::scoped_state(self.e, self.c)
        );
    }

    pub fn checked<T>(self, body: impl FnOnce(&mut Self) -> T) -> T {
        let mut fault = self;
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| body(&mut fault)));
        fault.finish().expect(
            "checked publication restoration must succeed before propagating an assertion panic",
        );
        match result {
            Ok(value) => value,
            Err(panic) => std::panic::resume_unwind(panic),
        }
    }

    fn restore(&mut self) -> Result<(), String> {
        let contract = &self.original["publication"]["contract"];
        let mut tx = self
            .admin
            .transaction()
            .map_err(|_| "restore_transaction")?;
        let publication = tx.execute(
            "UPDATE mission.registration_outbox SET contract=$1,event_id=$2 WHERE engagement_id=$3 AND campaign_id=$4 AND operation_id=$5",
            &[contract, &self.event, &self.e, &self.c, &self.operation],
        ).map_err(|_| "restore_publication")?;
        let owner = tx.execute(
            "UPDATE mission.missions SET goal_ref=$1 WHERE engagement_id=$2 AND campaign_id=$3 AND operation_id=$4",
            &[&self.goal, &self.e, &self.c, &self.operation],
        ).map_err(|_| "restore_owner")?;
        if (publication, owner) != (1, 1) {
            return Err("restore_row_count".into());
        }
        tx.commit().map_err(|_| "restore_commit".into())
    }

    pub fn finish(mut self) -> Result<(), String> {
        // Restore original contract
        self.restore()?;
        if crate::context_support::scoped_state(self.e, self.c) != self.original {
            return Err("restored_full_state_mismatch".into());
        }
        self.assert_foreign_unchanged();
        self.finished = true;
        Ok(())
    }
}

impl Drop for PublicationFault {
    fn drop(&mut self) {
        if !self.finished {
            let _ = self.restore();
        }
    }
}
