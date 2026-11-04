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
