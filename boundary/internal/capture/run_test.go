package capture

import (
	"bytes"
	"duskweave/boundary/internal/frame"
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
		Deadline: time.Now().Add(2 * time.Second).UnixMilli(), Duration: 2000}
}

func TestNoStartOrForgedControlNeverEntersAcquisition(t *testing.T) {
	for _, kind := range []string{"CANCEL", "ACK", "RESULT", "READY", ""} {
		var input, output bytes.Buffer
		q := request()
		w := frame.Writer{Sink: &input}
		if w.Write(q, frame.Limit) != nil {
			t.Fatal("request framing")
		}
		if kind != "" {
			if w.Write(frame.Control{Identity: frame.Bound(q, kind, 2), Deadline: q.Deadline}, frame.ControlLimit) != nil {
				t.Fatal("control framing")
			}
		}
		if Run(&input, &output) == nil {
			t.Fatal("acquisition admitted without START")
		}
		r := frame.Reader{Source: &output}
		var ready frame.Control
		if r.Read(&ready, frame.ControlLimit) != nil || !frame.Matches(ready.Identity, q, "READY", 1) {
			t.Fatal("missing closed READY")
		}
		if output.Len() != 0 {
			t.Fatal("raw result before parent START")
		}
	}
}

func TestInvalidRequestProducesNoRawOutput(t *testing.T) {
	q := request()
	q.Resolver = "127.0.0.1"
	var input, output bytes.Buffer
	if (&frame.Writer{Sink: &input}).Write(q, frame.Limit) != nil {
		t.Fatal("request framing")
	}
	if Run(&input, &output) == nil || output.Len() != 0 {
		t.Fatal("invalid endpoint reached READY")
	}
}
