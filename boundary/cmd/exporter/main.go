// exporter is the ADR-015 trusted egress validator. Its stdin carries the
// independently issued REQUEST frame, then the capture worker's readiness
// frame, then binary-framed raw bytes terminated by a zero-length sentinel,
// then the launcher's ACK frame. It validates the bounded candidate bytes
// against the issued context and emits the sole ordinary RESULT on stdout.
// It holds no network namespace connectivity; stderr discards unlogged.
package main

import (
	"bufio"
	"bytes"
	"io"
	"os"
	"time"

	"duskweave/boundary/dnswire"
	"duskweave/boundary/frame"
)

const (
	exitOK      = 0
	exitRefused = 1
	exitPhase   = 2
)

func main() {
	os.Exit(run(bufio.NewReader(os.Stdin), bufio.NewWriter(os.Stdout)))
}

func run(in *bufio.Reader, out *bufio.Writer) int {
	var req frame.ExporterRequest
	if frame.ReadJSON(in, &req) != nil || !validRequest(&req) {
		return exitRefused
	}
	if err := frame.WriteJSON(out, &frame.Ready{
		Version:   frame.RequestVersion,
		Phase:     frame.PhaseReady,
		Kind:      frame.KindExporter,
		AttemptID: req.AttemptID,
	}); err != nil || out.Flush() != nil {
		return exitPhase
	}
	started := time.Now()
	res := evaluate(in, &req)
	res.ElapsedMs = time.Since(started).Milliseconds()
	if frame.WriteJSON(out, &res) != nil || out.Flush() != nil {
		return exitPhase
	}
	return awaitAck(in, req.AttemptID)
}

func validRequest(req *frame.ExporterRequest) bool {
	return req.Version == frame.RequestVersion && req.Kind == frame.KindExporter &&
		req.Effect == frame.EffectDNSUDP && req.Qtype == "A" &&
		req.AttemptID != "" && req.SessionID != "" &&
		req.MaxRaw > 0 && req.MaxRaw <= 4096 && req.DeadlineMs > 0
}

// evaluate consumes the capture readiness frame, the raw channel and decides
// the closed disposition set; the raw bytes never leave this process.
func evaluate(in *bufio.Reader, req *frame.ExporterRequest) frame.Result {
	res := frame.Result{
		Version:   frame.ResultVersion,
		Phase:     frame.PhaseResult,
		AttemptID: req.AttemptID,
		Answers:   []frame.Answer{},
	}
	var ready frame.CaptureReady
	if frame.ReadJSON(in, &ready) != nil || ready.Phase != frame.PhaseReady ||
		ready.Kind != frame.KindCapture || ready.AttemptID != req.AttemptID {
		res.Disposition = frame.DispCancelled
		return res
	}
	raw, truncated, rcode := readRaw(in, req.MaxRaw)
	res.Counters = frame.Counters{RawBytes: uint32(raw.Len()), RawTruncated: truncated}
	if rcode != exitOK {
		res.Disposition = code2disp(rcode)
		return res
	}
	if raw.Len() == 0 {
		res.Disposition = frame.DispNoData
		return res
	}
	res.Disposition = classify(raw.Bytes(), req, &res)
	return res
}

// awaitAck reads the launcher's terminal control. A closed channel after
// RESULT is accepted as an implicit acknowledgement.
func awaitAck(in *bufio.Reader, attempt string) int {
	var ack frame.Ack
	err := frame.ReadJSON(in, &ack)
	if err == io.EOF || err == io.ErrUnexpectedEOF {
		return exitOK
	}
	if err != nil || ack.AttemptID != attempt ||
		ack.Version != frame.RequestVersion || ack.Phase != frame.PhaseAck {
		return exitPhase
	}
	return exitOK
}

func code2disp(code int) string {
	if code == 1 {
		return frame.DispOversized
	}
	return frame.DispMalformed
}

// readRaw consumes binary chunks until the sentinel. Return code 0 is clean
// termination, 1 oversize, 2 malformed transport.
func readRaw(in *bufio.Reader, max uint32) (*bytes.Buffer, bool, int) {
	var raw bytes.Buffer
	truncated := false
	for {
		done, err := frame.ReadRawChunk(in, &raw)
		if err == frame.ErrOversize {
			return &raw, truncated, 1
		}
		if err == io.EOF || err == io.ErrUnexpectedEOF {
			if raw.Len() == 0 {
				return &raw, truncated, 0
			}
			return &raw, truncated, 2
		}
		if err != nil {
			return &raw, truncated, 2
		}
		if done {
			return &raw, truncated, 0
		}
		if uint32(raw.Len()) > max {
			truncated = true
			raw.Truncate(int(max))
		}
	}
}

func classify(raw []byte, req *frame.ExporterRequest, res *frame.Result) string {
	parsed, err := dnswire.ParseResponse(raw, req.TxID, req.Qname)
	if err != nil {
		return frame.DispMalformed
	}
	res.Truncated = parsed.Truncated
	for _, a := range parsed.Answers {
		res.Answers = append(res.Answers, frame.Answer{IPv4: a.IPv4.String(), TTL: a.TTL})
	}
	res.Rcode = rcodeName(parsed.Rcode)
	if parsed.Truncated {
		return frame.DispTruncated
	}
	return mapRcode(parsed.Rcode, len(parsed.Answers))
}

func mapRcode(rcode int, answers int) string {
	switch rcode {
	case 0:
		if answers == 0 {
			return frame.DispNoAnswers
		}
		return frame.DispQualified
	case 3:
		return frame.DispNXDomain
	case 2:
		return frame.DispServFail
	case 5:
		return frame.DispRefused
	default:
		return frame.DispRcodeOther
	}
}

func rcodeName(rcode int) string {
	switch rcode {
	case 0:
		return "noerror"
	case 1:
		return "formerr"
	case 2:
		return "servfail"
	case 3:
		return "nxdomain"
	case 4:
		return "notimp"
	case 5:
		return "refused"
	default:
		return "other"
	}
}
