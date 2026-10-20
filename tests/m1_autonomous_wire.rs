use duskweave::m1_reasoning::request;
use serde_json::{Value, json};

#[test]
fn autonomous_request_consumer_accepts_all_six_design_packs_and_binds_full_wire() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../docs/contracts/fixtures/m1-autonomous-wire-examples-v1.json"
    ))
    .unwrap();
    for case in fixture["cases"].as_array().unwrap() {
        let p = case["pack"].as_str().unwrap();
        let repair_request: Value =
            serde_json::from_str(case["repair_request"].as_str().unwrap()).unwrap();
        let repair_fields: Value =
            serde_json::from_str(repair_request["input"][1]["content"].as_str().unwrap()).unwrap();
        let codes: Vec<&str> = repair_fields["repair_error_codes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        for (repair, key, hash) in [
            (None, "generation_request", "generation_binding"),
            (Some(codes.as_slice()), "repair_request", "repair_binding"),
        ] {
            let bound = request::bind(p.as_bytes(), repair)
                .expect("autonomous pack accepted by real consumer");
            assert!(
                bound.configuration == case["configuration"].as_str().unwrap().as_bytes(),
                "configuration {}",
                case["name"]
            );
            assert!(
                bound.body == case[key].as_str().unwrap().as_bytes(),
                "body {}",
                case["name"]
            );
            assert_eq!(bound.digest, case[hash]);
            request::verify_binding(
                p,
                std::str::from_utf8(&bound.configuration).unwrap(),
                std::str::from_utf8(&bound.body).unwrap(),
                &bound.digest,
                repair.is_some(),
            )
            .unwrap();
        }
    }
}

#[test]
fn context_limit_is_profile_specific_and_counts_utf8_bytes() {
    for (version, limit) in [
        ("m1_context_pack_v1", 4096),
        ("m1_context_pack_autonomous_v1", 8192),
    ] {
        let mut p = json!({"version":version,"references":[{"id":"asset_0"}],"padding":["","","","","","","","",""]});
        let base = serde_json::to_vec(&p).unwrap().len();
        let mut remaining = limit - base;
        for v in p["padding"].as_array_mut().unwrap() {
            let n = remaining.min(1024);
            *v = json!("x".repeat(n));
            remaining -= n;
        }
        assert_eq!(remaining, 0);
        let mut bytes = serde_json::to_vec(&p).unwrap();
        assert_eq!(bytes.len(), limit);
        assert!(request::reference_ids(&bytes).is_ok());
        bytes.push(b' ');
        assert!(request::reference_ids(&bytes).is_err());
    }
}
