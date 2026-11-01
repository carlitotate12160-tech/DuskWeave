use super::*;

#[test]
fn admission_refuses_a_non_canonical_invocation() {
    // The test process always carries harness arguments and a nonzero euid,
    // so harden() must refuse before any socket or rlimit is touched.
    let error = serve().unwrap_err();
    assert_eq!(error, Fail::Config("live_host_unqualified"));
}
