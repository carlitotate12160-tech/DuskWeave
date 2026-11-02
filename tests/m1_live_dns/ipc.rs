use duskweave::m1_dns::{DnsResult, FrameReader};
use serde_json::{Value, json};
use std::io::Cursor;

fn result() -> Value {
    json!({"schema":"m1_dns_ipc_v1","kind":"RESULT",
        "attempt":"00000000-0000-0000-0000-000000000001","session_generation":1,
        "sequence":3,"status":"completed","observed_at_ms":1000,"rcode":0,
        "truncated":false,"complete":true,"withheld_aliases":0,
        "answers":[{"ipv4":"198.51.100.7","ttl_seconds":30}],"aliases":[]})
}
fn frame(raw: &[u8]) -> Vec<u8> {
    let mut framed = (raw.len() as u32).to_be_bytes().to_vec();
    framed.extend_from_slice(raw);
    framed
}
fn decode(raw: &[u8]) -> Result<DnsResult, duskweave::Fail> {
    FrameReader::new(Cursor::new(frame(raw))).result()
}

#[test]
fn trusted_result_codec_rejects_noncanonical_shapes_and_raw_fields() {
    let valid = result();
    assert!(decode(&serde_json::to_vec(&valid).unwrap()).is_ok());
    for field in [
        "schema", "attempt", "status", "complete", "answers", "aliases",
    ] {
        let mut bad = valid.clone();
        bad.as_object_mut().unwrap().remove(field);
        assert!(
            decode(&serde_json::to_vec(&bad).unwrap()).is_err(),
            "required {field}"
        );
    }
    for (field, value) in [
        ("packet", json!("RAW_SENTINEL")),
        ("status", json!("arbitrary_detail")),
        ("sequence", json!(4294967296u64)),
        ("answers", json!([["198.51.100.7", 30]])),
        (
            "answers",
            json!([{"ipv4":"198.51.100.7","ttl_seconds":30,"detail":"RAW_SENTINEL"}]),
        ),
        (
            "answers",
            json!(vec![json!({"ipv4":"198.51.100.7","ttl_seconds":30}); 65]),
        ),
    ] {
        let mut bad = valid.clone();
        bad[field] = value;
        assert!(
            decode(&serde_json::to_vec(&bad).unwrap()).is_err(),
            "closed {field}"
        );
    }
    let text = serde_json::to_string(&valid).unwrap();
    let duplicate = text.replacen('{', "{\"sequence\":3,", 1);
    assert!(decode(duplicate.as_bytes()).is_err());
    assert!(decode(format!("{text}{text}").as_bytes()).is_err());
    assert!(decode(b"null").is_err());
}

#[test]
fn frame_lengths_and_partial_input_fail_without_allocating_claimed_capacity() {
    for bytes in [
        vec![],
        vec![0, 0, 0],
        vec![255; 4],
        vec![0, 0, 64, 1],
        vec![0, 0, 0, 2, b'{'],
    ] {
        assert!(FrameReader::new(Cursor::new(bytes)).result().is_err());
    }
}

#[test]
fn stop_only_host_commands_have_closed_correlated_wire_contracts() {
    use duskweave::m1_effect_control::{HostChannel, HostCommand};
    for raw in [
        br#"{"kind":"StopRun","run_id":"00000000-0000-0000-0000-000000000001"}"#.as_slice(),
        br#"{"kind":"StopCampaign","engagement_id":"00000000-0000-0000-0000-000000000001","campaign_id":"00000000-0000-0000-0000-000000000002"}"#.as_slice(),
    ] {
        let command = HostChannel::new(Cursor::new(frame(raw))).command();
        assert!(command.is_ok(), "authenticated stop must not require acquisition data");
        let encoded = serde_json::to_vec(&command.unwrap()).unwrap();
        let mut extra: Value = serde_json::from_slice(&encoded).unwrap();
        extra["path"] = json!("/caller-controlled");
        let mut channel = HostChannel::new(Cursor::new(frame(&serde_json::to_vec(&extra).unwrap())));
        assert!(channel.command().is_err(), "stop commands admit no executable or path");
        let duplicate = String::from_utf8(encoded).unwrap().replacen('{', "{\"kind\":\"StopRun\",", 1);
        assert!(HostChannel::new(Cursor::new(frame(duplicate.as_bytes()))).command().is_err());
    }
    let _ = std::mem::size_of::<HostCommand>();
}

