package export

import (
	"bytes"
	"encoding/base64"
	"encoding/json"
	"strings"
	"testing"
	"time"

	"duskweave/boundary/internal/frame"
)

func labRequest(now time.Time) frame.HttpsRequest {
	id := "00000000-0000-0000-0000-000000000001"
	deadline := now.Add(2 * time.Second).UnixMilli()
	return frame.HttpsRequest{
		Identity:   frame.Identity{Schema: frame.HttpsSchema, Kind: "REQUEST", Attempt: id, Generation: 1},
		Engagement: id, Campaign: id, Decision: id, Run: id, StepIndex: 1, AttemptKind: "head", Result: id,
		Profile: "https-head-v1", Mode: "live", RunProfile: frame.LabRunProfile, Release: strings.Repeat("ab", 32),
		Qualification: id, Asset: id, Endpoint: "endpoint_0", Hostname: frame.LabHost, IPv4: frame.LabIPv4,
		Port: 443, Path: "/", Method: "HEAD", ParentClaim: id, ParentRevision: 1, Resolver: frame.LabResolver,
		Vantage: id, Permission: id, MissionRevision: 1, EvaluatedAt: now.UnixMilli(), ClaimUntil: now.Add(time.Minute).Unix(),
		EpisodeDeadline: deadline + 1000, Deadline: deadline, Duration: 5000,
		Limits: frame.Limits{NetworkInboundBytes: 65536, HeaderBytes: 8192, FramePayloadBytes: 16384,
			ControlPayloadBytes: 1024, MaxWorkerFrames: 16, MinTLSVersion: "1.2", HTTPVersion: "HTTP/1.1",
			MaxConnects: 1, MaxRequests: 1},
	}
}

func runExporterWithClock(q frame.HttpsRequest, permits []string, raw frame.PrivateResult, withAck bool, nowFn func() time.Time) (frame.SafeResult, error) {
	var input, output bytes.Buffer
	w := frame.Writer{Sink: &input}
	seqMap := map[string]uint32{"connect": 2, "tls": 4, "head": 6}
	frames := []any{q}
	for _, p := range permits {
		frames = append(frames, frame.PhaseStart{Identity: frame.HttpsID(q.Identity, "START", seqMap[p]), Phase: p, Deadline: q.Deadline})
	}
	frames = append(frames, raw)
	if withAck {
		frames = append(frames, frame.HttpsAck{Identity: frame.HttpsID(q.Identity, "ACK", 8)})
	}
	for _, value := range frames {
		if err := w.Write(value, frame.Limit); err != nil {
			return frame.SafeResult{}, err
		}
	}
	in := frame.Reader{Source: &input}
	out := frame.Writer{Sink: &output}
	payload, err := in.Take(frame.Limit)
	if err != nil {
		return frame.SafeResult{}, err
	}
	in.Count++
	if err := runHTTPSWithClock(&in, &out, payload, nowFn); err != nil {
		return frame.SafeResult{}, err
	}
	r := frame.Reader{Source: &output}
	var ready frame.PhaseReady
	if err := r.Read(&ready, frame.ControlLimit); err != nil {
		return frame.SafeResult{}, err
	}
	var safe frame.SafeResult
	if err := r.Read(&safe, frame.Limit); err != nil {
		return frame.SafeResult{}, err
	}
	return safe, nil
}

func runExporterFrames(q frame.HttpsRequest, permits []string, raw frame.PrivateResult, withAck bool) (frame.SafeResult, error) {
	return runExporterWithClock(q, permits, raw, withAck, time.Now)
}

func TestExporterProfileSeparationF1(t *testing.T) {
	now := time.Now()
	q := labRequest(now)
	header := "HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n"
	raw := frame.PrivateResult{Identity: frame.HttpsID(q.Identity, "RESULT", 7), Phase: "head",
		Status: "received", Observed: now.UnixMilli(), Header: base64.StdEncoding.EncodeToString([]byte(header)), TLS: "none"}
	if safe, err := runExporterFrames(q, []string{"connect", "tls", "head"}, raw, true); err != nil || safe.Outcome != "head_response" {
		t.Fatalf("F1: valid https-head-v1 request failed: err=%v safe=%+v", err, safe)
	}
	q.Profile = "fixture-genai-v4-pro-low-v2"
	if _, err := runExporterFrames(q, []string{"connect", "tls", "head"}, raw, true); err == nil {
		t.Fatal("F1: inference profile in capability field must be rejected")
	}
}

