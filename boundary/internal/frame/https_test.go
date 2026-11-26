package frame

import (
	"bytes"
	"encoding/json"
	"math"
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

func samplePublicHttpsRequest(now time.Time) HttpsRequest {
	q := sampleHttpsRequest(now)
	q.RunProfile = PublicRunProfile
	q.Hostname = "probe.example.invalid"
	q.IPv4 = "93.184.216.34"
	q.Resolver = "1.1.1.1"
	return q
}

func TestValidateHTTPSPublicHostnameAndTupleLimits(t *testing.T) {
	now := time.Now()
	valid := samplePublicHttpsRequest(now)
	if err := ValidateHTTPS(valid, now); err != nil {
		t.Fatalf("valid public request rejected: %v", err)
	}
	maxHost := strings.Repeat("a", 63) + "." + strings.Repeat("b", 63) + "." + strings.Repeat("c", 63) + "." + strings.Repeat("d", 61)
	if len(maxHost) != 253 {
		t.Fatalf("unexpected maxHost len %d", len(maxHost))
	}
	maxQ := valid
	maxQ.Hostname = maxHost
	if err := ValidateHTTPS(maxQ, now); err != nil {
		t.Fatalf("253-byte canonical hostname rejected: %v", err)
	}
	badHosts := []string{
		"", "localhost", "probe.example.invalid.", ".probe.example.invalid",
		"a..b.invalid", "-a.example.invalid", "a-.example.invalid",
		"Probe.example.invalid", "https://probe.example.invalid",
		"probe.example.invalid/path", "93.184.216.34", "bad_name.example.invalid",
		strings.Repeat("a", 64) + ".example.invalid",
		strings.Repeat("a", 63) + "." + strings.Repeat("b", 63) + "." + strings.Repeat("c", 63) + "." + strings.Repeat("d", 62),
	}
	for _, h := range badHosts {
		q := valid
		q.Hostname = h
		if ValidateHTTPS(q, now) == nil {
			t.Fatalf("invalid public hostname accepted: %q", h)
		}
	}
	mutants := []struct {
		name   string
		mutate func(*HttpsRequest)
	}{
		{"path_health", func(q *HttpsRequest) { q.Path = "/health" }},
		{"port_8443", func(q *HttpsRequest) { q.Port = 8443 }},
		{"method_get", func(q *HttpsRequest) { q.Method = "GET" }},
		{"mode_fixture", func(q *HttpsRequest) { q.Mode = "controlled_fixture" }},
		{"profile_inference", func(q *HttpsRequest) { q.Profile = "deepseek-genai-v4-pro-low-v2" }},
		{"unknown_run_profile", func(q *HttpsRequest) { q.RunProfile = "autonomous-dns-genai-live-v1" }},
		{"public_with_lab_resolver", func(q *HttpsRequest) { q.Resolver = LabResolver }},
		{"lab_with_public_host", func(q *HttpsRequest) { q.RunProfile = LabRunProfile }},
		{"zero_duration", func(q *HttpsRequest) { q.Duration = 0 }},
		{"over_duration", func(q *HttpsRequest) { q.Duration = 5001 }},
		{"overflow_deadline", func(q *HttpsRequest) { q.Deadline = math.MaxInt64 - 500; q.EpisodeDeadline = math.MaxInt64 }},
		{"non_canonical_ip", func(q *HttpsRequest) { q.IPv4 = "093.184.216.34" }},
		{"non_canonical_resolver", func(q *HttpsRequest) { q.Resolver = "01.1.1.1" }},
	}
	for _, m := range mutants {
		q := valid
		m.mutate(&q)
		if ValidateHTTPS(q, now) == nil {
			t.Fatalf("invalid public mutant accepted: %s", m.name)
		}
	}
}

func TestValidateHTTPSPublicSpecialIPv4Matrix(t *testing.T) {
	now := time.Now()
	valid := samplePublicHttpsRequest(now)
	denied := []string{
		"0.0.0.0", "0.255.255.255", "10.0.0.0", "10.255.255.255",
		"100.64.0.0", "100.127.255.255", "127.0.0.0", "127.0.0.1", "127.255.255.255",
		"169.254.0.0", "169.254.255.255", "172.16.0.0", "172.31.255.255",
		"192.0.0.0", "192.0.0.255", "192.0.2.0", "192.0.2.53", "192.0.2.255",
		"192.88.99.0", "192.88.99.255", "192.168.0.0", "192.168.255.255",
		"198.18.0.0", "198.19.255.255", "198.51.100.0", "198.51.100.10", "198.51.100.255",
		"203.0.113.0", "203.0.113.255", "224.0.0.0", "240.0.0.0", "255.255.255.255",
		"::1", "not-an-ip",
	}
	for _, addr := range denied {
		qTarget := valid
		qTarget.IPv4 = addr
		if ValidateHTTPS(qTarget, now) == nil {
			t.Fatalf("special target IPv4 accepted: %s", addr)
		}
		qRes := valid
		qRes.Resolver = addr
		if ValidateHTTPS(qRes, now) == nil {
			t.Fatalf("special resolver IPv4 accepted: %s", addr)
		}
	}
	allowedBoundaries := []string{
		"1.0.0.0", "9.255.255.255", "11.0.0.0", "100.63.255.255", "100.128.0.0",
		"126.255.255.255", "128.0.0.0", "172.15.255.255", "172.32.0.0",
		"192.0.1.0", "198.17.255.255", "198.20.0.0", "223.255.255.255",
	}
	for _, addr := range allowedBoundaries {
		q := valid
		q.IPv4 = addr
		q.Resolver = addr
		if err := ValidateHTTPS(q, now); err != nil {
			t.Fatalf("global boundary IPv4 rejected %s: %v", addr, err)
		}
	}
}
