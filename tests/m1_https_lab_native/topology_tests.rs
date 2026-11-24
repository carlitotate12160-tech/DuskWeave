#![cfg(test)]
//! Unprivileged security-contract and partial-lifecycle regression tests for `topology.rs`.

use super::topology::{
    CmdOutput, CmdRunner, HOST_VETH, IP_BIN, IPTABLES_BIN, LAB_NETNS, OwnedTopologyState,
    PEER_VETH, PreflightInputs, RESPONDER_GID, RESPONDER_UID, RuntimeDirs, SETPRIV_BIN,
    SHARED_CORE_CONTROL_ROOT, SYSCTL_BIN, acquire_network_with, build_responder_launch_args,
    check_preflight_collisions, dispose_network_with, parse_proc_status_summary,
    resolve_core_gid_from_passwd, setup_network_with, verify_proc_status,
};
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Default)]
struct RecordingRunner {
    calls: Vec<(String, Vec<String>)>,
    fail_on: Vec<(String, String)>,
}

impl RecordingRunner {
    fn fail_matching(mut self, bin: &str, needle: &str) -> Self {
        self.fail_on.push((bin.into(), needle.into()));
        self
    }
}

impl CmdRunner for RecordingRunner {
    fn run(&mut self, bin: &str, args: &[&str]) -> Result<CmdOutput, String> {
        let owned: Vec<String> = args.iter().map(|s| (*s).to_string()).collect();
        self.calls.push((bin.to_string(), owned.clone()));
        let joined = owned.join(" ");
        let fail = self
            .fail_on
            .iter()
            .any(|(b, n)| b == bin && joined.contains(n));
        Ok(CmdOutput {
            success: !fail,
            stdout: String::new(),
            stderr: if fail {
                format!("injected failure on {joined}")
            } else {
                String::new()
            },
        })
    }
}

#[test]
fn selected_topology_commands_match_https_lab_spec_without_default_route_or_flush() {
    let mut runner = RecordingRunner::default();
    let mut owned = OwnedTopologyState::default();
    let comment = "dw-https-lab-r1:test123";
    setup_network_with(&mut runner, comment, &mut owned).unwrap();
    assert!(owned.netns && owned.host_veth && owned.route_dns && owned.route_https);
    assert_eq!(owned.iptables_rules, vec![0, 1, 2, 3]);

    let cmds: Vec<String> = runner
        .calls
        .iter()
        .map(|(b, a)| format!("{b} {}", a.join(" ")))
        .collect();
    for expected in [
        format!("{IP_BIN} netns add {LAB_NETNS}"),
        format!("{IP_BIN} link add {HOST_VETH} type veth peer name {PEER_VETH}"),
        format!("{IP_BIN} addr add 10.204.0.1/30 dev {HOST_VETH}"),
        format!("{IP_BIN} netns exec {LAB_NETNS} {IP_BIN} addr add 10.204.0.2/30 dev {PEER_VETH}"),
        format!("{IP_BIN} netns exec {LAB_NETNS} {IP_BIN} addr add 192.0.2.53/32 dev lo"),
        format!("{IP_BIN} netns exec {LAB_NETNS} {IP_BIN} addr add 198.51.100.10/32 dev lo"),
        format!(
            "{IP_BIN} netns exec {LAB_NETNS} {IP_BIN} route add 10.203.0.0/24 via 10.204.0.1 dev {PEER_VETH}"
        ),
        format!("{IP_BIN} route add 192.0.2.53/32 via 10.204.0.2 dev {HOST_VETH}"),
        format!("{IP_BIN} route add 198.51.100.10/32 via 10.204.0.2 dev {HOST_VETH}"),
    ] {
        assert!(cmds.iter().any(|c| c == &expected), "missing {expected}");
    }
    assert!(cmds.iter().any(|c| {
        c.contains(SYSCTL_BIN)
            && c.contains("net.ipv6.conf.all.disable_ipv6=1")
            && c.contains("net.ipv4.ip_unprivileged_port_start=0")
    }));
    assert!(!cmds.iter().any(|c| c.contains("route add default")
        || (c.contains("iptables") && c.contains("-F"))
        || c.contains("-P ")));
    assert_eq!(
        cmds.iter().filter(|c| c.starts_with(IPTABLES_BIN)).count(),
        4
    );
    assert!(
        cmds.iter()
            .filter(|c| c.starts_with(IPTABLES_BIN))
            .all(|c| c.contains(comment))
    );
}

