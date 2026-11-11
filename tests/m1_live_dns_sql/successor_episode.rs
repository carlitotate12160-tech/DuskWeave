//! Schema-0030 real-CLI SDK loopback: a fresh v3 fixture run drives the
//! actual m1-run entrypoint through two provider decisions, controlled DNS
//! feedback, durable history and key-free inspect/recover on the retained
//! 0030 assert path.
use super::fixture::*;
use super::run_support::*;
use serde_json::{Value, json};
use uuid::Uuid;

/// Whitelisted CLI environment: the worker's *_FILE variants stay absent so
/// env-local mode admits exactly the injected owner DSNs.
fn command() -> std::process::Command {
    let mut c = std::process::Command::new(env!("CARGO_BIN_EXE_duskweave"));
    c.env_clear();
    c.env("DW_DATABASE_CONFIG_MODE", "env-local").env(
        "DW_DATABASE_URL",
        std::env::var("DW_TEST_DATABASE_URL").unwrap(),
    );
    for key in [
        "DW_INFERENCE_BROKER_DATABASE_URL",
        "DW_COGNITION_DATABASE_URL",
        "DW_TRAJECTORY_DATABASE_URL",
        "DW_TERRAIN_DATABASE_URL",
        "SystemRoot",
        "LLVM_PROFILE_FILE",
    ] {
        if let Some(v) = std::env::var_os(key) {
            c.env(key, v);
        }
    }
    c
}

fn cli(op: &str, run: Uuid) -> (bool, Value) {
    let output = command()
        .args([op, "--run", &run.to_string()])
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&output.stdout);
    let receipt = text
        .lines()
        .find(|line| line.starts_with('{'))
        .map(|line| serde_json::from_str(line).unwrap())
        .unwrap_or_else(|| json!({"diagnostic": text.trim()}));
    (output.status.success(), receipt)
}

/// Private write-once control markers only; same host qualification as the
/// existing provider lanes.
fn control_root() -> std::path::PathBuf {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root =
        std::env::temp_dir().join(format!("dw-sql30-control-{}-{stamp}", std::process::id()));
    std::fs::create_dir(&root).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700)).unwrap();
    }
    #[cfg(windows)]
    {
        use std::process::Stdio;
        let system = std::env::var_os("SystemRoot").unwrap();
        let script = "$ErrorActionPreference='Stop'; \
            $s=[Security.Principal.WindowsIdentity]::GetCurrent().User; \
            $a=New-Object Security.AccessControl.DirectorySecurity; $a.SetOwner($s); \
            $a.SetAccessRuleProtection($true,$false); \
            $r=New-Object Security.AccessControl.FileSystemAccessRule($s,'FullControl','ContainerInherit,ObjectInherit','None','Allow'); \
            $a.AddAccessRule($r); Set-Acl -LiteralPath $env:DW_M1_RUN_CONTROL_ROOT -AclObject $a";
        let status = std::process::Command::new(
            std::path::Path::new(&system).join("System32/WindowsPowerShell/v1.0/powershell.exe"),
        )
        .env_clear()
        .env("SystemRoot", system)
        .env("DW_M1_RUN_CONTROL_ROOT", &root)
        .args(["-NoProfile", "-NonInteractive", "-Command", script])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .unwrap();
        assert!(
            status.success(),
            "private test control root ACL prerequisite"
        );
    }
    root
}

/// Immutable before/after witness for inspect/recover: run identity fields,
/// recorded decision entity/event pairs, durable call markers with binding
/// digests, fixture result identity and the issued Terrain publication.
fn run_identities(e: Uuid, c: Uuid, run: Uuid) -> Value {
    let mut a = admin();
    let run_row = a
        .query_one(
            "SELECT decision_one,decision_two,attempt,result,terrain_operation FROM execution.autonomous_runs WHERE run_id=$1",
            &[&run],
        )
        .unwrap();
    let terrain_op: Uuid = run_row.get(4);
    let decisions: Value = a
        .query_one(
            "SELECT coalesce(jsonb_agg(jsonb_build_array(d,payload->>'decision',payload->>'event') ORDER BY d),'[]') FROM cognition.reasoning_decisions WHERE e=$1 AND c=$2",
            &[&e, &c],
        )
        .unwrap()
        .get(0);
    let calls: Value = a
        .query_one(
            "SELECT coalesce(jsonb_agg(jsonb_build_array(call,binding,state) ORDER BY call),'[]') FROM execution.inference_calls WHERE e=$1 AND c=$2",
            &[&e, &c],
        )
        .unwrap()
        .get(0);
    let results: Value = a
        .query_one(
            "SELECT coalesce(jsonb_agg(jsonb_build_array(attempt,result) ORDER BY attempt),'[]') FROM execution.fixture_dns_results WHERE run_id=$1",
            &[&run],
        )
        .unwrap()
        .get(0);
    let effects: i64 = a
        .query_one(
            "SELECT (SELECT count(*) FROM execution.fixture_dns_attempts WHERE run_id=$1)+(SELECT count(*) FROM execution.fixture_dns_results WHERE run_id=$1)+(SELECT count(*) FROM execution.live_effect_sessions WHERE run_id=$1)+(SELECT count(*) FROM execution.live_dns_results WHERE run_id=$1)",
            &[&run],
        )
        .unwrap()
        .get(0);
    let terrain: Value = a
        .query_one(
            "SELECT jsonb_build_array(t.event_id,(SELECT count(*) FROM trajectory.terrain_history WHERE event_id=t.event_id AND status='accepted')) FROM terrain.outbox t WHERE (t.engagement_id,t.campaign_id,t.operation_id)=($1,$2,$3)",
            &[&e, &c, &terrain_op],
        )
        .unwrap()
        .get(0);
    json!({
        "run": [run_row.get::<_, Uuid>(0), run_row.get::<_, Uuid>(1),
            run_row.get::<_, Uuid>(2), run_row.get::<_, Uuid>(3), terrain_op],
        "decisions": decisions,
        "calls": calls,
        "results": results,
        "effects": effects,
        "terrain": terrain,
    })
}

