package frame

import (
	"bytes"
	"encoding/binary"
	"errors"
	"testing"
)

func writeJSON(t *testing.T, v any) []byte {
	t.Helper()
	var buf bytes.Buffer
	if err := WriteJSON(&buf, v); err != nil {
		t.Fatalf("write: %v", err)
	}
	return buf.Bytes()
}

func TestRoundTrip(t *testing.T) {
	raw := writeJSON(t, &Ready{Version: RequestVersion, Phase: PhaseReady, Kind: KindCapture, AttemptID: "a1"})
	var got Ready
	if err := ReadJSON(bytes.NewReader(raw), &got); err != nil {
		t.Fatalf("read: %v", err)
	}
	if got.AttemptID != "a1" || got.Kind != KindCapture {
		t.Fatalf("decoded %+v", got)
	}
}

func framed(t *testing.T, body string) []byte {
	t.Helper()
	var buf bytes.Buffer
	if err := binary.Write(&buf, binary.BigEndian, uint32(len(body))); err != nil {
		t.Fatal(err)
	}
	buf.WriteString(body)
	return buf.Bytes()
}

func TestRejects(t *testing.T) {
	cases := map[string]error{
		`{"version":"x","version":"y"}`:                              ErrDupKey,
		`{"a":{"b":{"c":{"d":{"e":{"f":{"g":{"h":{"i":1}}}}}}}}}`:     ErrDepth,
		`{"version":"x"}junk`:                                        ErrTrailing,
		`{"version":1e999}`:                                          nil, // numbers tolerated at token level; typed decode bounds
		`[1,2]`:                                                      ErrInvalidJSON,
		`{"a":{"a":{"a":{"a":{"a":{"a":{"a":{"a":{"a":{}}}}}}}}}`:     ErrDepth,
		`{"phase":"ready","phase":"start"}`:                          ErrDupKey,
	}
	for body, want := range cases {
		var v struct {
			Version string `json:"version"`
		}
		err := ReadJSON(bytes.NewReader(framed(t, body)), &v)
		if want == nil && err != nil {
			continue // token-level tolerance; typed decode may still fail
		}
		if want != nil && !errors.Is(err, want) {
			t.Fatalf("body %q: got %v want %v", body, err, want)
		}
	}
}

func TestOversize(t *testing.T) {
	var buf bytes.Buffer
	if err := binary.Write(&buf, binary.BigEndian, uint32(MaxJSONFrame+1)); err != nil {
		t.Fatal(err)
	}
	var v Ready
	if err := ReadJSON(bytes.NewReader(buf.Bytes()), &v); !errors.Is(err, ErrOversize) {
		t.Fatalf("got %v", err)
	}
}

func TestRawChannel(t *testing.T) {
	var buf bytes.Buffer
	if err := WriteRaw(&buf, []byte{1, 2, 3}); err != nil {
		t.Fatal(err)
	}
	if err := WriteSentinel(&buf); err != nil {
		t.Fatal(err)
	}
	r := bytes.NewReader(buf.Bytes())
	var dst bytes.Buffer
	done, err := ReadRawChunk(r, &dst)
	if err != nil || done || dst.Len() != 3 {
		t.Fatalf("chunk: done=%v err=%v len=%d", done, err, dst.Len())
	}
	done, err = ReadRawChunk(r, &dst)
	if err != nil || !done {
		t.Fatalf("sentinel: done=%v err=%v", done, err)
	}
}
