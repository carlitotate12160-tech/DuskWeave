package dnswire

import (
	"encoding/binary"
	"net/netip"
)

// ParsedResponse is the exporter's validated view of one wire reply.
type ParsedResponse struct {
	Rcode     int
	Truncated bool
	Answers   []Answer
}

type Answer struct {
	IPv4 netip.Addr
	TTL  uint32
}

const maxAnswers = 4
const maxPtrHops = 8

// ParseResponse validates a bounded wire reply against the issued attempt:
// response flag, opcode, transaction id and echoed question must match, and
// every structural bound is checked before allocation.
func ParseResponse(raw []byte, txid uint16, qname string) (*ParsedResponse, error) {
	if len(raw) < headerLen || len(raw) > maxQueryLen {
		return nil, ErrMalformed
	}
	if binary.BigEndian.Uint16(raw[0:2]) != txid {
		return nil, ErrIDMismatch
	}
	flags := binary.BigEndian.Uint16(raw[2:4])
	if flags&0x8000 == 0 || flags&0x7800 != 0 {
		return nil, ErrNotResp
	}
	resp := &ParsedResponse{
		Rcode:     int(flags & 0x000f),
		Truncated: flags&0x0200 != 0,
	}
	qd := int(binary.BigEndian.Uint16(raw[4:6]))
	an := int(binary.BigEndian.Uint16(raw[6:8]))
	if qd != 1 || an > maxAnswers {
		return nil, ErrMalformed
	}
	off := headerLen
	qn, next, err := decodeName(raw, off)
	if err != nil {
		return nil, err
	}
	if !equalName(qn, qname) {
		return nil, ErrMalformed
	}
	off = next + 4 // qtype + qclass
	if off > len(raw) {
		return nil, ErrMalformed
	}
	for i := 0; i < an; i++ {
		var a Answer
		a, off, err = parseAnswer(raw, off)
		if err != nil {
			return nil, err
		}
		resp.Answers = append(resp.Answers, a)
	}
	return resp, nil
}

// parseAnswer skips the owner name then reads a fixed RR head; only A/IN
// records with a four-byte rdata are admitted.
func parseAnswer(raw []byte, off int) (Answer, int, error) {
	var a Answer
	_, next, err := decodeName(raw, off)
	if err != nil {
		return a, 0, err
	}
	if next+10 > len(raw) {
		return a, 0, ErrMalformed
	}
	rtype := binary.BigEndian.Uint16(raw[next : next+2])
	class := binary.BigEndian.Uint16(raw[next+2 : next+4])
	a.TTL = binary.BigEndian.Uint32(raw[next+4 : next+8])
	rdlen := int(binary.BigEndian.Uint16(raw[next+8 : next+10]))
	data := next + 10
	if data+rdlen > len(raw) {
		return a, 0, ErrMalformed
	}
	if rtype != 1 || class != 1 || rdlen != 4 {
		return a, 0, ErrMalformed
	}
	a.IPv4 = netip.AddrFrom4([4]byte{raw[data], raw[data+1], raw[data+2], raw[data+3]})
	return a, data + rdlen, nil
}

// decodeName reads a possibly-compressed name at off. It returns the decoded
// dotted name and the offset just past the non-compressed part at this level;
// pointer hops are counted and bounded.
func decodeName(raw []byte, off int) (string, int, error) {
	var labels []string
	hops := 0
	next := -1
	for {
		if off >= len(raw) {
			return "", 0, ErrMalformed
		}
		l := int(raw[off])
		if l == 0 {
			if next < 0 {
				next = off + 1
			}
			break
		}
		if l&0xc0 == 0xc0 {
			if off+1 >= len(raw) {
				return "", 0, ErrMalformed
			}
			hops++
			if hops > maxPtrHops {
				return "", 0, ErrMalformed
			}
			if next < 0 {
				next = off + 2
			}
			off = ((l & 0x3f) << 8) | int(raw[off+1])
			continue
		}
		if l > maxLabelLen || off+1+l > len(raw) {
			return "", 0, ErrMalformed
		}
		labels = append(labels, string(raw[off+1:off+1+l]))
		off += 1 + l
		if len(labels) > maxNameLen/2 {
			return "", 0, ErrMalformed
		}
	}
	name := ""
	for i, l := range labels {
		if i > 0 {
			name += "."
		}
		name += l
	}
	if len(name) > maxNameLen {
		return "", 0, ErrMalformed
	}
	return name, next, nil
}

func equalName(wire, want string) bool {
	if len(wire) != len(want) {
		return false
	}
	for i := 0; i < len(wire); i++ {
		if lower(wire[i]) != lower(want[i]) {
			return false
		}
	}
	return true
}

func lower(c byte) byte {
	if c >= 'A' && c <= 'Z' {
		return c + ('a' - 'A')
	}
	return c
}
