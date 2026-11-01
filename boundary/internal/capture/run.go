// Package capture owns the one wire attempt. Its output is a private raw channel.
package capture

import (
	"duskweave/boundary/internal/dns"
	"duskweave/boundary/internal/frame"
	"io"
	"net"
	"net/netip"
	"time"
)

func Run(control io.Reader, raw io.Writer) error {
	r, w := frame.Reader{Source: control}, frame.Writer{Sink: raw}
	var q frame.Request
	if r.Read(&q, frame.Limit) != nil || dns.Validate(q, time.Now()) != nil {
		return frame.ErrFrame
	}
	readyAt := time.Now()
	ready := frame.Control{Identity: frame.Bound(q, "READY", 1), Deadline: q.Deadline}
	if w.Write(ready, frame.ControlLimit) != nil {
		return frame.ErrFrame
	}
	var start frame.Control
	if r.Read(&start, frame.ControlLimit) != nil || !frame.Matches(start.Identity, q, "START", 2) {
		return frame.ErrFrame
	}
	now := time.Now()
	if start.Deadline > q.Deadline || start.Deadline <= now.UnixMilli() || now.Sub(readyAt) >= time.Second {
		return frame.ErrFrame
	}
	result := exchange(q, time.UnixMilli(start.Deadline))
	if w.Write(result, frame.Limit) != nil {
		return frame.ErrFrame
	}
	// Raw acquisition ends immediately. The independently trusted exporter holds
	// the pair's ACK boundary until the ordinary owner durably records the result.
	return nil
}

func exchange(q frame.Request, end time.Time) frame.Raw {
	result := frame.Raw{Identity: frame.Bound(q, "RESULT", 3), Source: q.Resolver,
		Status: "process_failure", Observed: time.Now().UnixMilli(), Packet: []byte{}}
	query, err := dns.Query(q.QueryID, q.Name)
	if err != nil {
		return result
	}
	ip, err := netip.ParseAddr(q.Resolver)
	if err != nil {
		return result
	}
	conn, err := net.DialUDP("udp4", nil, net.UDPAddrFromAddrPort(netip.AddrPortFrom(ip, 53)))
	if err != nil {
		return result
	}
	defer conn.Close()
	deadline := time.Now().Add(time.Duration(q.Duration) * time.Millisecond)
	if end.Before(deadline) {
		deadline = end
	}
	if conn.SetDeadline(deadline) != nil || !time.Now().Before(deadline) {
		return result
	}
	// A single write is the only transmission. No retry, fallback or name lookup.
	n, err := conn.Write(query)
	if err != nil || n != len(query) {
		return result
	}
	packet := make([]byte, 4097)
	n, source, err := conn.ReadFromUDPAddrPort(packet)
	result.Observed = time.Now().UnixMilli()
	if err != nil {
		if timeout, ok := err.(net.Error); ok && timeout.Timeout() {
			result.Status = "timeout"
		}
		return result
	}
	if source.Addr() != ip || source.Port() != 53 {
		result.Status = "unsupported_response"
		return result
	}
	if n > 4096 {
		result.Status = "limit"
		return result
	}
	result.Status, result.Packet = "received", packet[:n]
	return result
}
