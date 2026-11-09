package frame

import (
	"bytes"
	"encoding/binary"
	"strings"
	"testing"
)

func encoded(text string) []byte {
	header := make([]byte, 4)
	binary.BigEndian.PutUint32(header, uint32(len(text)))
	return append(header, text...)
}

func TestStrictAdmission(t *testing.T) {
	for _, body := range []string{
		`{"schema":"a","schema":"b"}`, `{"unknown":"RAW_SENTINEL"}`,
		`{"schema":"a"} {"schema":"b"}`, `null`, `[]`,
		`{"sequence":4294967296}`, `{"schema":"` + string([]byte{0xff}) + `"}`,
		`{"schema":"a","nested":` + strings.Repeat(`[`, 8) + `0` + strings.Repeat(`]`, 8) + `}`,
		`{"schema":"a","entries":[` + strings.Repeat(`0,`, 64) + `0]}`,
	} {
		var control Control
		r := Reader{Source: bytes.NewReader(encoded(body))}
		if r.Read(&control, ControlLimit) == nil {
			t.Fatalf("invalid framing accepted")
		}
	}
	for _, data := range [][]byte{{}, {0, 0, 0}, {255, 255, 255, 255}, {0, 0, 0, 3, '{'}} {
		var value Control
		if (&Reader{Source: bytes.NewReader(data)}).Read(&value, ControlLimit) == nil {
			t.Fatal("invalid length or EOF accepted")
		}
	}
}

type shortWriter struct{ bytes.Buffer }

func (s *shortWriter) Write(data []byte) (int, error) { return s.Buffer.Write(data[:1]) }

func TestPartialWritesAndFiniteFrames(t *testing.T) {
	var sink shortWriter
	w := Writer{Sink: &sink}
	q := Control{Identity: Identity{Schema: Schema, Kind: "READY", Attempt: "safe", Generation: 1, Sequence: 1}, Deadline: 1}
	for i := 0; i < MaxFrames; i++ {
		if w.Write(q, ControlLimit) != nil {
			t.Fatal("partial write failed")
		}
	}
	if w.Write(q, ControlLimit) == nil {
		t.Fatal("excess frame accepted")
	}
	r := Reader{Source: bytes.NewReader(sink.Bytes())}
	for i := 0; i < MaxFrames; i++ {
		var got Control
		if r.Read(&got, ControlLimit) != nil || got != q {
			t.Fatal("round trip failed")
		}
	}
	var got Control
	if r.Read(&got, ControlLimit) == nil {
		t.Fatal("reader frame bound absent")
	}
}

func TestExactJsonNamesAtEveryLevel(t *testing.T) {
	base := `{"schema":"m1_dns_ipc_v1","kind":"RESULT","attempt":"a","session_generation":1,"sequence":1,"status":"completed","observed_at_ms":1,"rcode":0,"truncated":false,"complete":true,"withheld_aliases":0,"answers":[{"ipv4":"192.0.2.53","ttl_seconds":60}],"aliases":[]}`
	frame := func(body string) []byte {
		var out [4]byte
		binary.BigEndian.PutUint32(out[:], uint32(len(body)))
		return append(out[:], []byte(body)...)
	}
	var good Result
	if (&Reader{Source: bytes.NewReader(frame(base))}).Read(&good, Limit) != nil {
		t.Fatal("canonical result rejected")
	}
	for _, mutant := range []string{
		`{"schema":"m1_dns_ipc_v1","kind":"RESULT","attempt":"a","session_generation":1,"sequence":1,"Status":"completed","observed_at_ms":1,"rcode":0,"truncated":false,"complete":true,"withheld_aliases":0,"answers":[{"ipv4":"192.0.2.53","ttl_seconds":60}],"aliases":[]}`,
		`{"schema":"m1_dns_ipc_v1","kind":"RESULT","attempt":"a","session_generation":1,"sequence":1,"status":"completed","observed_at_ms":1,"rcode":0,"truncated":false,"complete":true,"withheld_aliases":0,"answers":[{"IPv4":"192.0.2.53","ttl_seconds":60}],"aliases":[]}`,
		`{"schema":"m1_dns_ipc_v1","kind":"RESULT","attempt":"a","session_generation":1,"sequence":1,"status":"completed","observed_at_ms":1,"rcode":0,"truncated":false,"complete":true,"withheld_aliases":0,"answers":[{"ipv4":"192.0.2.53"}],"aliases":[]}`,
		`{"schema":"m1_dns_ipc_v1","kind":"RESULT","attempt":"a","session_generation":1,"sequence":1,"status":"completed","observed_at_ms":1,"rcode":0,"truncated":false,"complete":true,"withheld_aliases":0,"answers":[{"ipv4":"192.0.2.53","ttl_seconds":60,"extra":1}],"aliases":[]}`,
		`{"schema":"m1_dns_ipc_v1","kind":"RESULT","attempt":"a","session_generation":1,"sequence":1,"status":"completed","observed_at_ms":1,"rcode":0,"truncated":false,"complete":true,"withheld_aliases":0,"answers":[{"ipv4":null,"ttl_seconds":60}],"aliases":[]}`,
	} {
		var got Result
		if (&Reader{Source: bytes.NewReader(frame(mutant))}).Read(&got, Limit) == nil {
			t.Fatalf("non-canonical frame accepted: %s", mutant)
		}
	}
}
