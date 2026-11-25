package frame

import (
	"bytes"
	"encoding/json"
	"strings"
	"testing"
	"time"
)

func sampleHttpsRequest(now time.Time) HttpsRequest {
	id := "00000000-0000-0000-0000-000000000001"
	deadline := now.Add(2 * time.Second).UnixMilli()
	return HttpsRequest{
		Identity:        Identity{Schema: HttpsSchema, Kind: "REQUEST", Attempt: id, Generation: 1, Sequence: 0},
		Engagement:      id,
		Campaign:        id,
		Decision:        id,
		Run:             id,
		StepIndex:       1,
		AttemptKind:     "head",
		Result:          id,
		Profile:         "https-head-v1",
		Mode:            "live",
		RunProfile:      "autonomous-https-genai-lab-v1",
		Release:         strings.Repeat("ab", 32),
		Qualification:   id,
		Asset:           id,
		Endpoint:        "endpoint_0",
		Hostname:        "head.duskweave.test",
		IPv4:            "198.51.100.10",
		Port:            443,
		Path:            "/",
		Method:          "HEAD",
		ParentClaim:     id,
		ParentRevision:  1,
		Resolver:        "192.0.2.53",
		Vantage:         id,
		Permission:      id,
		MissionRevision: 1,
		EvaluatedAt:     now.UnixMilli(),
		ClaimUntil:      now.Add(time.Minute).Unix(),
		EpisodeDeadline: deadline + 1000,
		Deadline:        deadline,
		Duration:        5000,
		Limits: Limits{
			NetworkInboundBytes: 65536,
			HeaderBytes:         8192,
			FramePayloadBytes:   16384,
			ControlPayloadBytes: 1024,
			MaxWorkerFrames:     16,
			MinTLSVersion:       "1.2",
			HTTPVersion:         "HTTP/1.1",
			MaxConnects:         1,
			MaxRequests:         1,
		},
	}
}

func TestValidateHTTPSCapabilityProfileAndClosedTuple(t *testing.T) {
	now := time.Now()
	valid := sampleHttpsRequest(now)
	if err := ValidateHTTPS(valid, now); err != nil {
		t.Fatalf("F1: valid https-head-v1 request rejected: %v", err)
	}

	// F1: inference profile placed in capability profile must be refused even when all else matches.
	mutants := []struct {
		name   string
		mutate func(*HttpsRequest)
	}{
		{"inference_profile_alias", func(q *HttpsRequest) { q.Profile = "fixture-genai-v4-pro-low-v2" }},
		{"wrong_mode", func(q *HttpsRequest) { q.Mode = "controlled_fixture" }},
		{"wrong_run_profile", func(q *HttpsRequest) { q.RunProfile = "autonomous-dns-genai-lab-v1" }},
		{"wrong_hostname", func(q *HttpsRequest) { q.Hostname = "other.duskweave.test" }},
		{"wrong_ip", func(q *HttpsRequest) { q.IPv4 = "198.51.100.11" }},
		{"wrong_port", func(q *HttpsRequest) { q.Port = 8443 }},
		{"wrong_path", func(q *HttpsRequest) { q.Path = "/admin" }},
		{"wrong_method", func(q *HttpsRequest) { q.Method = "GET" }},
		{"wrong_resolver", func(q *HttpsRequest) { q.Resolver = "192.0.2.54" }},
		{"wrong_attempt_kind", func(q *HttpsRequest) { q.AttemptKind = "initial" }},
		{"bad_step_zero", func(q *HttpsRequest) { q.StepIndex = 0 }},
		{"bad_step_five", func(q *HttpsRequest) { q.StepIndex = 5 }},
		{"nil_uuid", func(q *HttpsRequest) { q.Qualification = "00000000-0000-0000-0000-000000000000" }},
		{"bad_endpoint", func(q *HttpsRequest) { q.Endpoint = "endpoint_x" }},
		{"bad_release_hex", func(q *HttpsRequest) { q.Release = strings.Repeat("AB", 32) }},
		{"zero_generation", func(q *HttpsRequest) { q.Generation = 0 }},
		{"nonzero_sequence", func(q *HttpsRequest) { q.Sequence = 1 }},
		{"expired_deadline", func(q *HttpsRequest) { q.Deadline = now.UnixMilli() }},
		{"exceeded_duration", func(q *HttpsRequest) { q.Duration = 5001 }},
		{"deadline_beyond_duration", func(q *HttpsRequest) { q.Deadline = now.UnixMilli() + 6000; q.EpisodeDeadline = q.Deadline }},
		{"episode_before_deadline", func(q *HttpsRequest) { q.EpisodeDeadline = q.Deadline - 1 }},
		{"future_evaluated_at", func(q *HttpsRequest) { q.EvaluatedAt = now.UnixMilli() + 1 }},
		{"expired_claim", func(q *HttpsRequest) { q.ClaimUntil = now.Unix() }},
		{"wrong_header_limit", func(q *HttpsRequest) { q.Limits.HeaderBytes = 8193 }},
	}
	for _, m := range mutants {
		q := valid
		m.mutate(&q)
		if ValidateHTTPS(q, now) == nil {
			t.Fatalf("invalid request accepted for %s", m.name)
		}
	}
}

func TestHttpsFrameStrictJSONDecoding(t *testing.T) {
	now := time.Now()
	valid := sampleHttpsRequest(now)
	raw, err := json.Marshal(valid)
	if err != nil {
		t.Fatal(err)
	}
	if len(raw) > int(Limit) {
		t.Fatalf("request exceeded frame limit: %d", len(raw))
	}
	var decoded HttpsRequest
	if err := Decode(raw, &decoded); err != nil {
		t.Fatalf("valid JSON decode failed: %v", err)
	}
	// Duplicate key must reject.
	dup := bytes.Replace(raw, []byte(`"method":"HEAD"`), []byte(`"method":"HEAD","method":"HEAD"`), 1)
	if Decode(dup, &decoded) == nil {
		t.Fatal("duplicate field accepted")
	}
	// Unknown field must reject.
	extra := bytes.Replace(raw, []byte(`"method":"HEAD"`), []byte(`"method":"HEAD","extra":1`), 1)
	if Decode(extra, &decoded) == nil {
		t.Fatal("unknown field accepted")
	}
	// Null field must reject.
	nulled := bytes.Replace(raw, []byte(`"method":"HEAD"`), []byte(`"method":null`), 1)
	if Decode(nulled, &decoded) == nil {
		t.Fatal("null field accepted")
	}
}
