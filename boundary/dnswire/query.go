// Package dnswire builds and parses bounded DNS/UDP wire messages for the
// M1 acquisition boundary. Only the classic <=512-byte UDP profile is
// supported; EDNS and every other qtype remain out of scope.
package dnswire

import (
	"encoding/binary"
	"errors"
	"strings"
)

const (
	headerLen   = 12
	maxNameLen  = 253
	maxLabelLen = 63
	maxQueryLen = 512

	rcodeNoError  = 0
	rcodeFormErr  = 1
	rcodeServFail = 2
	rcodeNXDomain = 3
	rcodeNotImp   = 4
	rcodeRefused  = 5
)

var (
	ErrBadName    = errors.New("dns_bad_name")
	ErrOversize   = errors.New("dns_oversize")
	ErrMalformed  = errors.New("dns_malformed")
	ErrIDMismatch = errors.New("dns_id_mismatch")
	ErrNotResp    = errors.New("dns_not_response")
)

// BuildQuery encodes an A/IN query for qname under txid.
func BuildQuery(txid uint16, qname string) ([]byte, error) {
	name, err := encodeName(qname)
	if err != nil {
		return nil, err
	}
	buf := make([]byte, 0, headerLen+len(name)+4)
	buf = binary.BigEndian.AppendUint16(buf, txid)
	buf = binary.BigEndian.AppendUint16(buf, 0x0100) // RD
	buf = binary.BigEndian.AppendUint16(buf, 1)      // QDCOUNT
	buf = append(buf, 0, 0, 0, 0, 0, 0)              // AN/NS/AR
	buf = append(buf, name...)
	buf = binary.BigEndian.AppendUint16(buf, 1) // A
	buf = binary.BigEndian.AppendUint16(buf, 1) // IN
	if len(buf) > maxQueryLen {
		return nil, ErrOversize
	}
	return buf, nil
}

func encodeName(qname string) ([]byte, error) {
	if len(qname) == 0 || len(qname) > maxNameLen {
		return nil, ErrBadName
	}
	var out []byte
	for _, label := range strings.Split(qname, ".") {
		if len(label) == 0 || len(label) > maxLabelLen {
			return nil, ErrBadName
		}
		for i := 0; i < len(label); i++ {
			c := label[i]
			if !isNameByte(c) {
				return nil, ErrBadName
			}
		}
		out = append(out, byte(len(label)))
		out = append(out, label...)
	}
	return append(out, 0), nil
}

func isNameByte(c byte) bool {
	return c >= 'a' && c <= 'z' || c >= 'A' && c <= 'Z' ||
		c >= '0' && c <= '9' || c == '-' || c == '_'
}
