use super::*;
use duskweave::Fail;
use std::cell::Cell;
use std::time::{Duration, Instant};

#[test]
fn derived_names_are_deterministic_and_tagged() {
    let attempt = uuid::Uuid::from_u128(0x1234);
    let n = super::super::names(attempt);
    let tag = attempt.simple().to_string()[..12].to_string();
    assert_eq!(n.netns, format!("dw{tag}"));
    assert_eq!(n.host, format!("dvh{tag}"));
    assert_eq!(n.inner, format!("dvn{tag}"));
    assert_eq!(n.table, format!("dwf{tag}"));
    assert_eq!(n.base & 0x03, 0, "base keeps only the 6 high bits");
}

#[test]
fn absent_and_destroy_tolerate_resources_that_never_existed() {
    let n = super::super::names(uuid::Uuid::new_v4());
    assert!(absent(&n), "fresh names map to no host resources");
    if rustix::process::geteuid().as_raw() != 0 {
        return;
    }
    // Destroy must not fail when the link, netns and nft table are all absent;
    // absent nft tables are skipped via the list checks, never blindly deleted.
    let end = Instant::now() + Duration::from_secs(5);
    assert!(destroy(&n, end), "absent resources are already destroyed");
}

#[test]
fn table_listing_requires_complete_inet_table_lines() {
    let target = "dwf123456789012";
    assert_eq!(
        table_listed(&format!("table inet {target}\n"), target),
        Ok(true)
    );
    assert_eq!(table_listed("table inet other\n", target), Ok(false));
    assert_eq!(table_listed("", target), Ok(false));
    assert_eq!(
        table_listed(&format!("table inet other\ntable inet {target}\n"), target),
        Ok(true)
    );
    for malformed in [
        "table ip other\n",
        "table inet target extra\n",
        "table inet\n",
        "table inet other\npartial",
        " ",
    ] {
        assert!(table_listed(malformed, target).is_err());
        assert!(!table_disposed_with(
            target,
            Instant::now() + Duration::from_secs(1),
            |_, _| Ok(malformed.to_string()),
            |_, _| Ok(()),
        ));
    }
}
#[test]
fn table_disposed_with_confirms_absence_when_table_not_in_listing() {
    let end = Instant::now() + Duration::from_secs(5);
    let deleted = Cell::new(false);
    let disposed = table_disposed_with(
        "dwf_target",
        end,
        |_cmd, _end| Ok("table inet foreign_a\ntable inet foreign_b\n".to_string()),
        |_cmd, _end| {
            deleted.set(true);
            Ok(())
        },
    );
    assert!(disposed, "table not in listing must be confirmed absent");
    assert!(!deleted.get(), "confirmed absent table must not be deleted");
}

#[test]
fn table_disposed_with_deletes_present_table_and_succeeds() {
    let end = Instant::now() + Duration::from_secs(5);
    let delete_cmd = Cell::new(String::new());
    let disposed = table_disposed_with(
        "dwf_target",
        end,
        |_cmd, _end| Ok("table inet foreign\ntable inet dwf_target\n".to_string()),
        |cmd, _end| {
            delete_cmd.set(cmd.to_string());
            Ok(())
        },
    );
    assert!(
        disposed,
        "present table deleted successfully must be disposed"
    );
    assert_eq!(
        delete_cmd.take(),
        "delete table inet dwf_target\n",
        "must delete exact target table"
    );
}

#[test]
fn table_disposed_with_deletion_failure_retains_ownership() {
    let end = Instant::now() + Duration::from_secs(5);
    let disposed = table_disposed_with(
        "dwf_target",
        end,
        |_cmd, _end| Ok("table inet dwf_target\n".to_string()),
        |_cmd, _end| Err(Fail::Config("delete_denied")),
    );
    assert!(
        !disposed,
        "failed deletion must not be reported as disposed"
    );
}

