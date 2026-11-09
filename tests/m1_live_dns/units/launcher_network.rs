use super::*;
#[test]
#[ignore = "requires the packet-owned Linux host and namespace privileges"]
fn owned_lab_namespace_can_be_prepared_without_transmitting() {
    assert_eq!(rustix::process::geteuid().as_raw(), 0);
    let attempt = Uuid::parse_str(
        std::fs::read_to_string("/proc/sys/kernel/random/uuid")
            .unwrap()
            .trim(),
    )
    .unwrap();
    let image = super::super::docker::metadata(&[
        "image",
        "inspect",
        "--format",
        "{{.Id}}",
        "dw-live-dns-20261030:candidate",
    ])
    .unwrap();
    let preparation_start = Instant::now();
    let end = preparation_start + Duration::from_millis(5000);
    let container = super::super::docker::create(image.trim(), attempt, false, end).unwrap();
    let prepared = create(attempt, container.pid, "192.0.2.53", end);
    let elapsed = preparation_start.elapsed();
    let cleanup_end = Instant::now() + Duration::from_millis(1000);
    let cleaned = destroy(attempt, cleanup_end);
    super::super::docker::remove(attempt, false, cleanup_end).unwrap();
    eprintln!(
        "preparation_ms={} egress_granted=false",
        elapsed.as_millis()
    );
    assert!(
        prepared.is_ok(),
        "owned namespace preparation failed: {prepared:?}"
    );
    assert!(cleaned);
    assert!(!std::path::Path::new(&netns_path(attempt)).exists());
}