func TestExporterDropsHeaderSentinelAndStrictHeaderValidationF2(t *testing.T) {
	now := time.Now()
	q := labRequest(now)
	redirectHeader := "HTTP/1.1 302 Found\r\nLocation: https://evil.example/RAW_SENTINEL\r\n\r\n"
	raw := frame.PrivateResult{Identity: frame.HttpsID(q.Identity, "RESULT", 7), Phase: "head",
		Status: "received", Observed: now.UnixMilli(), Header: base64.StdEncoding.EncodeToString([]byte(redirectHeader)), TLS: "none"}

	if _, err := runExporterFrames(q, []string{"connect"}, raw, true); err == nil {
		t.Fatal("head result without tls and head permits must fail closed")
	}
	safe, err := runExporterFrames(q, []string{"connect", "tls", "head"}, raw, true)
	if err != nil || safe.Outcome != "head_response" || safe.StatusCode != 302 || safe.Redirect != "refused_redirect" {
		t.Fatalf("valid redirect projection failed: err=%v safe=%+v", err, safe)
	}
	encodedSafe, _ := json.Marshal(safe)
	if bytes.Contains(encodedSafe, []byte("RAW_SENTINEL")) || bytes.Contains(encodedSafe, []byte("evil.example")) {
		t.Fatal("ordinary output retained raw header material")
	}

	badHeaders := []string{
		"HTTP/1.1 200 OK\r\nServer: incomplete",
		"HTTP/1.1 200 OK\r\nBrokenHeaderNoColon\r\n\r\n",
		"HTTP/1.1 200 OK\r\nBad Name: v\r\n\r\n",
		"HTTP/1.1 200 OK\r\nServer: a\r\n obs-fold\r\n\r\n",
		"HTTP/1.1 200 OK\r\nContent-Length: 5\r\n\r\nhello",
		"HTTP/1.1 200 OK\r\n\r\nHTTP/1.1 200 OK\r\n\r\n",
		"HTTP/1.1 200 OK\r\nContent-Length: 5\r\nTransfer-Encoding: chunked\r\n\r\n",
		"HTTP/1.1 200 OK\r\nContent-Length: 5\r\nContent-Length: 6\r\n\r\n",
		"HTTP/1.1 101 Switching Protocols\r\n\r\n",
		"HTTP/2 200 OK\r\n\r\n",
		"HTTP/1.0 200 OK\r\n\r\n",
	}
	for _, hdr := range badHeaders {
		badRaw := frame.PrivateResult{Identity: frame.HttpsID(q.Identity, "RESULT", 7), Phase: "head",
			Status: "received", Observed: now.UnixMilli(), Header: base64.StdEncoding.EncodeToString([]byte(hdr)), TLS: "none"}
		if _, err := runExporterFrames(q, []string{"connect", "tls", "head"}, badRaw, true); err == nil {
			t.Fatalf("F2: invalid/unsupported header became valid export: %q", hdr)
		}
	}

	legalWithH2Token := "HTTP/1.1 200 OK\r\nVia: 1.1 proxy, HTTP/2.0 gw\r\n\r\n"
	okRaw := frame.PrivateResult{Identity: frame.HttpsID(q.Identity, "RESULT", 7), Phase: "head",
		Status: "received", Observed: now.UnixMilli(), Header: base64.StdEncoding.EncodeToString([]byte(legalWithH2Token)), TLS: "none"}
	if safe, err := runExporterFrames(q, []string{"connect", "tls", "head"}, okRaw, true); err != nil || safe.Outcome != "head_response" {
		t.Fatalf("F2: legal header value containing HTTP/2.0 rejected: err=%v", err)
	}

	prefix := "HTTP/1.1 200 OK\r\nX-Pad: "
	suffix := "\r\n\r\n"
	exact8k := prefix + strings.Repeat("a", 8192-len(prefix)-len(suffix)) + suffix
	raw8k := frame.PrivateResult{Identity: frame.HttpsID(q.Identity, "RESULT", 7), Phase: "head",
		Status: "received", Observed: now.UnixMilli(), Header: base64.StdEncoding.EncodeToString([]byte(exact8k)), TLS: "none"}
	if safe, err := runExporterFrames(q, []string{"connect", "tls", "head"}, raw8k, true); err != nil || safe.Header != 8192 {
		t.Fatalf("exact 8192 header rejected: err=%v", err)
	}
	over8k := exact8k[:len(exact8k)-len(suffix)] + "a" + suffix
	rawOver := frame.PrivateResult{Identity: frame.HttpsID(q.Identity, "RESULT", 7), Phase: "head",
		Status: "received", Observed: now.UnixMilli(), Header: base64.StdEncoding.EncodeToString([]byte(over8k)), TLS: "none"}
	if _, err := runExporterFrames(q, []string{"connect", "tls", "head"}, rawOver, true); err == nil {
		t.Fatal("8193 header accepted")
	}
}

