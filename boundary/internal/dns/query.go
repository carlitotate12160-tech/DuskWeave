// Package dns issues only an explicitly admitted DNS question, never a system lookup.
package dns

import (
	"duskweave/boundary/internal/frame"
	"encoding/binary"
	"errors"
	"net/netip"
	"strings"
	"time"
)

var ErrDNS = errors.New("unsupported_response")

func Name(name string) bool {
	if len(name) == 0 || len(name) > 253 {
		return false
	}
	for _, label := range strings.Split(name, ".") {
		if len(label) == 0 || len(label) > 63 || label[0] == '-' || label[len(label)-1] == '-' {
			return false
		}
		for _, ch := range []byte(label) {
			if !(ch >= 'a' && ch <= 'z' || ch >= '0' && ch <= '9' || ch == '-') {
				return false
			}
		}
	}
	return true
}
func uuid(s string) bool {
	if len(s) != 36 || s == "00000000-0000-0000-0000-000000000000" {
		return false
	}
	for i, c := range []byte(s) {
		if i == 8 || i == 13 || i == 18 || i == 23 {
			if c != '-' {
				return false
			}
			continue
		}
		if !(c >= '0' && c <= '9' || c >= 'a' && c <= 'f') {
			return false
		}
	}
	return true
}
func Validate(q frame.Request, now time.Time) error {
	if !frame.Matches(q.Identity, q, "REQUEST", 0) || q.Mode != "live" || q.Profile != "dns-routing-v2" || q.Transport != "udp" {
		return ErrDNS
	}
	for _, id := range []string{q.Engagement, q.Campaign, q.Decision, q.Attempt, q.Qualification} {
		if !uuid(id) {
			return ErrDNS
		}
	}
	if q.Generation == 0 || q.Duration == 0 || q.Duration > 2000 || q.Deadline <= now.UnixMilli() || !Name(q.Name) {
		return ErrDNS
	}
	if _, err := netip.ParseAddr(q.Name); err == nil {
		return ErrDNS
	}
	if len(q.Release) != 64 {
		return ErrDNS
	}
	for _, b := range []byte(q.Release) {
		if !(b >= '0' && b <= '9' || b >= 'a' && b <= 'f') {
			return ErrDNS
		}
	}
	if len(q.AllowedNames) == 0 || len(q.AllowedNames) > 8 || !allowed(q.Name, q.AllowedNames) {
		return ErrDNS
	}
	for _, name := range q.AllowedNames {
		if !Name(name) {
			return ErrDNS
		}
	}
	ip, err := netip.ParseAddr(q.Resolver)
	if err != nil || !ip.Is4() || ip.IsUnspecified() || ip.IsMulticast() || ip.IsLoopback() {
		return ErrDNS
	}
	if q.RunProfile == "autonomous-dns-genai-lab-v1" {
		if q.Resolver != "192.0.2.53" || !strings.HasSuffix(q.Name, ".duskweave.test") {
			return ErrDNS
		}
	} else if q.RunProfile != "autonomous-dns-genai-live-v1" {
		return ErrDNS
	}
	return nil
}
func allowed(name string, names []string) bool {
	for _, n := range names {
		if name == n {
			return true
		}
	}
	return false
}
func Query(id uint16, name string) ([]byte, error) {
	if !Name(name) {
		return nil, ErrDNS
	}
	q := make([]byte, 12, 12+len(name)+6)
	binary.BigEndian.PutUint16(q[:2], id)
	q[2], q[5] = 1, 1
	for _, label := range strings.Split(name, ".") {
		q = append(q, byte(len(label)))
		q = append(q, label...)
	}
	return append(q, 0, 0, 1, 0, 1), nil
}

// TCP framing is implemented for shared conformance; this run offers only UDP.
func TCPPacket(packet []byte) ([]byte, error) {
	if len(packet) < 12 || len(packet) > 4096 {
		return nil, ErrDNS
	}
	out := make([]byte, len(packet)+2)
	binary.BigEndian.PutUint16(out, uint16(len(packet)))
	copy(out[2:], packet)
	return out, nil
}
func TCPDecode(data []byte) ([]byte, error) {
	if len(data) < 2 {
		return nil, ErrDNS
	}
	n := int(binary.BigEndian.Uint16(data))
	if n < 12 || n > 4096 || len(data) != n+2 {
		return nil, ErrDNS
	}
	return data[2:], nil
}
