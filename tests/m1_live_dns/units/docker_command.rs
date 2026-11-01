use super::*;

#[test]
fn preparation_uses_remaining_original_time_without_the_old_sub_limit() {
    let origin = Instant::now();
    let end = origin + Duration::from_millis(5000);
    assert_eq!(
        remaining_at(end, origin + Duration::from_millis(1500)),
        Ok(Duration::from_millis(3500))
    );
    assert!(remaining_at(end, origin + Duration::from_millis(5000)).is_err());
    inject(move |args, deadline| {
        assert_eq!(args[0], "create");
        assert_eq!(deadline, end);
        assert!(remaining_at(deadline, origin + Duration::from_millis(1500)).is_ok());
        Ok("created".into())
    });
    assert_eq!(
        create("exact", "image", uuid::Uuid::from_u128(1), false, end),
        Ok("created".into())
    );
}

#[test]
fn expired_shared_deadline_cannot_submit_another_helper() {
    inject(|_, _| panic!("expired preparation or teardown must not spawn a helper"));
    assert!(metadata_until(&["create"], Instant::now() - Duration::from_millis(1)).is_err());
}

#[test]
fn completed_daemon_create_with_lost_client_reply_is_removed_but_not_refunded() {
    use std::{cell::Cell, rc::Rc};
    let present = Rc::new(Cell::new(false));
    let removed = Rc::new(Cell::new(false));
    let running = present.clone();
    let killed = removed.clone();
    inject(move |args, _| match args[0] {
        "ps" => Ok(if running.get() { "present" } else { "" }.into()),
        "create" => {
            running.set(true);
            Err(Fail::Unresolved("live_host_metadata_unavailable"))
        }
        "inspect" => Ok(format!(
            "{} sha256:expected live-dns-20261030 {}",
            "a".repeat(64),
            uuid::Uuid::from_u128(2)
        )),
        "rm" => {
            assert_eq!(args[2], "a".repeat(64));
            running.set(false);
            killed.set(true);
            Ok(String::new())
        }
        _ => unreachable!(),
    });
    let result = super::super::create(
        "sha256:expected",
        uuid::Uuid::from_u128(2),
        false,
        Instant::now() + Duration::from_millis(5000),
    );
    assert!(matches!(
        result,
        Err(Fail::Unresolved("live_disposal_unconfirmed"))
    ));
    assert!(removed.get());
    assert!(!present.get());
    let record = super::super::record_path(uuid::Uuid::from_u128(2), false);
    assert!(
        record.exists(),
        "a timed-out daemon request must retain its uncertainty even after scoped removal"
    );
    std::fs::remove_file(record).unwrap();
}

#[test]
fn expired_post_stop_budget_is_shared_by_both_workers() {
    inject(|_, _| panic!("teardown cannot refresh an exhausted shared deadline"));
    let end = Instant::now() - Duration::from_millis(1);
    for exporter in [false, true] {
        assert!(super::super::remove(uuid::Uuid::from_u128(3), exporter, end).is_err());
    }
}

#[test]
fn over_limit_output_is_rejected_before_a_full_pipe_stalls_exit() {
    let mut child = Command::new("/usr/bin/head")
        .args(["-c", "131072", "/dev/zero"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let result = collect(&mut child, Instant::now() + Duration::from_millis(100));
    assert!(child.try_wait().unwrap().is_some(), "helper must be reaped");
    assert_eq!(
        result,
        Err(Fail::Config("live_host_unqualified")),
        "a full output pipe must produce a bounded-output failure, not deadlock until the helper timeout"
    );
}

#[test]
fn stalled_output_times_out_and_reaps_the_helper() {
    let mut child = Command::new("/bin/sh")
        .args(["-c", "printf x; exec /bin/sleep 30"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    assert_eq!(
        collect(&mut child, Instant::now() + Duration::from_millis(100)),
        Err(Fail::Unresolved("live_host_metadata_unavailable"))
    );
    assert!(
        child.try_wait().unwrap().is_some(),
        "stalled helper must be reaped"
    );
}