#[test]
#[ignore = "runs only inside the dedicated schema-0030 fixture worker"]
fn sql_live_sdk_v3_cli_feedback_and_key_free_recovery_on_schema0030() {
    assert!(worker());
    let identity = successor_pack_identity();

    // Fresh v3 registration (permission v5, fixture-genai profile) and the
    // typed v3 run request derived from the accepted publication.
    let (e, c, _operator) = register_live(0x6e01, false);
    let run = Uuid::new_v4();
    let publication = live_publication(e, c, run);
    let request = successor_request(&publication, run, "m1_autonomous_run_request_v3");
    let input = std::env::temp_dir().join(format!("dw-sql30-run-{run}.json"));
    std::fs::write(&input, serde_json::to_vec(&request).unwrap()).unwrap();

    // Loopback GenAI fixture endpoint; an occupied port refuses the case.
    let server = genai::Server::lab(|r| {
        let wire: Value = serde_json::from_slice(&r.body).unwrap();
        let pack = genai::pack_in(&wire).expect("SDK request carries immutable pack");
        assert_eq!(pack["version"], "m1_context_pack_autonomous_v3");
        assert_eq!(wire["model"], "deepseek-flash");
        assert_eq!(wire["reasoning"]["effort"], "low");
        assert_eq!(wire["max_output_tokens"], 4096);
        assert_eq!(wire["store"], false);
        assert!(wire["stream"].as_bool().is_none_or(|v| !v));
        for field in [
            "temperature",
            "top_p",
            "previous_response_id",
            "conversation",
        ] {
            assert!(wire.get(field).is_none());
        }
        assert!(wire["tools"].as_array().is_none_or(|v| v.is_empty()));
        genai::Reply::Length(
            200,
            serde_json::to_vec(&genai::envelope(&genai::proposal(&pack))).unwrap(),
        )
    });
    assert_eq!(server.address.port(), 18083);

    let root = control_root();
    let output = command()
        .env("DW_M1_RUN_CONTROL_ROOT", &root)
        .args([
            "m1-run",
            "--input",
            input.to_str().unwrap(),
            "--timing-profile",
            "reasoning-bounded-v1",
        ])
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "m1-run on schema0030: {text}; stderr={}; sends={}",
        String::from_utf8_lossy(&output.stderr),
        server.count()
    );
    let receipt: Value = serde_json::from_str(text.lines().last().unwrap()).unwrap();
    assert_eq!(receipt["kind"], "m1_autonomous_run_receipt_v1");
    assert_eq!(receipt["status"], "completed");
    assert_eq!(receipt["decision_count"], 2);
    assert_eq!(receipt["decision_history_count"], 2);
    assert_eq!(receipt["effect_count"], 1);
    assert_eq!(receipt["terrain_history_count"], 1);
    assert_eq!(receipt["acquisition"], "committed_fixture_result");
    assert_eq!(receipt["terrain"], "committed");
    let calls = receipt["inference"].as_array().unwrap();
    assert_eq!(calls.len(), 2, "one accounting row per decision");
    for row in calls {
        assert_eq!(row["state"], "completed");
    }
    assert_eq!(
        receipt["recorded_decisions"][0]["proposal"]["next_intent"]["category"],
        "observe_more"
    );
    assert_eq!(
        receipt["recorded_decisions"][1]["proposal"]["next_intent"]["category"],
        "retain"
    );
    assert_ne!(
        receipt["recorded_decisions"][0]["proposal"]["hypothesis"],
        receipt["recorded_decisions"][1]["proposal"]["hypothesis"],
        "D2 must consume real predecessor feedback, not a replayed proposal"
    );
    assert_eq!(server.count(), 2, "one provider send per decision");
    for r in server.requests.lock().unwrap().iter() {
        assert!(r.body.len() <= 16384, "provider wire byte cap");
    }

    // Immutable identities captured BEFORE any inspect/recover; a query
    // issued only afterwards is no before/after witness.
    let identities = run_identities(e, c, run);

    // Key-free inspection and repeated recovery stay offline: no new send,
    // identical durable identities after each action.
    for op in ["m1-run-inspect", "m1-run-recover", "m1-run-recover"] {
        let (ok, r) = cli(op, run);
        assert!(ok, "{op}: {r}");
        assert_eq!(r["run_id"], json!(run));
        assert_eq!(r["decision_count"], 2);
        assert_eq!(r["effect_count"], 1);
        assert_eq!(
            run_identities(e, c, run),
            identities,
            "{op} must leave decision/event/call/binding/attempt/result/Terrain identities unchanged"
        );
    }
    assert_eq!(
        server.count(),
        2,
        "key-free inspect/recover must never contact provider"
    );

    // Marker and accounting identities: pack/intent bytes stay inside the
    // admitted caps and the stored binding digests verify locally.
    let rows = admin()
        .query(
            "SELECT p,configuration,body,binding,state FROM execution.inference_calls WHERE e=$1",
            &[&e],
        )
        .unwrap();
    assert_eq!(rows.len(), 2);
    let requests = server.requests.lock().unwrap();
    let mut second_pack: Option<Value> = None;
    for row in rows {
        let (p, i, b, h, state): (String, String, String, String, String) =
            (row.get(0), row.get(1), row.get(2), row.get(3), row.get(4));
        assert_eq!(state, "completed");
        assert!(b.is_empty(), "SDK intents store no legacy body");
        assert_eq!(
            duskweave::m1_reasoning::inference_intent::digest(p.as_bytes(), i.as_bytes()),
            h,
            "stored binding must verify over stored pack and intent"
        );
        assert!(p.len() <= 8192 && i.len() <= 16384, "pack/intent byte caps");
        assert!(
            !i.contains("DEEPSEEK_API_KEY") && !i.contains("synthetic-sdk-fixture-key"),
            "intents carry no credential material"
        );
        let pack: Value = serde_json::from_str(&p).unwrap();
        let intent: Value = serde_json::from_str(&i).unwrap();
        let wire = requests
            .iter()
            .map(|r| serde_json::from_slice::<Value>(&r.body).unwrap())
            .find(|v| genai::pack_in(v).unwrap()["decision_id"] == intent["decision_id"])
            .expect("every stored intent must map to one provider wire request");
        assert!(
            wire["text"]["format"]["schema"] == intent["schema"],
            "provider wire schema must equal the bound intent schema"
        );
        if pack["decision_id"] != json!(identities["run"][0].clone()) {
            second_pack = Some(pack);
        }
    }
    drop(requests);

    // D2 linkage: the second pack's previous_decision reference equals the
    // actual D1 decision/event identities, and its predecessor binds the
    // issued Terrain publication and fixture result — not equal counts or a
    // different hypothesis alone.
    let (d1, d2): (Uuid, Uuid) = (
        serde_json::from_value(identities["run"][0].clone()).unwrap(),
        serde_json::from_value(identities["run"][1].clone()).unwrap(),
    );
    let second = second_pack.expect("a stored pack for decision_two");
    assert_eq!(second["decision_id"], json!(d2));
    let d1_row = admin()
        .query_one(
            "SELECT payload->>'decision',payload->>'event',payload->>'recorded_at_ms',payload#>'{proposal,hypothesis}' FROM cognition.reasoning_decisions WHERE e=$1 AND c=$2 AND d=$3",
            &[&e, &c, &d1],
        )
        .unwrap();
    let prior_ref = second["references"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["id"] == "previous_decision")
        .expect("D2 pack previous_decision reference");
    assert_eq!(prior_ref["entity"], json!(d1_row.get::<_, String>(0)));
    assert_eq!(prior_ref["event"], json!(d1_row.get::<_, String>(1)));
    assert_eq!(
        prior_ref["recorded_at"],
        json!(d1_row.get::<_, String>(2).parse::<i64>().unwrap() / 1000)
    );
    assert_eq!(prior_ref["status"], "committed_historical_decision");
    assert_eq!(prior_ref["source"], "cognition");
    assert_eq!(
        second["predecessor"]["terrain_publication_ref"], identities["terrain"][0],
        "D2 predecessor must reference the issued Terrain publication event"
    );
    assert_eq!(
        second["predecessor"]["result_ref"], identities["run"][3],
        "D2 predecessor must reference the durable fixture result"
    );
    assert_eq!(second["predecessor"]["attempt_ref"], identities["run"][2]);
    assert_eq!(
        second["predecessor"]["hypothesis_summary"],
        d1_row.get::<_, Value>(3),
        "D2 predecessor carries the actual D1 hypothesis"
    );
    assert_eq!(
        identities["terrain"][1],
        json!(1),
        "issued Terrain publication accepted in history"
    );

    assert_eq!(
        successor_pack_identity(),
        identity,
        "installed0030 assert_successor_pack must be unchanged by the episode"
    );
    eprintln!(
        "schema0030_sdk_loopback run={run} sends={} e={e}",
        server.count()
    );
    std::fs::remove_file(input).unwrap();
}