#[test]
fn host_channel_framing_and_size_bounds() {
    use duskweave::Fail;
    use duskweave::m1_effect_control::{HostChannel, HostReply};

    let mut chan = HostChannel::new(Cursor::new([0, 0, 0, 0]));
    match chan.command() {
        Err(e) => assert_eq!(e, Fail::State("live_host_protocol_refused")),
        Ok(_) => panic!("expected failure"),
    }

    let mut big_hdr = (16385u32).to_be_bytes().to_vec();
    big_hdr.extend_from_slice(&vec![0u8; 16385]);
    let mut chan = HostChannel::new(Cursor::new(big_hdr));
    match chan.command() {
        Err(e) => assert_eq!(e, Fail::State("live_host_protocol_refused")),
        Ok(_) => panic!("expected failure"),
    }

    let bad_start = frame(b" [1, 2, 3]");
    let mut chan = HostChannel::new(Cursor::new(bad_start));
    match chan.command() {
        Err(e) => assert_eq!(e, Fail::State("live_host_protocol_refused")),
        Ok(_) => panic!("expected failure"),
    }

    let single_frame = frame(br#"{"kind":"Refused"}"#);
    let mut stream = Vec::new();
    for _ in 0..17 {
        stream.extend_from_slice(&single_frame);
    }
    let mut chan = HostChannel::new(Cursor::new(stream));
    for _ in 0..16 {
        assert!(chan.reply().is_ok());
    }
    match chan.reply() {
        Err(e) => assert_eq!(e, Fail::State("live_host_protocol_refused")),
        Ok(_) => panic!("expected 16-frame failure"),
    }

    let mut chan = HostChannel::new(Cursor::new(Vec::new()));
    for _ in 0..16 {
        assert!(chan.send_reply(&HostReply::Refused).is_ok());
    }
    match chan.send_reply(&HostReply::Refused) {
        Err(e) => assert_eq!(e, Fail::State("live_host_protocol_refused")),
        Ok(_) => panic!("expected 16-frame send failure"),
    }
}

#[test]
fn host_reply_roundtrip_all_variants() {
    use duskweave::m1_effect_control::{HostChannel, HostReply};
    use uuid::Uuid;

    let replies = [
        HostReply::Ready {
            attempt: Uuid::from_u128(1),
            generation: 2,
        },
        HostReply::Stopped {
            disposal_confirmed: true,
        },
        HostReply::RunStopped {
            run_id: Uuid::from_u128(3),
            stop_latched: true,
            disposal_confirmed: false,
        },
        HostReply::CampaignStopped {
            engagement_id: Uuid::from_u128(4),
            campaign_id: Uuid::from_u128(5),
            stop_latched: false,
            disposal_confirmed: true,
        },
        HostReply::Refused,
        HostReply::Qualified {
            qualification_ref: Uuid::from_u128(6),
            manifest_sha256: "b".repeat(64),
            boot_id: Uuid::from_u128(7),
        },
    ];

    for reply in replies {
        let mut cur = Cursor::new(Vec::new());
        let mut sender = HostChannel::new(&mut cur);
        sender.send_reply(&reply).unwrap();
        let bytes = cur.into_inner();
        let mut receiver = HostChannel::new(Cursor::new(bytes));
        let decoded = receiver.reply().unwrap();
        match (reply, decoded) {
            (
                HostReply::Ready {
                    attempt: a1,
                    generation: g1,
                },
                HostReply::Ready {
                    attempt: a2,
                    generation: g2,
                },
            ) => {
                assert_eq!((a1, g1), (a2, g2));
            }
            (
                HostReply::Stopped {
                    disposal_confirmed: d1,
                },
                HostReply::Stopped {
                    disposal_confirmed: d2,
                },
            ) => {
                assert_eq!(d1, d2);
            }
            (
                HostReply::RunStopped { run_id: r1, .. },
                HostReply::RunStopped { run_id: r2, .. },
            ) => {
                assert_eq!(r1, r2);
            }
            (
                HostReply::CampaignStopped {
                    campaign_id: c1, ..
                },
                HostReply::CampaignStopped {
                    campaign_id: c2, ..
                },
            ) => {
                assert_eq!(c1, c2);
            }
            (HostReply::Refused, HostReply::Refused) => {}
            (
                HostReply::Qualified {
                    qualification_ref: q1,
                    ..
                },
                HostReply::Qualified {
                    qualification_ref: q2,
                    ..
                },
            ) => {
                assert_eq!(q1, q2);
            }
            _ => panic!("variant mismatch"),
        }
    }
}

#[test]
fn control_lifecycle_and_frame_reader_writer() {
    use duskweave::m1_dns::{
        Control, DnsRequest, FrameReader, FrameWriter, MAX_CONTROL, MAX_FRAME,
    };
    use uuid::Uuid;

    let req_val = serde_json::json!({
        "schema": "m1_dns_ipc_v1",
        "kind": "REQUEST",
        "attempt": "00000000-0000-0000-0000-000000000001",
        "session_generation": 1,
        "sequence": 0,
        "engagement_id": "00000000-0000-0000-0000-000000000002",
        "campaign_id": "00000000-0000-0000-0000-000000000003",
        "decision_id": "00000000-0000-0000-0000-000000000004",
        "profile": "dns-routing-v2",
        "execution_mode": "live",
        "run_profile": "autonomous-dns-genai-lab-v1",
        "release_sha256": "a".repeat(64),
        "qualification_ref": "00000000-0000-0000-0000-000000000005",
        "name": "example.com",
        "resolver_ipv4": "192.0.2.53",
        "transport": "udp",
        "query_id": 1,
        "allowed_names": ["example.com"],
        "deadline_at_ms": 2000,
        "max_duration_ms": 2000
    });
    let req: DnsRequest = serde_json::from_value(req_val).unwrap();

    let ctrl = Control::new(&req, "READY", 1);
    assert!(ctrl.validate(&req, "READY", 1).is_ok());

    assert!(ctrl.validate(&req, "START", 1).is_err());
    assert!(ctrl.validate(&req, "READY", 2).is_err());
    let mut bad_req = req.clone();
    bad_req.attempt = Uuid::new_v4();
    assert!(ctrl.validate(&bad_req, "READY", 1).is_err());
    let mut bad_req = req.clone();
    bad_req.session_generation = 99;
    assert!(ctrl.validate(&bad_req, "READY", 1).is_err());
    let mut bad_req = req.clone();
    bad_req.deadline_at_ms = 99999;
    assert!(ctrl.validate(&bad_req, "READY", 1).is_err());
    let bad_schema = Control {
        schema: "wrong".into(),
        ..Control::new(&req, "READY", 1)
    };
    assert!(bad_schema.validate(&req, "READY", 1).is_err());
    let bad_schema = Control { schema: "wrong".into(), ..Control::new(&req, "READY", 1) };
    assert!(bad_schema.validate(&req, "READY", 1).is_err());

    let mut cur = Cursor::new(Vec::new());
    let mut writer = FrameWriter::new(&mut cur);
    writer.write(&ctrl, MAX_CONTROL).unwrap();
    let bytes = cur.into_inner();
    let mut reader = FrameReader::new(Cursor::new(bytes.clone()));
    let read_ctrl = reader.control().unwrap();
    assert_eq!(read_ctrl.kind, "READY");

    let mut cur = Cursor::new(Vec::new());
    let mut writer = FrameWriter::new(&mut cur);
    writer.write(&req, MAX_FRAME).unwrap();
    let req_bytes = cur.into_inner();
    let mut reader = FrameReader::new(Cursor::new(req_bytes));
    let read_req = reader.request().unwrap();
    assert_eq!(read_req.name, "example.com");

    let mut cur = Cursor::new(Vec::new());
    let mut writer = FrameWriter::new(&mut cur);
    assert!(writer.write(&req, 5).is_err());

    let mut cur = Cursor::new(Vec::new());
    let mut writer = FrameWriter::new(&mut cur);
    for _ in 0..16 {
        assert!(writer.write(&ctrl, MAX_CONTROL).is_ok());
    }
    assert!(writer.write(&ctrl, MAX_CONTROL).is_err());

    let mut all_bytes = Vec::new();
    for _ in 0..16 {
        all_bytes.extend_from_slice(&bytes);
    }
    let mut reader = FrameReader::new(Cursor::new(all_bytes));
    for _ in 0..16 {
        assert!(reader.control().is_ok());
    }
    assert!(reader.control().is_err());

    // Non-object start
    assert!(
        FrameReader::new(Cursor::new(vec![0, 0, 0, 2, b'[', b']']))
            .control()
            .is_err()
    );
    // Bad JSON
    assert!(
        FrameReader::new(Cursor::new(vec![0, 0, 0, 3, b'{', b'x', b'}']))
            .control()
            .is_err()
    );

    let mut all_bytes = Vec::new();
    for _ in 0..16 {
        all_bytes.extend_from_slice(&bytes);
    }
    let mut reader = FrameReader::new(Cursor::new(all_bytes));
    for _ in 0..16 {
        assert!(reader.control().is_ok());
    }
    assert!(reader.control().is_err());

    // Non-object start
    assert!(FrameReader::new(Cursor::new(vec![0, 0, 0, 2, b'[', b']'])).control().is_err());
    // Bad JSON
    assert!(FrameReader::new(Cursor::new(vec![0, 0, 0, 3, b'{', b'x', b'}'])).control().is_err());
}
