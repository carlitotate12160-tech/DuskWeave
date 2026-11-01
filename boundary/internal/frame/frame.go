// Package frame owns bounded framing, not authority or result admission.
package frame

import (
	"bytes"
	"encoding/binary"
	"encoding/json"
	"errors"
	"io"
	"reflect"
	"strings"
	"unicode/utf8"
)

const Schema = "m1_dns_ipc_v1"
const Limit = 16384
const ControlLimit = 1024
const MaxFrames = 16

var ErrFrame = errors.New("ipc_failure")

type Identity struct {
	Schema     string `json:"schema"`
	Kind       string `json:"kind"`
	Attempt    string `json:"attempt"`
	Generation uint64 `json:"session_generation"`
	Sequence   uint32 `json:"sequence"`
}
type Request struct {
	Identity
	Engagement    string   `json:"engagement_id"`
	Campaign      string   `json:"campaign_id"`
	Decision      string   `json:"decision_id"`
	Profile       string   `json:"profile"`
	Mode          string   `json:"execution_mode"`
	RunProfile    string   `json:"run_profile"`
	Release       string   `json:"release_sha256"`
	Qualification string   `json:"qualification_ref"`
	Name          string   `json:"name"`
	Resolver      string   `json:"resolver_ipv4"`
	Transport     string   `json:"transport"`
	QueryID       uint16   `json:"query_id"`
	AllowedNames  []string `json:"allowed_names"`
	Deadline      int64    `json:"deadline_at_ms"`
	Duration      uint32   `json:"max_duration_ms"`
}
type Control struct {
	Identity
	Deadline int64 `json:"deadline_at_ms"`
}

// Raw is permitted only on the private capture-to-exporter pipe.
type Raw struct {
	Identity
	Source   string `json:"source_ipv4"`
	Status   string `json:"status"`
	Observed int64  `json:"observed_at_ms"`
	Packet   []byte `json:"packet"`
}
type Answer struct {
	IPv4 string `json:"ipv4"`
	TTL  uint32 `json:"ttl_seconds"`
}
type Result struct {
	Identity
	Status          string   `json:"status"`
	Observed        int64    `json:"observed_at_ms"`
	Rcode           uint8    `json:"rcode"`
	Truncated       bool     `json:"truncated"`
	Complete        bool     `json:"complete"`
	WithheldAliases uint32   `json:"withheld_aliases"`
	Answers         []Answer `json:"answers"`
	Aliases         []string `json:"aliases"`
}
type Reader struct {
	Source io.Reader
	Count  uint32
}
type Writer struct {
	Sink  io.Writer
	Count uint32
}

func (r *Reader) Read(dst any, limit uint32) error {
	if r.Count >= MaxFrames || limit == 0 || limit > Limit {
		return ErrFrame
	}
	var header [4]byte
	if _, err := io.ReadFull(r.Source, header[:]); err != nil {
		return ErrFrame
	}
	n := binary.BigEndian.Uint32(header[:])
	if n == 0 || n > limit {
		return ErrFrame
	}
	data := make([]byte, int(n))
	if _, err := io.ReadFull(r.Source, data); err != nil {
		return ErrFrame
	}
	if !utf8.Valid(data) || validate(data) != nil {
		return ErrFrame
	}
	var fields map[string]json.RawMessage
	if json.Unmarshal(data, &fields) != nil || !required(reflect.TypeOf(dst), fields) {
		return ErrFrame
	}
	d := json.NewDecoder(bytes.NewReader(data))
	d.DisallowUnknownFields()
	if err := d.Decode(dst); err != nil {
		return ErrFrame
	}
	r.Count++
	return nil
}

// Every field in this closed protocol is required, including legal zero values.
func required(t reflect.Type, fields map[string]json.RawMessage) bool {
	if t == nil {
		return false
	}
	if t.Kind() == reflect.Pointer {
		t = t.Elem()
	}
	if t.Kind() != reflect.Struct {
		return false
	}
	for i := 0; i < t.NumField(); i++ {
		field := t.Field(i)
		if field.Anonymous {
			if !required(field.Type, fields) {
				return false
			}
			continue
		}
		name := strings.Split(field.Tag.Get("json"), ",")[0]
		if _, found := fields[name]; !found {
			return false
		}
		if bytes.Equal(bytes.TrimSpace(fields[name]), []byte("null")) {
			return false
		}
	}
	return true
}
func (w *Writer) Write(value any, limit uint32) error {
	if w.Count >= MaxFrames || limit == 0 || limit > Limit {
		return ErrFrame
	}
	data, err := json.Marshal(value)
	if err != nil || len(data) == 0 || len(data) > int(limit) {
		return ErrFrame
	}
	var header [4]byte
	binary.BigEndian.PutUint32(header[:], uint32(len(data)))
	for _, part := range [][]byte{header[:], data} {
		for len(part) > 0 {
			n, err := w.Sink.Write(part)
			if err != nil || n <= 0 || n > len(part) {
				return ErrFrame
			}
			part = part[n:]
		}
	}
	w.Count++
	return nil
}
func validate(data []byte) error {
	data = bytes.TrimSpace(data)
	if len(data) == 0 || data[0] != '{' {
		return ErrFrame
	}
	d := json.NewDecoder(bytes.NewReader(data))
	d.UseNumber()
	if err := node(d, 1); err != nil {
		return ErrFrame
	}
	if _, err := d.Token(); err != io.EOF {
		return ErrFrame
	}
	return nil
}
func node(d *json.Decoder, depth int) error {
	token, err := d.Token()
	if err != nil {
		return ErrFrame
	}
	delim, container := token.(json.Delim)
	if !container {
		return nil
	}
	if depth > 8 {
		return ErrFrame
	}
	if delim != '{' && delim != '[' {
		return ErrFrame
	}
	seen, count := make(map[string]bool), 0
	for d.More() {
		count++
		if count > 64 {
			return ErrFrame
		}
		if delim == '{' {
			key, err := d.Token()
			name, ok := key.(string)
			if err != nil || !ok || seen[name] {
				return ErrFrame
			}
			seen[name] = true
		}
		if err := node(d, depth+1); err != nil {
			return ErrFrame
		}
	}
	end, err := d.Token()
	if err != nil || (delim == '{' && end != json.Delim('}')) || (delim == '[' && end != json.Delim(']')) {
		return ErrFrame
	}
	return nil
}
func Bound(q Request, kind string, sequence uint32) Identity {
	return Identity{Schema, kind, q.Attempt, q.Generation, sequence}
}
func Matches(id Identity, q Request, kind string, sequence uint32) bool {
	return id == Bound(q, kind, sequence)
}