#[test]
fn table_disposed_with_query_failure_does_not_report_absence() {
    let end = Instant::now() + Duration::from_secs(5);
    let disposed = table_disposed_with(
        "dwf_target",
        end,
        |_cmd, _end| Err(Fail::Unresolved("nft_query_unavailable")),
        |_cmd, _end| Ok(()),
    );
    assert!(
        !disposed,
        "query failure must not be reported as absence/disposal"
    );
}

#[test]
fn table_disposed_with_deadline_expired_fails_closed() {
    let past = Instant::now() - Duration::from_millis(50);
    let called = Cell::new(false);
    let disposed = table_disposed_with(
        "dwf_target",
        past,
        |_cmd, _end| {
            called.set(true);
            Ok(String::new())
        },
        |_cmd, _end| Ok(()),
    );
    assert!(
        !disposed,
        "expired deadline must fail closed without querying"
    );
    assert!(
        !called.get(),
        "must not query when deadline already expired"
    );
}

#[test]
fn reproduce_namespace_disposal_finding_failed_specific_query_not_absence() {
    // REPRODUCTION:
    // In the earlier implementation, a general listing succeeded (listed = true)
    // while a subsequent specific query failed (e.g. deadline or command failure),
    // causing `present = false` and `(!present && listed)` to evaluate to `true`.
    // That falsely reported confirmed absence/disposal on query failure.
    //
    // In the corrected implementation, absence is ONLY confirmed when the
    // listing succeeds AND the table is genuinely absent from the listing.
    // Any query failure or unresolved state fails closed.
    let end = Instant::now() + Duration::from_secs(5);
    let listing_failed = table_disposed_with(
        "dwf_target",
        end,
        |_cmd, _end| Err(Fail::Unresolved("query_interrupted")),
        |_cmd, _end| Ok(()),
    );
    assert!(
        !listing_failed,
        "failed listing/query must NEVER be reported as confirmed absence"
    );
}

#[test]
fn destroy_with_handles_all_resource_combinations() {
    let n = super::super::names(uuid::Uuid::new_v4());
    let end = Instant::now() + Duration::from_secs(5);

    // 1. Everything clean
    let ok = destroy_with(
        &n,
        end,
        |_cmd, _end| Ok(()),
        |_cmd, _end| Ok("table inet other\n".to_string()),
        |_cmd, _end| Ok(()),
    );
    assert!(ok, "clean destruction must succeed");

    // 2. Table query fails -> overall destruction fails closed
    let table_fail = destroy_with(
        &n,
        end,
        |_cmd, _end| Ok(()),
        |_cmd, _end| Err(Fail::Config("listing_failed")),
        |_cmd, _end| Ok(()),
    );
    assert!(!table_fail, "table query failure must fail overall destroy");

    // 3. Deadline expired
    let past = Instant::now() - Duration::from_millis(50);
    let expired = destroy_with(
        &n,
        past,
        |_cmd, _end| Ok(()),
        |_cmd, _end| Ok(String::new()),
        |_cmd, _end| Ok(()),
    );
    assert!(!expired, "expired deadline must return false");
}

#[test]
fn rule_sets_format_correct_firewall_and_masquerade() {
    let attempt = uuid::Uuid::from_u128(0xabcd);
    let n = super::super::names(attempt);
    let resolver = "10.0.0.53";
    let (inside, outside) = rule_sets(&n, resolver);

    // Inside rules must isolate netns to only resolver UDP 53
    assert!(inside.contains(&format!("table inet {}", n.table)));
    assert!(inside.contains(&format!("ip daddr {resolver} udp dport 53 accept")));
    assert!(inside.contains(&format!(
        "ip saddr {resolver} udp sport 53 ct state established accept"
    )));
    assert!(inside.contains("policy drop"));

    // Outside rules must masquerade and filter veth with 2000ms timeout
    assert!(outside.contains(&format!("table inet {}", n.table)));
    assert!(outside.contains("flags timeout; timeout 2000ms;"));
    assert!(outside.contains("masquerade"));
    assert!(outside.contains(&format!("iifname \"{}\"", n.host)));
    assert!(outside.contains(&format!("oifname \"{}\"", n.host)));
}