#[test]
fn preflight_refuses_every_collision_and_active_work_before_mutation() {
    assert!(check_preflight_collisions(&PreflightInputs::default()).is_ok());
    macro_rules! refuse {
        ($field:ident, $val:expr) => {{
            let mut inp = PreflightInputs::default();
            inp.$field = $val;
            assert!(
                check_preflight_collisions(&inp).is_err(),
                stringify!($field)
            );
        }};
    }
    refuse!(netns_paths_exist, vec!["dw-https-lab-r1".into()]);
    refuse!(ip_netns_list, "dw-https-lab-r1 (id: 0)\n".into());
    refuse!(ip_netns_list, "dwabcdef123456\n".into());
    refuse!(sys_net_exists, vec!["dwhlab1".into()]);
    refuse!(ip_link_show, "12: dwslab1@if11: <BROADCAST>".into());
    refuse!(ip_route_all, "10.204.0.0/30 dev dwhlab1".into());
    refuse!(ip_route_all, "192.0.2.53 via 10.204.0.2".into());
    refuse!(ip_addr_show, "    inet 198.51.100.10/32 lo".into());
    refuse!(iptables_docker_user, "-A DOCKER-USER -o dwhlab1".into());
    refuse!(etc_passwd, "nobody2:x:65531:65531::/:/nologin\n".into());
    refuse!(etc_group, "grp:x:65531:\n".into());
    refuse!(proc_uids_gids, vec![(1000, 65531)]);
    refuse!(docker_ps_names, "dwcap012345\n".into());
    refuse!(launcher_entries, vec!["0123abcdef/session.json".into()]);
    refuse!(
        active_dw_procs,
        vec!["/usr/local/libexec/duskweave/duskweave".into()]
    );
}

#[test]
fn responder_launch_drops_all_privileges_and_proc_status_validator_enforces_them() {
    let run_id = Uuid::new_v4();
    let args = build_responder_launch_args(
        "/srv/duskweave-build/https-lab-r1/go-release/httpslab-arm64",
        "timeout",
        run_id,
        30,
        30,
        PathBuf::from("/run/duskweave-https-lab-r1/inst1/responder/https-counters.json").as_path(),
    );
    assert!(
        args.join(" ")
            .starts_with(&format!("netns exec {LAB_NETNS} {SETPRIV_BIN} "))
    );
    for flag in [
        format!("--reuid={RESPONDER_UID}"),
        format!("--regid={RESPONDER_GID}"),
        "--clear-groups".into(),
        "--inh-caps=-all".into(),
        "--ambient-caps=-all".into(),
        "--bounding-set=-all".into(),
        "--no-new-privs".into(),
    ] {
        assert!(args.contains(&flag), "missing {flag}");
    }

    let clean_status = "Name:\thttpslab-arm64\nUid:\t65531\t65531\t65531\t65531\nGid:\t65531\t65531\t65531\t65531\nGroups:\t\nNoNewPrivs:\t1\nCapInh:\t0000000000000000\nCapPrm:\t0000000000000000\nCapEff:\t0000000000000000\nCapBnd:\t0000000000000000\nCapAmb:\t0000000000000000\n";
    assert!(verify_proc_status(clean_status, RESPONDER_UID, RESPONDER_GID).is_ok());

    for (from, to) in [
        ("Uid:\t65531\t65531\t65531\t65531", "Uid:\t0\t0\t0\t0"),
        (
            "Gid:\t65531\t65531\t65531\t65531",
            "Gid:\t65531\t0\t65531\t65531",
        ),
        ("Groups:\t", "Groups:\t0 27"),
        ("NoNewPrivs:\t1", "NoNewPrivs:\t0"),
        ("CapBnd:\t0000000000000000", "CapBnd:\t0000000000000400"),
        ("CapEff:\t0000000000000000", "CapEff:\t0000000000000001"),
    ] {
        let bad = clean_status.replace(from, to);
        assert!(
            verify_proc_status(&bad, RESPONDER_UID, RESPONDER_GID).is_err(),
            "{from}->{to}"
        );
    }

    let passwd = "root:x:0:0:root:/root:/bin/bash\nduskweave-core:x:998:997::/nonexistent:/usr/sbin/nologin\n";
    assert_eq!(resolve_core_gid_from_passwd(passwd, 998).unwrap(), 997);
    assert!(resolve_core_gid_from_passwd(passwd, 999).is_err());
}

