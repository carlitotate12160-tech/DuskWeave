package dnswire

import (
	"encoding/binary"
	"testing"
)

// craftResponse builds a one-question, one-answer A record response for
// qname under txid, with the answer owner compressed to the question name.
func craftResponse(txid uint16, qname string, ip [4]byte, ttl uint32) []byte {
	q, _ := BuildQuery(txid, qname)
	question := q[12:]
	out := make([]byte, 0, 64)
	out = binary.BigEndian.AppendUint16(out, txid)
	out = binary.BigEndian.AppendUint16(out, 0x8180) // QR+RD+RA, rcode 0
	out = binary.BigEndian.AppendUint16(out, 1)      // QD
	out = binary.BigEndian.AppendUint16(out, 1)      // AN
	out = append(out, 0, 0, 0, 0)
	out = append(out, question...)
	out = append(out, 0xc0, 0x0c) // ptr to qname at 12
	out = binary.BigEndian.AppendUint16(out, 1)     // A
	out = binary.BigEndian.AppendUint16(out, 1)     // IN
	out = binary.BigEndian.AppendUint32(out, ttl)
	out = binary.BigEndian.AppendUint16(out, 4)     // rdlen
	out = append(out, ip[:]...)
	return out
}

func TestRoundTrip(t *testing.T) {
	raw := craftResponse(0xbeef, "probe.example.invalid", [4]byte{192, 0, 2, 80}, 60)
	got, err := ParseResponse(raw, 0xbeef, "probe.example.invalid")
	if err != nil {
		t.Fatalf("parse: %v", err)
	}
	if len(got.Answers) != 1 || got.Answers[0].IPv4.String() != "192.0.2.80" ||
		got.Answers[0].TTL != 60 || got.Rcode != 0 || got.Truncated {
		t.Fatalf("decoded %+v", got)
	}
}

func TestIDMismatch(t *testing.T) {
	raw := craftResponse(0xbeef, "probe.example.invalid", [4]byte{192, 0, 2, 80}, 60)
	if _, err := ParseResponse(raw, 0x1234, "probe.example.invalid"); err != ErrIDMismatch {
		t.Fatalf("got %v", err)
	}
}

func TestQNameMismatch(t *testing.T) {
	raw := craftResponse(0xbeef, "probe.example.invalid", [4]byte{192, 0, 2, 80}, 60)
	if _, err := ParseResponse(raw, 0xbeef, "other.example.invalid"); err == nil {
		t.Fatal("expected rejection")
	}
}

func TestQueryFlagRejected(t *testing.T) {
	q, _ := BuildQuery(0xbeef, "probe.example.invalid")
	if _, err := ParseResponse(q, 0xbeef, "probe.example.invalid"); err != ErrNotResp {
		t.Fatalf("got %v", err)
	}
}

func TestPointerLoopBounded(t *testing.T) {
	raw := craftResponse(0xbeef, "probe.example.invalid", [4]byte{192, 0, 2, 80}, 60)
	qd := binary.BigEndian.Uint16(raw[4:6])
	off := 12
	for i := 0; i < int(qd); i++ {
		_, next, err := decodeName(raw, off)
		if err != nil {
			t.Fatal(err)
		}
		off = next + 4
	}
	// Corrupt the answer owner into a self-pointer.
	raw[off] = 0xc0
	raw[off+1] = byte(off)
	if _, err := ParseResponse(raw, 0xbeef, "probe.example.invalid"); err == nil {
		t.Fatal("expected pointer-loop rejection")
	}
}

func TestTruncatedInput(t *testing.T) {
	raw := craftResponse(0xbeef, "probe.example.invalid", [4]byte{192, 0, 2, 80}, 60)
	if _, err := ParseResponse(raw[:len(raw)-2], 0xbeef, "probe.example.invalid"); err == nil {
		t.Fatal("expected rejection")
	}
}

func TestBuildQueryRejects(t *testing.T) {
	for _, name := range []string{"", "a..b", "bad name.example", string(make([]byte, 300))} {
		if _, err := BuildQuery(1, name); err == nil {
			t.Fatalf("accepted %q", name)
		}
	}
}
