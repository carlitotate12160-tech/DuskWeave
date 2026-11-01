package export

import (
	"bytes"
	"duskweave/boundary/internal/dns"
	"duskweave/boundary/internal/frame"
	"io"
	"strings"
	"testing"
	"time"
)

func request() frame.Request {
	id := "00000000-0000-0000-0000-000000000001"
	return frame.Request{Identity: frame.Identity{Schema: frame.Schema, Kind: "REQUEST", Attempt: id, Generation: 1},
		Engagement: id, Campaign: id, Decision: id, Qualification: id, Release: strings.Repeat("a", 64),
		Mode: "live", Profile: "dns-routing-v2", RunProfile: "autonomous-dns-genai-lab-v1", Transport: "udp",
		Name: "seed.duskweave.test", Resolver: "192.0.2.53", AllowedNames: []string{"seed.duskweave.test"},
		Deadline: time.Now().Add(2 * time.Second).UnixMilli(), Duration: 2000, QueryID: 7}
}

func TestIndependentParentAndRawPipesProjectOnlyAllowedDNS(t *testing.T) {
	q := request()
	parentR, parentW := io.Pipe()
	rawR, rawW := io.Pipe()
	safeR, safeW := io.Pipe()
	defer parentR.Close()
	defer parentW.Close()
	defer rawR.Close()
	defer rawW.Close()
	defer safeR.Close()
	defer safeW.Close()
	finished := make(chan error, 1)
	go func() { finished <- Run(parentR, rawR, safeW) }()
	control := frame.Writer{Sink: parentW}
	out := frame.Reader{Source: safeR}
	if control.Write(q, frame.Limit) != nil {
		t.Fatal("request")
	}
	var ready frame.Control
	if out.Read(&ready, frame.ControlLimit) != nil || !frame.Matches(ready.Identity, q, "READY", 1) {
		t.Fatal("READY")
	}
	if control.Write(frame.Control{Identity: frame.Bound(q, "START", 2), Deadline: q.Deadline}, frame.ControlLimit) != nil {
		t.Fatal("START")
	}
	packet, _ := dns.Query(q.QueryID, q.Name)
	packet[2], packet[3], packet[7] = 0x81, 0x80, 1
	packet = append(packet, 0xc0, 12, 0, 1, 0, 1, 0, 0, 0, 30, 0, 4, 198, 51, 100, 7)
	raw := frame.Raw{Identity: frame.Bound(q, "RESULT", 3), Source: q.Resolver, Status: "received", Observed: time.Now().UnixMilli(), Packet: packet}
	if (&frame.Writer{Sink: rawW}).Write(raw, frame.Limit) != nil {
		t.Fatal("candidate")
	}
	rawW.Close()
	var result frame.Result
	if out.Read(&result, frame.Limit) != nil || result.Status != "completed" || len(result.Answers) != 1 || result.Answers[0].IPv4 != "198.51.100.7" {
		t.Fatal("safe result")
	}
	if control.Write(frame.Control{Identity: frame.Bound(q, "ACK", 4), Deadline: q.Deadline}, frame.ControlLimit) != nil {
		t.Fatal("ACK")
	}
	select {
	case err := <-finished:
		if err != nil {
			t.Fatal(err)
		}
	case <-time.After(time.Second):
		t.Fatal("exporter failed to terminate")
	}
}

func TestForgedIdentitySourceTimeAndRawSentinelCannotBecomeSafeResult(t *testing.T) {
	q := request()
	start := frame.Control{Identity: frame.Bound(q, "START", 2), Deadline: q.Deadline}
	now := time.Now()
	base := frame.Raw{Identity: frame.Bound(q, "RESULT", 3), Source: q.Resolver, Status: "received", Observed: now.UnixMilli(), Packet: []byte("RAW_SENTINEL")}
	for _, change := range []func(*frame.Raw){
		func(r *frame.Raw) {}, func(r *frame.Raw) { r.Kind = "START" }, func(r *frame.Raw) { r.Generation++ },
		func(r *frame.Raw) { r.Source = "169.254.169.254" }, func(r *frame.Raw) { r.Observed = q.Deadline + 1 },
		func(r *frame.Raw) { r.Observed = now.Add(time.Second).UnixMilli() },
		func(r *frame.Raw) { r.Status = "timeout" }, func(r *frame.Raw) { r.Packet = bytes.Repeat([]byte("RAW_SENTINEL"), 500) },
	} {
		r := base
		change(&r)
		result := project(q, start, r, now.Add(-time.Millisecond))
		if result.Status != "unsupported_response" || len(result.Answers) != 0 || len(result.Aliases) != 0 {
			t.Fatal("untrusted candidate escaped")
		}
		var sink bytes.Buffer
		if (&frame.Writer{Sink: &sink}).Write(result, frame.Limit) != nil || bytes.Contains(sink.Bytes(), []byte("RAW_SENTINEL")) {
			t.Fatal("raw material on safe pipe")
		}
	}
}

func TestMultipleCandidateFramesFailBeforeOrdinaryProjection(t *testing.T) {
	q := request()
	var parent, raw, safe bytes.Buffer
	w := frame.Writer{Sink: &parent}
	for _, value := range []any{q,
		frame.Control{Identity: frame.Bound(q, "START", 2), Deadline: q.Deadline},
		frame.Control{Identity: frame.Bound(q, "ACK", 4), Deadline: q.Deadline}} {
		if w.Write(value, frame.Limit) != nil {
			t.Fatal("parent framing")
		}
	}
	candidate := frame.Raw{Identity: frame.Bound(q, "RESULT", 3), Source: q.Resolver,
		Status: "timeout", Observed: time.Now().UnixMilli(), Packet: []byte{}}
	rawWriter := frame.Writer{Sink: &raw}
	if rawWriter.Write(candidate, frame.Limit) != nil {
		t.Fatal("raw framing")
	}
	if rawWriter.Write(candidate, frame.Limit) != nil {
		t.Fatal("raw framing")
	}
	if Run(&parent, &raw, &safe) != nil {
		t.Fatal("closed failure handshake")
	}
	r := frame.Reader{Source: &safe}
	var ready frame.Control
	var result frame.Result
	if r.Read(&ready, frame.ControlLimit) != nil || r.Read(&result, frame.Limit) != nil {
		t.Fatal("safe framing")
	}
	if result.Status != "ipc_failure" || len(result.Answers) != 0 || safe.Len() != 0 {
		t.Fatal("multiple frames admitted")
	}
}

func TestTimeoutAfterKernelDeadlineRetainsFixedFailureCategory(t *testing.T) {
	q := request()
	now := time.Now()
	start := frame.Control{Identity: frame.Bound(q, "START", 2), Deadline: now.Add(-time.Millisecond).UnixMilli()}
	raw := frame.Raw{Identity: frame.Bound(q, "RESULT", 3), Source: q.Resolver, Status: "timeout", Observed: now.UnixMilli(), Packet: []byte{}}
	result := project(q, start, raw, now.Add(-2*time.Second))
	if result.Status != "timeout" || result.Observed != 0 || result.Complete || len(result.Answers) != 0 {
		t.Fatal("deadline wake-up changed truthful timeout into another result")
	}
}
