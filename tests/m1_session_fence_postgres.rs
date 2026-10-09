use duskweave::Fail;

#[path = "support/registration_db.rs"]
mod db_support;
use db_support::{accepted, db, ports, scope};

use duskweave::m1_session::SessionFence;
use duskweave::postgres_m1_session::PgSessionFence;

#[test]
fn session_fence_can_be_prepared_and_released() {
    let _guard = db();
    let (e, c) = scope(301);
    let (mut alloc, mut store, mut traj) = ports();
    accepted(&mut alloc, &mut store, &mut traj, e, c);

    let client = db_support::runtime_client();
    let mut fence = PgSessionFence::new(client);

    // Initial prepare
    let generation = fence.prepare_m1_session(e, c).unwrap();
    assert_eq!(generation, 1);

    // Cannot prepare again
    let err = fence.prepare_m1_session(e, c).unwrap_err();
    assert_eq!(err, Fail::State("already_prepared"));

    // Recover returns the same generation
    let recovered_gen = fence.recover_m1_session(e, c).unwrap();
    assert_eq!(recovered_gen, 1);

    // Release with wrong generation fails
    let err = fence.release_m1_session(e, c, 2).unwrap_err();
    assert_eq!(err, Fail::Conflict("generation_mismatch"));

    // Release success
    fence.release_m1_session(e, c, generation).unwrap();

    // Cannot recover released
    let err = fence.recover_m1_session(e, c).unwrap_err();
    assert_eq!(err, Fail::State("not_prepared"));

    // Can prepare again, generation increments
    let generation2 = fence.prepare_m1_session(e, c).unwrap();
    assert_eq!(generation2, 2);
}
