//! Schema-0030 admission SQL regressions: single canonical seed and legacy support.
use super::fixture::*;
use serde_json::{Value, json};
use uuid::Uuid;

#[test]
#[ignore = "runs only inside the dedicated schema-0030 fixture worker"]
fn sql_live_v4_admission_requires_single_canonical_seed() {
    assert!(worker());
    let (e, c, _operator) = register_live(0x5c01, true);
    let now: i64 = admin()
        .query_one(
            "SELECT floor(extract(epoch FROM clock_timestamp())*1000)::bigint",
            &[],
        )
        .unwrap()
        .get(0);
    let asset = Uuid::new_v4();
    let request = |seeds: Value, version: &str, profile: &str| {
        let mut q = json!({"version":version,"run_id":Uuid::new_v4(),"run_profile":profile,
            "execution_mode":if version=="m1_autonomous_run_request_v4" {"live"} else {"controlled_fixture"},
            "deadline_at_ms":now+300000,"inference_profile":"fixture-genai-responses-v1",
            "context":{"version":"m1_context_prepare_request_v1","mission":{"engagement_id":e,"campaign_id":c},
                "candidates":seeds.as_array().unwrap().iter().map(|s| json!({"asset_ref":s["asset_ref"],"priority":"high"})).collect::<Vec<_>>()},
            "seed_names":seeds});
        if version == "m1_autonomous_run_request_v4" {
            q["live_binding"] = json!({"qualification_ref":Uuid::new_v4(),"release_sha256":RELEASE,"resolver_ipv4":"192.0.2.53"});
        } else {
            q.as_object_mut().unwrap().remove("live_binding");
        }
        q
    };
    let seed = |name: &str| json!({"asset_ref":asset,"name":name});
    let v4 = "m1_autonomous_run_request_v4";
    let v3 = "m1_autonomous_run_request_v3";
    let start = |q: &Value| broker().query_one("SELECT execution.run_start($1,$2)", &[q, &RELEASE]);
    // Exactly one v4 seed/candidate: two seeds refuse before any write.
    for q in [
        request(
            json!([seed("a.duskweave.test"), seed("b.duskweave.test")]),
            v4,
            LAB,
        ),
        request(json!([seed(".duskweave.test")]), v4, LAB),
        request(json!([seed("duskweave.test")]), v4, LAB),
        request(json!([seed("other.test")]), v4, LAB),
        request(json!([seed("seed.example.com")]), v4, LAB),
    ] {
        let error = start(&q).unwrap_err();
        assert!(
            db_message(&error).contains("run_request_refused"),
            "{q}: {error}"
        );
    }
    // A canonical LAB name with a nonempty (possibly multi-label) prefix is
    // consistent with Rust admission and passes the seed gate, refusing only
    // later at authority publication. Legacy versions keep 1-8 seed semantics.
    for q in [
        request(json!([seed("a.b.duskweave.test")]), v4, LAB),
        request(json!([seed("seed.duskweave.test")]), v4, LAB),
        request(
            json!([seed("x.example"), seed("y.example")]),
            v3,
            "autonomous-dns-fixture-v2",
        ),
    ] {
        let error = start(&q).unwrap_err();
        assert!(
            !db_message(&error).contains("run_request_refused"),
            "{q}: {error}"
        );
        assert!(
            db_message(&error).contains("run_authority_unavailable"),
            "{q}: {error}"
        );
    }
    // Refusals consumed no run identity (leases/sessions cannot exist without one).
    assert_eq!(
        admin()
            .query_one(
                "SELECT count(*) FROM execution.autonomous_runs WHERE e=$1",
                &[&e]
            )
            .unwrap()
            .get::<_, i64>(0),
        0
    );
}