#[test]
fn nft_collector_refuses_output_beyond_the_cap() {
    use std::process::{Command, Stdio};

    let child = Command::new("/usr/bin/head")
        .args(["-c", "65537", "/dev/zero"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let result = finish_output(child, Instant::now() + Duration::from_secs(2));
    assert_eq!(result, Err(Fail::Config("live_host_unqualified")));
}

#[test]
fn nft_collector_requires_exit_eof_utf8_and_original_deadline() {
    use std::process::{Command, Stdio};

    let at_cap = Command::new("/usr/bin/head")
        .args(["-c", "65536", "/dev/zero"])
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    assert_eq!(
        finish_output(at_cap, Instant::now() + Duration::from_secs(2))
            .unwrap()
            .len(),
        65536
    );
    for script in ["printf '\\377'", "printf ok; exit 2"] {
        let child = Command::new("/bin/sh")
            .args(["-c", script])
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        assert_eq!(
            finish_output(child, Instant::now() + Duration::from_secs(1)),
            Err(Fail::Config("live_host_unqualified"))
        );
    }
    let child = Command::new("/bin/sh")
        .args(["-c", "exec /bin/sleep 30"])
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    assert_eq!(
        finish_output(child, Instant::now() + Duration::from_millis(100)),
        Err(Fail::Unresolved("live_host_control_unavailable"))
    );
}

#[test]
fn nft_collector_reap_after_elapsed_deadline_is_unresolved() {
    use std::process::{Command, Stdio};

    // Post-deadline confirmation: with stdout absent nft_stdout yields Config,
    // and reap_nft's confirmation against the already-elapsed deadline is
    // false, so finish_output must classify the outcome as Unresolved. The
    // harmless child is explicitly waited before transfer, so no child or
    // zombie survives this test.
    let end = Instant::now();
    let mut child = Command::new("/bin/sh")
        .args(["-c", "exit 0"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    child.wait().unwrap();
    assert_eq!(
        finish_output(child, end),
        Err(Fail::Unresolved("live_host_control_unavailable"))
    );
}

#[path = "network_fixture.rs"]
pub(crate) mod fixture;
thread_local! {
    static TEST_SLOT_ROOT: std::cell::RefCell<Option<std::path::PathBuf>> = const { std::cell::RefCell::new(None) };
}
pub(super) fn slot_root() -> String {
    TEST_SLOT_ROOT
        .with(|slot| {
            slot.borrow()
                .as_ref()
                .map(|p| p.to_string_lossy().into_owned())
        })
        .unwrap_or_else(|| ROOT.into())
}
pub(super) fn forwarding_path() -> Option<String> {
    TEST_SLOT_ROOT.with(|slot| {
        slot.borrow()
            .as_ref()
            .map(|p| p.join("ip_forward").to_string_lossy().into_owned())
    })
}

#[test]
fn owner_preparation_installs_default_deny_before_activating_links() {
    let io = fixture::Network::new();
    let attempt = Uuid::new_v4();
    let end = Instant::now() + Duration::from_secs(3);
    create(attempt, 42, "192.0.2.53", end).unwrap();
    let calls = io.events.borrow();
    assert_eq!(calls.len(), 11);
    assert_eq!(calls[0].program, IP);
    assert_eq!(
        calls[0].args,
        ["netns", "attach", &names(attempt).netns, "42"]
    );
    let inner = std::fs::read_to_string(&calls[4].input).unwrap();
    let outer = std::fs::read_to_string(&calls[5].input).unwrap();
    assert!(inner.contains("policy drop"));
    assert!(outer.contains("flags timeout; timeout 2000ms"));
    assert!(
        !outer.contains("add element"),
        "preparation cannot grant egress"
    );
    assert!(calls[8].args.ends_with(&["up".into()]));
    assert!(calls[9].args.ends_with(&["up".into()]));
    drop(calls);
    assert!(
        create(attempt, 42, "192.0.2.53", end).is_err(),
        "slot cannot be reused"
    );
    assert!(destroy(attempt, Instant::now() + Duration::from_secs(2)));
    assert!(!std::path::Path::new(&slot(&names(attempt))).exists());
}

#[test]
fn grants_require_the_same_slot_owner_and_never_extend_the_two_second_lease() {
    let io = fixture::Network::new();
    let attempt = Uuid::new_v4();
    let n = names(attempt);
    assert!(grant(attempt, i64::MAX).is_err());
    acquire_slot(&n, attempt).unwrap();
    let now = super::super::qualification::now_ms().unwrap();
    assert_eq!(
        grant(attempt, now),
        Err(Fail::State("live_effect_deadline"))
    );
    grant(attempt, now + 10000).unwrap();
    let rules = std::fs::read_to_string(&io.events.borrow()[0].input).unwrap();
    assert!(rules.contains("timeout 2000ms"));
    assert!(rules.contains(&n.table));
    revoke(attempt).unwrap();
    let revoked = std::fs::read_to_string(&io.events.borrow()[1].input).unwrap();
    assert_eq!(revoked, format!("flush set inet {} live\n", n.table));
    std::fs::write(slot(&n), Uuid::new_v4().to_string()).unwrap();
    assert!(!destroy(attempt, Instant::now() + Duration::from_secs(1)));
}

#[test]
fn namespace_failure_stops_configuration_and_retains_honest_cleanup() {
    let io = fixture::Network::new();
    io.fail_at.set(Some(1));
    let attempt = Uuid::new_v4();
    assert!(
        create(
            attempt,
            42,
            "192.0.2.53",
            Instant::now() + Duration::from_secs(2)
        )
        .is_err()
    );
    let calls = io.events.borrow();
    assert!(!calls.iter().any(|c| c.args.iter().any(|a| a == "up")));
    assert!(!calls.iter().any(|c| c.args.iter().any(|a| a == "route")));
    drop(calls);
    assert!(qualify_capture(0, "192.0.2.53").is_err());
    assert!(qualify_capture(42, "invalid").is_err());
    std::fs::write(io.root.join("ip_forward"), "0\n").unwrap();
    assert!(qualify_capture(42, "192.0.2.53").is_err());
}

#[test]
fn process_completion_and_rule_input_keep_one_absolute_budget() {
    for script in ["exit 0", "exit 7"] {
        let child = std::process::Command::new("/bin/sh")
            .args(["-c", script])
            .spawn()
            .unwrap();
        assert_eq!(
            finish(child, Instant::now() + Duration::from_secs(1)).is_ok(),
            script == "exit 0"
        );
    }
    let mut child = std::process::Command::new("/bin/cat")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    write_nft_rules(
        &mut child,
        "finite rules\n",
        Instant::now() + Duration::from_secs(1),
    )
    .unwrap();
    assert_eq!(
        finish_output(child, Instant::now() + Duration::from_secs(1)).unwrap(),
        "finite rules\n"
    );
    let expired = Instant::now();
    assert!(run("/bin/true", &[], expired).is_err());
    assert!(spawn_nft(None, "", false, expired).is_err());
}
