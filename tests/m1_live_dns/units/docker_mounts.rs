use super::*;

#[test]
fn writable_mount_parsing_keeps_only_allowed_system_mounts() {
    let line = |path: &str, flags: &str| {
        format!("30 25 8:1 / {path} {flags} - ext4 /dev/sda1 rw")
    };
    assert_eq!(writable_path(&line("/proc", "rw")).unwrap(), Some("/proc"));
    assert_eq!(writable_path(&line("/dev/pts", "rw")).unwrap(), Some("/dev/pts"));
    assert_eq!(writable_path(&line("/proc/kcore", "rw")).unwrap(), Some("/proc/kcore"));
    assert_eq!(writable_path(&line("/sys", "ro")).unwrap(), None);
    assert_eq!(writable_path(&line("/proc", "ro")).unwrap(), None);
    for denied in ["/etc", "/opt/payload", "/var/lib", "/proc/sys"] {
        assert!(
            writable_path(&line(denied, "rw")).is_err(),
            "unknown writable mount refuses: {denied}"
        );
    }
    assert!(writable_path("30 25 8:1").is_err(), "short lines fail closed");
    for allowed in [
        "/proc", "/dev", "/dev/pts", "/dev/mqueue", "/proc/interrupts",
        "/proc/kcore", "/proc/keys", "/proc/latency_stats", "/proc/timer_list",
    ] {
        assert!(allowed_system_mount(allowed), "{allowed}");
    }
    for denied in ["", "/proc/sys", "/proc1", "/dev/null", "/etc/duskweave"] {
        assert!(!allowed_system_mount(denied), "{denied}");
    }
}
