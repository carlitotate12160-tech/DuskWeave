// Package export owns the trusted projection. Raw input has a distinct descriptor.
package export

import (
	"duskweave/boundary/internal/dns"
	"duskweave/boundary/internal/frame"
	"io"
	"time"
)

func Run(parent io.Reader, raw io.Reader, safe io.Writer) error {
	control, candidate := frame.Reader{Source: parent}, frame.Reader{Source: raw}
	out := frame.Writer{Sink: safe}
	var q frame.Request
	if control.Read(&q, frame.Limit) != nil || dns.Validate(q, time.Now()) != nil {
		return frame.ErrFrame
	}
	readyAt := time.Now()
	if out.Write(frame.Control{Identity: frame.Bound(q, "READY", 1), Deadline: q.Deadline}, frame.ControlLimit) != nil {
		return frame.ErrFrame
	}
	var start frame.Control
	if control.Read(&start, frame.ControlLimit) != nil || !frame.Matches(start.Identity, q, "START", 2) {
		return frame.ErrFrame
	}
	now := time.Now()
	if start.Deadline > q.Deadline || start.Deadline <= now.UnixMilli() || now.Sub(readyAt) >= time.Second {
		return frame.ErrFrame
	}
	var input frame.Raw
	result := failed(q, "ipc_failure")
	if candidate.Read(&input, frame.Limit) == nil {
		var extra [1]byte
		if n, err := raw.Read(extra[:]); n == 0 && err == io.EOF {
			result = project(q, start, input, readyAt)
		}
	}
	if out.Write(result, frame.Limit) != nil {
		return frame.ErrFrame
	}
	var ack frame.Control
	if control.Read(&ack, frame.ControlLimit) != nil || !frame.Matches(ack.Identity, q, "ACK", 4) {
		return frame.ErrFrame
	}
	return nil
}

func failed(q frame.Request, category string) frame.Result {
	return frame.Result{Identity: frame.Bound(q, "RESULT", 3), Status: category,
		Answers: []frame.Answer{}, Aliases: []string{}}
}

func project(q frame.Request, start frame.Control, raw frame.Raw, began time.Time) frame.Result {
	if !frame.Matches(raw.Identity, q, "RESULT", 3) || raw.Source != q.Resolver || raw.Observed < began.UnixMilli() || raw.Observed > time.Now().UnixMilli() || len(raw.Packet) > 4096 {
		return failed(q, "unsupported_response")
	}
	if raw.Status != "received" {
		if len(raw.Packet) != 0 {
			return failed(q, "unsupported_response")
		}
		switch raw.Status {
		case "timeout", "limit", "process_failure", "unsupported_response":
			return failed(q, raw.Status)
		default:
			return failed(q, "unsupported_response")
		}
	}
	if raw.Observed > start.Deadline {
		return failed(q, "unsupported_response")
	}
	p, err := dns.Decode(raw.Packet, q)
	if err != nil {
		return failed(q, "unsupported_response")
	}
	status := "completed"
	if !p.Complete {
		status = "limited"
	}
	return frame.Result{Identity: frame.Bound(q, "RESULT", 3), Status: status,
		Observed: raw.Observed, Rcode: p.Rcode, Truncated: p.Truncated, Complete: p.Complete,
		WithheldAliases: p.Withheld, Answers: p.Answers, Aliases: p.Aliases}
}
