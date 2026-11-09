package dns

import (
	"duskweave/boundary/internal/frame"
	"encoding/binary"
	"net/netip"
	"strings"
)

type record struct {
	name       string
	kind       uint16
	ttl        uint32
	start, end int
}
type Projection struct {
	Rcode               uint8
	Truncated, Complete bool
	Withheld            uint32
	Answers             []frame.Answer
	Aliases             []string
}

// Name decoding follows only prior pointers, with an independent finite step bound.
func decodeName(data []byte, start int) (string, int, error) {
	position, end, steps := start, -1, 0
	labels := []string{}
	for {
		steps++
		if position < 0 || position >= len(data) || steps > 128 {
			return "", 0, ErrDNS
		}
		n := int(data[position])
		if n&0xc0 == 0xc0 {
			if position+1 >= len(data) {
				return "", 0, ErrDNS
			}
			target := (n&0x3f)<<8 | int(data[position+1])
			if target < 12 || target >= position {
				return "", 0, ErrDNS
			}
			if end < 0 {
				end = position + 2
			}
			position = target
			continue
		}
		if n&0xc0 != 0 {
			return "", 0, ErrDNS
		}
		position++
		if n == 0 {
			if end < 0 {
				end = position
			}
			break
		}
		if n > 63 || position+n > len(data) {
			return "", 0, ErrDNS
		}
		label := strings.ToLower(string(data[position : position+n]))
		if !Name(label) || strings.Contains(label, ".") {
			return "", 0, ErrDNS
		}
		labels = append(labels, label)
		position += n
	}
	name := strings.Join(labels, ".")
	if name != "" && !Name(name) {
		return "", 0, ErrDNS
	}
	return name, end, nil
}
func records(data []byte, offset, count int) ([]record, error) {
	items := make([]record, 0, count)
	addresses, aliases := 0, 0
	for i := 0; i < count; i++ {
		name, end, err := decodeName(data, offset)
		if err != nil || end+10 > len(data) {
			return nil, ErrDNS
		}
		kind := binary.BigEndian.Uint16(data[end:])
		if binary.BigEndian.Uint16(data[end+2:]) != 1 {
			return nil, ErrDNS
		}
		size := int(binary.BigEndian.Uint16(data[end+8:]))
		item := record{name, kind, binary.BigEndian.Uint32(data[end+4:]), end + 10, end + 10 + size}
		if item.ttl > 86400 {
			return nil, ErrDNS
		}
		if item.end > len(data) {
			return nil, ErrDNS
		}
		if err := recordShape(data, item); err != nil {
			return nil, err
		}
		if kind == 1 {
			addresses++
		}
		if kind == 5 {
			aliases++
		}
		if addresses > 4 || aliases > 4 {
			return nil, ErrDNS
		}
		items = append(items, item)
		offset = item.end
	}
	if offset != len(data) {
		return nil, ErrDNS
	}
	return items, nil
}
func recordShape(data []byte, r record) error {
	switch r.kind {
	case 1:
		if r.end-r.start != 4 {
			return ErrDNS
		}
	case 2, 5:
		_, end, err := decodeName(data, r.start)
		if err != nil || end != r.end {
			return ErrDNS
		}
	case 6:
		_, end, err := decodeName(data, r.start)
		if err != nil {
			return ErrDNS
		}
		_, end, err = decodeName(data, end)
		if err != nil || end+20 != r.end {
			return ErrDNS
		}
	default:
		return ErrDNS
	}
	return nil
}
func Decode(data []byte, q frame.Request) (Projection, error) {
	empty := Projection{}
	if len(data) < 12 || len(data) > 4096 || binary.BigEndian.Uint16(data) != q.QueryID {
		return empty, ErrDNS
	}
	flags := binary.BigEndian.Uint16(data[2:])
	if flags&15 > 5 {
		return empty, ErrDNS
	}
	if flags&0x8000 == 0 || flags&0x7800 != 0 || flags&0x0040 != 0 || binary.BigEndian.Uint16(data[4:]) != 1 {
		return empty, ErrDNS
	}
	name, end, err := decodeName(data, 12)
	if err != nil || name != q.Name || end+4 > len(data) || binary.BigEndian.Uint16(data[end:]) != 1 || binary.BigEndian.Uint16(data[end+2:]) != 1 {
		return empty, ErrDNS
	}
	answers := int(binary.BigEndian.Uint16(data[6:]))
	total := answers + int(binary.BigEndian.Uint16(data[8:])) + int(binary.BigEndian.Uint16(data[10:]))
	if total > 32 {
		return empty, ErrDNS
	}
	p := Projection{Rcode: uint8(flags & 15), Truncated: flags&0x0200 != 0, Complete: true,
		Answers: []frame.Answer{}, Aliases: []string{}}
	if p.Truncated {
		p.Complete = false
		return p, nil
	}
	rr, err := records(data, end+4, total)
	if err != nil {
		return empty, err
	}
	if p.Rcode != 0 {
		return p, nil
	}
	return project(data, rr[:answers], q, p)
}
func project(data []byte, rr []record, q frame.Request, p Projection) (Projection, error) {
	name := q.Name
	seen := map[string]bool{name: true}
	lowest := ^uint32(0)
	for {
		var alias *record
		for i := range rr {
			if rr[i].name == name && rr[i].kind == 5 {
				if alias != nil {
					return Projection{}, ErrDNS
				}
				alias = &rr[i]
			}
		}
		if alias == nil {
			break
		}
		for _, r := range rr {
			if r.name == name && r.kind == 1 {
				return Projection{}, ErrDNS
			}
		}
		if len(seen) > 4 {
			return Projection{}, ErrDNS
		}
		next, _, err := decodeName(data, alias.start)
		if err != nil || seen[next] {
			return Projection{}, ErrDNS
		}
		seen[next] = true
		if alias.ttl < lowest {
			lowest = alias.ttl
		}
		if !allowed(next, q.AllowedNames) {
			p.Withheld++
			p.Complete = false
			return p, nil
		}
		p.Aliases = append(p.Aliases, next)
		name = next
	}
	addressIndex := map[string]int{}
	for _, r := range rr {
		if r.kind != 1 || r.name != name {
			continue
		}
		ip := netip.AddrFrom4([4]byte(data[r.start:r.end]))
		ttl := r.ttl
		if lowest < ttl {
			ttl = lowest
		}
		if index, exists := addressIndex[ip.String()]; exists {
			if ttl < p.Answers[index].TTL {
				p.Answers[index].TTL = ttl
			}
			continue
		}
		addressIndex[ip.String()] = len(p.Answers)
		p.Answers = append(p.Answers, frame.Answer{IPv4: ip.String(), TTL: ttl})
	}
	return p, nil
}
