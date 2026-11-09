use super::*;
use std::time::Duration;

#[test]
fn writable_mount_parsing_keeps_only_allowed_system_mounts() {
    let line = |path: &str, flags: &str| format!("30 25 8:1 / {path} {flags} - ext4 /dev/sda1 rw");
    assert_eq!(writable_path(&line("/proc", "rw")).unwrap(), Some("/proc"));
    assert_eq!(
        writable_path(&line("/dev/pts", "rw")).unwrap(),
        Some("/dev/pts")
    );
    assert_eq!(
        writable_path(&line("/proc/kcore", "rw")).unwrap(),
        Some("/proc/kcore")
    );
    assert_eq!(writable_path(&line("/sys", "ro")).unwrap(), None);
    assert_eq!(writable_path(&line("/proc", "ro")).unwrap(), None);
    for denied in ["/etc", "/opt/payload", "/var/lib", "/proc/sys"] {
        assert!(
            writable_path(&line(denied, "rw")).is_err(),
            "unknown writable mount refuses: {denied}"
        );
    }
    assert!(
        writable_path("30 25 8:1").is_err(),
        "short lines fail closed"
    );
    for allowed in [
        "/proc",
        "/dev",
        "/dev/pts",
        "/dev/mqueue",
        "/proc/interrupts",
        "/proc/kcore",
        "/proc/keys",
        "/proc/latency_stats",
        "/proc/timer_list",
    ] {
        assert!(allowed_system_mount(allowed), "{allowed}");
    }
    for denied in ["", "/proc/sys", "/proc1", "/dev/null", "/etc/duskweave"] {
        assert!(!allowed_system_mount(denied), "{denied}");
    }
}

#[test]
fn seal_mounts_fails_closed_when_mountinfo_unavailable() {
    assert_eq!(
        seal_mounts(999999, Instant::now() + Duration::from_secs(1)).unwrap_err(),
        Fail::Config("live_host_unqualified")
    );
}

#[test]
fn remount_commands_and_verification_share_the_original_deadline() {
    let end = Instant::now() + Duration::from_secs(1);
    let before = "1 0 0:1 / /proc rw - proc proc rw\n2 0 0:2 / /sys ro - sysfs sysfs ro\n";
    let after = before.replace("/proc rw", "/proc ro");
    let mut reads = [before.to_string(), after].into_iter();
    let mut commands = Vec::new();
    seal_with(
        42,
        end,
        || Ok(reads.next().unwrap()),
        |command, deadline| {
            assert_eq!(deadline, end);
            assert_eq!(command.get_program(), "/usr/bin/nsenter");
            commands.push(
                command
                    .get_args()
                    .map(|a| a.to_string_lossy().into_owned())
                    .collect::<Vec<_>>(),
            );
            Ok(String::new())
        },
    )
    .unwrap();
    assert_eq!(
        commands,
        [vec![
            "--target",
            "42",
            "--mount",
            "--root=/",
            "--wd=/",
            "--",
            "/usr/bin/mount",
            "--no-canonicalize",
            "-o",
            "remount,bind,ro",
            "/proc/42/root/proc"
        ]]
    );
    assert!(
        reads.next().is_none(),
        "the post-command snapshot was consumed"
    );
}

#[test]
fn sealing_refuses_incomplete_remounts_and_stops_on_first_io_failure() {
    let end = Instant::now() + Duration::from_secs(1);
    let rw = "1 0 0:1 / /dev rw - devtmpfs devtmpfs rw\n";
    let mut calls = 0;
    assert_eq!(
        seal_with(
            42,
            end,
            || Ok(rw.into()),
            |_, _| {
                calls += 1;
                Ok(String::new())
            }
        ),
        Err(Fail::Config("live_host_unqualified"))
    );
    assert_eq!(
        calls, 1,
        "successful command does not prove a read-only mount"
    );
    let mut reads = 0;
    assert_eq!(
        seal_with(
            42,
            end,
            || {
                reads += 1;
                Ok(rw.into())
            },
            |_, _| Err(Fail::Unresolved("live_host_metadata_unavailable"))
        ),
        Err(Fail::Unresolved("live_host_metadata_unavailable"))
    );
    assert_eq!(
        reads, 1,
        "no verification or later command follows a failed remount"
    );
    assert!(
        seal_with(
            42,
            end,
            || Ok("1 0 0:1 / /foreign rw - x x rw".into()),
            |_, _| panic!("unknown mount must refuse before command execution")
        )
        .is_err()
    );
}
