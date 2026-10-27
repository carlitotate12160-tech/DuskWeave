// capture is the ADR-015 raw-facing worker. It reads its issued REQUEST and
// one-use START over stdin, performs exactly one bounded UDP exchange against
// the single admitted endpoint, and emits its readiness frame plus framed raw
// wire bytes on stdout. That stdout is forwarded only into the isolated
// exporter; capture holds no ordinary result descriptor. stderr is discarded
// unlogged by the launcher; exit codes carry only the fixed category.
package main

import (
	"bufio"
	"net"
	"os"
	"time"

	"duskweave/boundary/dnswire"
	"duskweave/boundary/frame"
)

const (
	exitOK       = 0
	exitRefused  = 1
	exitPhase    = 2
	exitCanceled = 3
	exitNet      = 4
)

func main() {
	os.Exit(run(bufio.NewReader(os.Stdin), bufio.NewWriter(os.Stdout)))
}

func run(in *bufio.Reader, out *bufio.Writer) int {
	req, code := readRequest(in)
	if code != exitOK {
		return code
	}
	if err := frame.WriteJSON(out, &frame.CaptureReady{
		Version:   frame.RequestVersion,
		Phase:     frame.PhaseReady,
		Kind:      frame.KindCapture,
		AttemptID: req.AttemptID,
	}); err != nil || out.Flush() != nil {
		return exitPhase
	}
	if code := awaitStart(in, req.AttemptID); code != exitOK {
		return code
	}
	return exchange(out, req)
}

func readRequest(in *bufio.Reader) (*frame.CaptureRequest, int) {
	var req frame.CaptureRequest
	if frame.ReadJSON(in, &req) != nil {
		return nil, exitRefused
	}
	ok := req.Version == frame.RequestVersion && req.Kind == frame.KindCapture &&
		req.Effect == frame.EffectDNSUDP && req.Qtype == "A" &&
		req.AttemptID != "" && req.SessionID != "" &&
		req.MaxWire > 0 && req.MaxWire <= 512 && req.DeadlineMs > 0 &&
		validEndpoint(&req.Endpoint)
	if !ok {
		return nil, exitRefused
	}
	return &req, exitOK
}

func validEndpoint(ep *frame.Endpoint) bool {
	return ep.Transport == "udp" && net.ParseIP(ep.IPv4) != nil && ep.Port > 0
}

func awaitStart(in *bufio.Reader, attempt string) int {
	var msg struct {
		Version   string `json:"version"`
		Phase     string `json:"phase"`
		AttemptID string `json:"attempt_id"`
	}
	if frame.ReadJSON(in, &msg) != nil || msg.AttemptID != attempt ||
		msg.Version != frame.RequestVersion {
		return exitPhase
	}
	if msg.Phase == frame.PhaseCancel {
		return exitCanceled
	}
	if msg.Phase != frame.PhaseStart {
		return exitPhase
	}
	return exitOK
}

func exchange(out *bufio.Writer, req *frame.CaptureRequest) int {
	defer out.Flush()
	remaining := time.Until(time.UnixMilli(req.DeadlineMs))
	if remaining <= 0 {
		_ = frame.WriteSentinel(out)
		return exitNet
	}
	wire, err := dnswire.BuildQuery(req.TxID, req.Qname)
	if err != nil {
		_ = frame.WriteSentinel(out)
		return exitRefused
	}
	code := sendRecv(req, wire, remaining, out)
	_ = frame.WriteSentinel(out)
	return code
}

func sendRecv(req *frame.CaptureRequest, wire []byte, budget time.Duration, out *bufio.Writer) int {
	conn, err := net.DialUDP("udp", nil, &net.UDPAddr{
		IP:   net.ParseIP(req.Endpoint.IPv4),
		Port: int(req.Endpoint.Port),
	})
	if err != nil {
		return exitNet
	}
	defer conn.Close()
	_ = conn.SetDeadline(time.Now().Add(budget))
	if _, err := conn.Write(wire); err != nil {
		return exitNet
	}
	buf := make([]byte, req.MaxWire)
	n, err := conn.Read(buf)
	if err != nil {
		return exitNet
	}
	if frame.WriteRaw(out, buf[:n]) != nil {
		return exitPhase
	}
	return exitOK
}
