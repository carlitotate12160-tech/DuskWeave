// Package frame implements the ADR-017 length-prefixed strict-JSON control
// channel and the binary raw channel between the boundary pair. Every reader
// and writer is the sole owner of its endpoint; no stream is shared.
package frame

import (
	"bytes"
	"encoding/binary"
	"encoding/json"
	"errors"
	"fmt"
	"io"
)

// MaxJSONFrame bounds any control message. Chosen below ADR-013 effective
// request bounds; checked before allocation.
const MaxJSONFrame = 16384

// MaxRawFrame bounds one binary raw chunk on the capture-to-exporter channel.
const MaxRawFrame = 4096

var (
	ErrOversize    = errors.New("frame_oversize")
	ErrInvalidJSON = errors.New("frame_invalid_json")
	ErrDupKey      = errors.New("frame_duplicate_key")
	ErrDepth       = errors.New("frame_depth")
	ErrTrailing    = errors.New("frame_trailing_data")
)

// ReadJSON reads one length-prefixed frame and decodes it strictly into v:
// unknown keys, duplicate keys, nesting depth, numeric overflow and trailing
// data are all rejected.
func ReadJSON(r io.Reader, v any) error {
	raw, err := readLenPrefixed(r, MaxJSONFrame)
	if err != nil {
		return err
	}
	if err := checkStrict(raw); err != nil {
		return err
	}
	dec := json.NewDecoder(bytes.NewReader(raw))
	dec.DisallowUnknownFields()
	if err := dec.Decode(v); err != nil {
		return fmt.Errorf("%w: %v", ErrInvalidJSON, err)
	}
	if dec.More() {
		return ErrTrailing
	}
	return nil
}

// WriteJSON encodes v as one strict JSON object frame.
func WriteJSON(w io.Writer, v any) error {
	raw, err := json.Marshal(v)
	if err != nil {
		return fmt.Errorf("marshal: %w", err)
	}
	if len(raw) > MaxJSONFrame {
		return ErrOversize
	}
	return writeLenPrefixed(w, raw)
}

// WriteRaw writes one binary raw chunk frame. A zero-length chunk is the
// end-of-raw sentinel.
func WriteRaw(w io.Writer, b []byte) error {
	if len(b) > MaxRawFrame {
		return ErrOversize
	}
	return writeLenPrefixed(w, b)
}

// WriteSentinel terminates the raw channel.
func WriteSentinel(w io.Writer) error {
	return writeLenPrefixed(w, nil)
}

// ReadRawChunk reads one binary raw chunk. The boolean reports whether the
// sentinel (end of raw data) was reached.
func ReadRawChunk(r io.Reader, dst *bytes.Buffer) (bool, error) {
	var hdr [4]byte
	if _, err := io.ReadFull(r, hdr[:]); err != nil {
		return false, err
	}
	n := binary.BigEndian.Uint32(hdr[:])
	if n == 0 {
		return true, nil
	}
	if n > MaxRawFrame {
		return false, ErrOversize
	}
	if dst != nil && int64(dst.Len())+int64(n) > int64(MaxRawFrame*8) {
		return false, ErrOversize
	}
	buf := make([]byte, n)
	if _, err := io.ReadFull(r, buf); err != nil {
		return false, err
	}
	if dst != nil {
		dst.Write(buf)
	}
	return false, nil
}

func readLenPrefixed(r io.Reader, max uint32) ([]byte, error) {
	var hdr [4]byte
	if _, err := io.ReadFull(r, hdr[:]); err != nil {
		return nil, err
	}
	n := binary.BigEndian.Uint32(hdr[:])
	if n > max {
		return nil, ErrOversize
	}
	buf := make([]byte, n)
	if _, err := io.ReadFull(r, buf); err != nil {
		return nil, err
	}
	return buf, nil
}

func writeLenPrefixed(w io.Writer, b []byte) error {
	var hdr [4]byte
	binary.BigEndian.PutUint32(hdr[:], uint32(len(b)))
	if _, err := w.Write(hdr[:]); err != nil {
		return err
	}
	if len(b) == 0 {
		return nil
	}
	_, err := w.Write(b)
	return err
}

// checkStrict rejects duplicate object keys and excess nesting before any
// typed decode, walking the token stream without materializing a map. The
// root must be a single JSON object.
func checkStrict(raw []byte) error {
	dec := json.NewDecoder(bytes.NewReader(raw))
	tok, err := dec.Token()
	if err != nil {
		return fmt.Errorf("%w: %v", ErrInvalidJSON, err)
	}
	if d, ok := tok.(json.Delim); !ok || d != '{' {
		return ErrInvalidJSON
	}
	depth := 1
	if err := walkObject(dec, &depth); err != nil {
		return err
	}
	if dec.More() {
		return ErrTrailing
	}
	return nil
}

// walkObject consumes members through the matching '}'.
func walkObject(dec *json.Decoder, depth *int) error {
	seen := map[string]bool{}
	for dec.More() {
		tok, err := dec.Token()
		if err != nil {
			return fmt.Errorf("%w: %v", ErrInvalidJSON, err)
		}
		key, ok := tok.(string)
		if !ok || seen[key] {
			return ErrDupKey
		}
		seen[key] = true
		if err := walkValue(dec, depth); err != nil {
			return err
		}
	}
	return expectDelim(dec, '}')
}

// walkArray consumes elements through the matching ']'.
func walkArray(dec *json.Decoder, depth *int) error {
	for dec.More() {
		if err := walkValue(dec, depth); err != nil {
			return err
		}
	}
	return expectDelim(dec, ']')
}

func walkValue(dec *json.Decoder, depth *int) error {
	tok, err := dec.Token()
	if err != nil {
		return fmt.Errorf("%w: %v", ErrInvalidJSON, err)
	}
	d, ok := tok.(json.Delim)
	if !ok {
		return nil
	}
	switch d {
	case '{':
		*depth++
		if *depth > 8 {
			return ErrDepth
		}
		err := walkObject(dec, depth)
		*depth--
		return err
	case '[':
		*depth++
		if *depth > 8 {
			return ErrDepth
		}
		err := walkArray(dec, depth)
		*depth--
		return err
	}
	return ErrInvalidJSON
}

func expectDelim(dec *json.Decoder, want json.Delim) error {
	tok, err := dec.Token()
	if err != nil {
		return fmt.Errorf("%w: %v", ErrInvalidJSON, err)
	}
	if d, ok := tok.(json.Delim); !ok || d != want {
		return ErrInvalidJSON
	}
	return nil
}