#[test]
fn partial_setup_rolls_back_and_disposal_continues_after_individual_error() {
    let mut setup_runner =
        RecordingRunner::default().fail_matching(IPTABLES_BIN, "198.51.100.10/32");
    let mut partial = OwnedTopologyState::default();
    assert!(acquire_network_with(&mut setup_runner, "dw-https-lab-r1:p", &mut partial).is_err());
    assert!(
        !partial.has_any(),
        "partial setup must roll back all acquired resources"
    );
    let setup_cmds: Vec<String> = setup_runner
        .calls
        .iter()
        .map(|(b, a)| format!("{b} {}", a.join(" ")))
        .collect();
    assert_eq!(
        setup_cmds
            .iter()
            .filter(|c| c.contains("iptables -I"))
            .count(),
        3
    );
    assert_eq!(
        setup_cmds
            .iter()
            .filter(|c| c.contains("iptables -D"))
            .count(),
        2
    );
    assert!(
        setup_cmds
            .iter()
            .any(|c| c.contains("netns del dw-https-lab-r1"))
    );

    let mut runner = RecordingRunner::default().fail_matching(IPTABLES_BIN, "--sport 443");
    let mut owned = OwnedTopologyState {
        netns: true,
        host_veth: true,
        route_dns: true,
        route_https: true,
        iptables_rules: vec![0, 1, 2, 3],
    };
    let errs = dispose_network_with(&mut runner, "dw-https-lab-r1:t", &mut owned).unwrap_err();
    assert_eq!(errs.len(), 1);
    let cmds: Vec<String> = runner
        .calls
        .iter()
        .map(|(b, a)| format!("{b} {}", a.join(" ")))
        .collect();
    assert_eq!(
        cmds.iter().filter(|c| c.starts_with(IPTABLES_BIN)).count(),
        4
    );
    for expected in [
        format!("{IP_BIN} route del 198.51.100.10/32 via 10.204.0.2 dev {HOST_VETH}"),
        format!("{IP_BIN} route del 192.0.2.53/32 via 10.204.0.2 dev {HOST_VETH}"),
        format!("{IP_BIN} link del {HOST_VETH}"),
        format!("{IP_BIN} netns del {LAB_NETNS}"),
    ] {
        assert!(cmds.iter().any(|c| c == &expected), "missing {expected}");
    }
    assert_eq!(owned.iptables_rules, vec![3]);
    assert!(!owned.netns && !owned.host_veth && !owned.route_dns && !owned.route_https);
}