func TestExporterClosedPhaseStatusMatrixF3(t *testing.T) {
	now := time.Now()
	q := labRequest(now)
	headConnFail := frame.PrivateResult{Identity: frame.HttpsID(q.Identity, "RESULT", 7), Phase: "head",
		Status: "connection_failed", Observed: now.UnixMilli(), Network: 1200, TLS: "none"}
	safe, err := runExporterFrames(q, []string{"connect", "tls", "head"}, headConnFail, true)
	if err != nil || safe.Outcome != "connection_failed" {
		t.Fatalf("F3: head connection_failed refused by exporter: err=%v safe=%+v", err, safe)
	}
	tlsAlpn := frame.PrivateResult{Identity: frame.HttpsID(q.Identity, "RESULT", 7), Phase: "tls",
		Status: "unsupported_http", Observed: now.UnixMilli(), Network: 600, TLS: "none"}
	safe, err = runExporterFrames(q, []string{"connect", "tls"}, tlsAlpn, true)
	if err != nil || safe.Outcome != "execution_refused" {
		t.Fatalf("F3: tls ALPN unsupported_http refused by exporter: err=%v safe=%+v", err, safe)
	}
	if _, err := runExporterFrames(q, []string{"connect", "tls"}, frame.PrivateResult{
		Identity: frame.HttpsID(q.Identity, "RESULT", 7), Phase: "tls",
		Status: "connection_failed", Observed: now.UnixMilli(), TLS: "none",
	}, true); err == nil {
		t.Fatal("connection_failed accepted at tls phase")
	}
	if _, err := runExporterFrames(q, []string{"connect"}, frame.PrivateResult{
		Identity: frame.HttpsID(q.Identity, "RESULT", 7), Phase: "connect",
		Status: "header_limit", Observed: now.UnixMilli(), TLS: "none",
	}, true); err == nil {
		t.Fatal("header_limit accepted at connect phase")
	}
}

func TestExporterTerminalTimeoutWindowF4(t *testing.T) {
	base := time.Unix(1_700_000_000, 0)
	q := labRequest(base)
	deadlineTime := time.UnixMilli(q.Deadline)

	// Deterministic clock: Validate/READY/START run before deadline, RESULT arrives at deadline+250ms.
	step := 0
	clock := func() time.Time {
		step++
		if step <= 4 {
			return base
		}
		return deadlineTime.Add(250 * time.Millisecond)
	}
	rawTimeout := frame.PrivateResult{Identity: frame.HttpsID(q.Identity, "RESULT", 7), Phase: "connect",
		Status: "timed_out", Observed: q.Deadline + 250, TLS: "none"}
	safe, err := runExporterWithClock(q, []string{"connect"}, rawTimeout, true, clock)
	if err != nil || safe.Outcome != "timed_out" || safe.Observed != q.Deadline+250 {
		t.Fatalf("F4: post-deadline timed_out within <=1000ms handling window rejected in full run: err=%v safe=%+v", err, safe)
	}

	// Late received success after deadline refuses even with clock inside handling window.
	step = 0
	lateOK := frame.PrivateResult{Identity: frame.HttpsID(q.Identity, "RESULT", 7), Phase: "head",
		Status: "received", Observed: q.Deadline + 10, Header: base64.StdEncoding.EncodeToString([]byte("HTTP/1.1 200 OK\r\n\r\n")), TLS: "none"}
	if _, err := runExporterWithClock(q, []string{"connect", "tls", "head"}, lateOK, true, func() time.Time {
		step++
		if step <= 6 {
			return base
		}
		return deadlineTime.Add(250 * time.Millisecond)
	}); err == nil {
		t.Fatal("F4: late received success after deadline was accepted in run")
	}

	// Future observed > receiver current time refuses.
	futureTimeout := frame.PrivateResult{Identity: frame.HttpsID(q.Identity, "RESULT", 7), Phase: "connect",
		Status: "timed_out", Observed: q.Deadline + 500, TLS: "none"}
	if acceptPrivateAt(q, "connect", futureTimeout, q.Deadline+200) {
		t.Fatal("F4: future observation accepted")
	}
	// Timed_out beyond +1000ms refuses.
	if acceptPrivateAt(q, "connect", frame.PrivateResult{Identity: frame.HttpsID(q.Identity, "RESULT", 7), Phase: "connect",
		Status: "timed_out", Observed: q.Deadline + 1001, TLS: "none"}, q.Deadline+1500) {
		t.Fatal("F4: timed_out beyond deadline+1000ms accepted")
	}
	// Overflow on deadline refuses.
	overflowQ := q
	overflowQ.Deadline = 1<<63 - 500
	if acceptPrivateAt(overflowQ, "connect", rawTimeout, base.UnixMilli()) {
		t.Fatal("F4: overflow deadline accepted")
	}
}
