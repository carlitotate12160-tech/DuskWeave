use super::fixture::*;
use duskweave::m1_reasoning::{inference::*, proposal::RecordingPremises};
use std::sync::{Arc, atomic::AtomicBool};
use uuid::Uuid;

#[test]
fn accepted_v4_infer_records_successor_version_and_recording_integrity() {
    for (recording, disposition, retained) in [
        (Ok(RecordingPremises::Fresh), "validated_proposal", true),
        (
            Ok(RecordingPremises::ExpiredByTtl),
            "recorded_stale_premises",
            true,
        ),
        (
            Err("current_authority_unavailable"),
            "current_authority_unavailable",
            false,
        ),
    ] {
        // This port control injects post-release recording state; real TTL/SQL
        // qualification is required separately through the operational consumer.
        let released = Arc::new(AtomicBool::new(false));
        let mut basis = Basis {
            released: released.clone(),
            recording_checked: false,
            recording,
        };
        let mut sender = Sender(0, released);
        let mut store = Store::default();
        let scope = scope();
        let pack = pack();
        let receipt = infer(
            &mut Ledger,
            &mut sender,
            &mut basis,
            &mut store,
            &mut History,
            InferenceInput {
                scope: &scope,
                pack: &pack,
                event_id: Uuid::from_u128(7),
                occurred_at_ms: 1000,
                source_mode: "blind".into(),
            },
        );
        assert_eq!(
            sender.0, 1,
            "accepted generation reached provider port once"
        );
        let decision = store.0.as_ref().expect("published decision");
        assert_eq!(
            decision.version, 2,
            "v4 uses existing successor decision v2"
        );
        assert!(
            basis.recording_checked,
            "infer must check recording premises"
        );
        assert_eq!(decision.disposition, disposition);
        assert_eq!(decision.proposal.is_some(), retained);
        assert_eq!(receipt.proposal_disposition, disposition);
        assert_eq!(receipt.publication, "committed");
        assert_eq!(receipt.history, "completed");
    }
}