#[test]
fn control_root_fault_renames_only_owned_core_control_dir() {
    let base = std::env::temp_dir().join(format!("dw-hlab-fault-test-{}", Uuid::new_v4().simple()));
    let mut dirs = RuntimeDirs::new_under(&base, "inst1", true);
    for d in [
        &dirs.core_control_dir,
        &dirs.core_input_dir,
        &dirs.responder_dir,
    ] {
        std::fs::create_dir_all(d).unwrap();
    }
    let sibling = dirs.inject_control_root_loss("case1").unwrap();
    assert!(
        !dirs.core_control_dir.exists()
            && sibling.exists()
            && dirs.core_input_dir.exists()
            && dirs.responder_dir.exists()
    );
    assert!(dirs.inject_control_root_loss("case2").is_err());

    let mut shared = dirs.clone();
    shared.faulted_control_dir = None;
    shared.core_control_dir = PathBuf::from(SHARED_CORE_CONTROL_ROOT);
    assert!(shared.validate_control_fault_target("x").is_err());
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn proc_status_parser_uses_numeric_tokens_not_substrings() {
    let decoys = "Name:\tdecoy\nState:\tS (sleeping)\nUid:\t165531\t655310\t165531\t655310\nGid:\t265531\t655311\t265531\t655311\nVmSize:\t1024 kB\n";
    let decoy_sum = parse_proc_status_summary(decoys).unwrap();
    assert!(!decoy_sum.uids.contains(&RESPONDER_UID));
    assert!(!decoy_sum.gids.contains(&RESPONDER_GID));
    assert!(decoy_sum.has_vm_size && !decoy_sum.is_zombie && !decoy_sum.is_kthread);

    let zombie_kthread =
        "Name:\tz\nState:\tZ (zombie)\nUid:\t65531\t0\t0\t0\nGid:\t0\t65531\t0\t0\nKthread:\t1\n";
    let zk = parse_proc_status_summary(zombie_kthread).unwrap();
    assert!(zk.uids.contains(&RESPONDER_UID) && zk.gids.contains(&RESPONDER_GID));
    assert!(zk.is_zombie && zk.is_kthread && !zk.has_vm_size);
    assert!(parse_proc_status_summary("Uid:\tabc\nGid:\t0\t0\t0\t0\n").is_err());
}

#[cfg(target_os = "linux")]
#[test]
fn linux_scoped_dirs_enforce_0711_0700_and_rollback_preserves_collisions() {
    use super::topology_linux::{
        cleanup_runtime_dirs, create_scoped_dirs_with_owner, rollback_created_dirs,
    };
    use std::os::unix::fs::{MetadataExt, symlink};
    let owner = (
        rustix::process::geteuid().as_raw(),
        rustix::process::getegid().as_raw(),
    );
    let base = std::env::temp_dir().join(format!("dw-hlab-dir-test-{}", Uuid::new_v4().simple()));
    let root = base.join("root");
    std::fs::create_dir_all(&base).unwrap();

    let mut dirs = create_scoped_dirs_with_owner(&root, "inst1", owner, owner, owner).unwrap();
    for (dir, mode) in [
        (&root, 0o711),
        (&dirs.instance_dir, 0o711),
        (&dirs.responder_dir, 0o700),
        (&dirs.core_control_dir, 0o700),
    ] {
        assert_eq!(std::fs::symlink_metadata(dir).unwrap().mode() & 0o777, mode);
    }
    assert!(create_scoped_dirs_with_owner(&root, "inst2", owner, owner, owner).is_err());
    assert!(
        dirs.instance_dir.exists(),
        "collision must not delete existing inst1"
    );
    cleanup_runtime_dirs(&mut dirs).unwrap();
    assert!(!root.exists());

    // Non-empty collision target must remain untouched and report rollback failure.
    let stale_dir = base.join("stale");
    std::fs::create_dir_all(stale_dir.join("keep")).unwrap();
    assert!(rollback_created_dirs(std::slice::from_ref(&stale_dir)).is_err());
    assert!(stale_dir.join("keep").exists());

    let link_root = base.join("symroot");
    symlink(&base, &link_root).unwrap();
    assert!(create_scoped_dirs_with_owner(&link_root, "inst1", owner, owner, owner).is_err());
    let _ = std::fs::remove_dir_all(&base);
}

#[cfg(target_os = "linux")]
#[test]
fn linux_proc_scanner_fails_closed_on_errors_and_handles_kthread_zombie() {
    use super::topology_linux::scan_proc_state_at;
    let base = std::env::temp_dir().join(format!("dw-hlab-proc-{}", Uuid::new_v4().simple()));
    assert!(scan_proc_state_at(&base.join("missing"), 99999).is_err());

    std::fs::create_dir_all(base.join("100")).unwrap();
    std::fs::write(
        base.join("100/status"),
        "State:\tS (sleeping)\nUid:\t165531\t165531\t165531\t165531\nGid:\t165531\t165531\t165531\t165531\nVmSize:\t4096 kB\n",
    )
    .unwrap();
    // Live user process with missing exe must fail closed instead of pretending empty host.
    assert!(scan_proc_state_at(&base, 99999).is_err());

    let _ = std::fs::remove_dir_all(base.join("100"));
    std::fs::create_dir_all(base.join("101")).unwrap();
    std::fs::write(
        base.join("101/status"),
        "State:\tZ (zombie)\nUid:\t0\t0\t0\t0\nGid:\t0\t0\t0\t0\n",
    )
    .unwrap();
    std::fs::create_dir_all(base.join("102")).unwrap();
    std::fs::write(
        base.join("102/status"),
        "State:\tI (idle)\nUid:\t65531\t65531\t65531\t65531\nGid:\t0\t0\t0\t0\nKthread:\t1\n",
    )
    .unwrap();
    std::fs::write(base.join("102/cmdline"), b"").unwrap();
    let (ids, procs) = scan_proc_state_at(&base, 99999).unwrap();
    assert_eq!(ids, vec![(RESPONDER_UID, 0)]);
    assert!(procs.is_empty());
    let _ = std::fs::remove_dir_all(&base);
}

#[cfg(target_os = "linux")]
#[test]
fn linux_topology_immediate_responder_custody_and_disposal_errors() {
    use super::topology::ResponderHandle;
    use super::topology_linux::LabTopology;
    use std::os::unix::process::ExitStatusExt;
    use std::process::ExitStatus;
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };
    use std::time::Duration;

    struct FakeHandle {
        killed: Arc<AtomicBool>,
        kill_fails: bool,
        exit_on_kill: bool,
        already_exited: bool,
    }
    impl ResponderHandle for FakeHandle {
        fn id(&self) -> u32 {
            4242
        }
        fn kill(&mut self) -> std::io::Result<()> {
            self.killed.store(true, Ordering::SeqCst);
            (!self.kill_fails)
                .then_some(())
                .ok_or_else(|| std::io::Error::other("kill failed"))
        }
        fn try_wait(&mut self) -> std::io::Result<Option<ExitStatus>> {
            let done =
                self.already_exited || (self.exit_on_kill && self.killed.load(Ordering::SeqCst));
            Ok(done.then(|| ExitStatus::from_raw(0)))
        }
    }

    let tmp_dirs = || {
        let base = std::env::temp_dir().join(format!("dw-hlab-topo-{}", Uuid::new_v4().simple()));
        RuntimeDirs::new_under(&base, "inst", false)
    };
    let make_topo = |dirs, comment, netns, responder| LabTopology {
        dirs,
        rule_comment: comment,
        owned: OwnedTopologyState {
            netns,
            ..Default::default()
        },
        responder,
        reap_timeout: Duration::from_millis(20),
    };
    let fh = |killed, kill_fails, exit_on_kill, already_exited| -> Box<dyn ResponderHandle> {
        Box::new(FakeHandle {
            killed,
            kill_fails,
            exit_on_kill,
            already_exited,
        })
    };

    // 1. Readiness failure still kills/reaps immediately owned responder and rolls back network.
    let dirs1 = tmp_dirs();
    std::fs::create_dir_all(&dirs1.responder_dir).unwrap();
    std::fs::write(&dirs1.counter_path, b"{}").unwrap();
    let base1 = dirs1.root.clone();
    let killed = Arc::new(AtomicBool::new(false));
    let mut runner = RecordingRunner::default();
    let mut topo = make_topo(dirs1, "dw-https-lab-r1:custody".into(), false, None);
    let err = topo
        .start_with_spawn(
            &mut runner,
            || Ok(fh(killed.clone(), false, true, false)),
            |_| Err("proc check failed".into()),
        )
        .unwrap_err();
    assert!(
        err.contains("proc check failed")
            && killed.load(Ordering::SeqCst)
            && topo.responder.is_none()
    );
    let _ = std::fs::remove_dir_all(&base1);

    // 2. Reap failure preserves handle and error while still running network cleanup.
    let mut cleanup_runner = RecordingRunner::default();
    let mut unreaped = make_topo(
        tmp_dirs(),
        "dw-https-lab-r1:unreaped".into(),
        true,
        Some(fh(Arc::default(), true, false, false)),
    );
    assert!(
        !unreaped
            .dispose_with(&mut cleanup_runner)
            .unwrap_err()
            .is_empty()
            && unreaped.responder.is_some()
    );
    assert!(
        cleanup_runner
            .calls
            .iter()
            .any(|(b, a)| b == IP_BIN && a.join(" ") == "netns del dw-https-lab-r1")
    );
    unreaped.responder = None;

    // 3. Already-exited process clears custody cleanly even when kill would error.
    let mut exited = make_topo(
        tmp_dirs(),
        "dw-https-lab-r1:exited".into(),
        false,
        Some(fh(Arc::default(), true, false, true)),
    );
    exited
        .dispose_with(&mut RecordingRunner::default())
        .unwrap();
    assert!(exited.responder.is_none());
}
