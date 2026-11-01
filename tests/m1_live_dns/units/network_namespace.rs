use super::*;

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
    // Destroy must not fail when the link, netns and nft table are all absent;
    // absent nft tables are skipped via the list checks, never blindly deleted.
    let end = Instant::now() + std::time::Duration::from_secs(5);
    assert!(destroy(&n, end), "absent resources are already destroyed");
}
