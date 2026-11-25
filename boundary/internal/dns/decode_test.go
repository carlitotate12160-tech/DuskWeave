package dns

import (
	"bytes"
	"duskweave/boundary/internal/frame"
	"encoding/binary"
	"strings"
	"testing"
	"time"
)

func question() frame.Request {
	return frame.Request{Name: "seed.duskweave.test", QueryID: 17, AllowedNames: []string{"seed.duskweave.test"}}
}
func response(t *testing.T) []byte {
	t.Helper()
	p, err := Query(17, "seed.duskweave.test")
	if err != nil {
		t.Fatal(err)
	}
	p[2], p[3], p[7] = 0x81, 0x80, 1
	return append(p, 0xc0, 12, 0, 1, 0, 1, 0, 0, 0, 30, 0, 4, 198, 51, 100, 7)
}

func TestQualifiedAAndTruthfulLimitedResults(t *testing.T) {
	p, err := Decode(response(t), question())
	if err != nil || !p.Complete || len(p.Answers) != 1 || p.Answers[0].IPv4 != "198.51.100.7" || p.Answers[0].TTL != 30 {
		t.Fatal("valid A projection failed")
	}
	for _, code := range []byte{0, 3} {
		wire, _ := Query(17, "seed.duskweave.test")
		wire[2], wire[3] = 0x81, 0x80|code
		p, err = Decode(wire, question())
		if err != nil || p.Rcode != code || len(p.Answers) != 0 {
			t.Fatal("empty or negative result fabricated")
		}
	}
	wire := response(t)
	wire[2] |= 2
	p, err = Decode(wire, question())
	if err != nil || !p.Truncated || p.Complete || len(p.Answers) != 0 {
		t.Fatal("truncation promoted")
	}
}

func TestMalformedAndUnexpectedMaterialIsRejected(t *testing.T) {
	base := response(t)
	for _, change := range []func([]byte) []byte{
		func(p []byte) []byte { p[1]++; return p },
		func(p []byte) []byte { p[12] = 0xc0; p[13] = 12; return p },
		func(p []byte) []byte { p[12] = 0xc0; p[13] = 14; return p },
		func(p []byte) []byte { p[7] = 33; return p },
		func(p []byte) []byte { p[len(p)-13] = 16; return p },
		func(p []byte) []byte { return p[:len(p)-1] },
		func(p []byte) []byte { return append(p, 0) },
		func(p []byte) []byte { return append(p, bytes.Repeat([]byte("RAW_SENTINEL"), 500)...) },
		func(p []byte) []byte { binary.BigEndian.PutUint32(p[len(p)-10:], 86401); return p },
	} {
		if _, err := Decode(change(bytes.Clone(base)), question()); err == nil {
			t.Fatal("malformed or unsupported DNS accepted")
		}
	}
	// A literal dot inside a single wire label must not be normalized into labels.
	wire := []byte{0, 17, 0x81, 0x80, 0, 1, 0, 0, 0, 0, 0, 0, 19}
	wire = append(wire, []byte("seed.duskweave.test")...)
	wire = append(wire, 0, 0, 1, 0, 1)
	if _, err := Decode(wire, question()); err == nil {
		t.Fatal("embedded label dot accepted")
	}
}

func TestTCPFramingDoesNotAdmitExtraOrTruncatedData(t *testing.T) {
	packet := response(t)
	framed, err := TCPPacket(packet)
	if err != nil {
		t.Fatal(err)
	}
	got, err := TCPDecode(framed)
	if err != nil || !bytes.Equal(got, packet) {
		t.Fatal("TCP round trip")
	}
	for _, data := range [][]byte{framed[:1], framed[:len(framed)-1], append(bytes.Clone(framed), 0), {255, 255}} {
		if _, err := TCPDecode(data); err == nil {
			t.Fatal("invalid TCP length accepted")
		}
	}
}

func TestConflictingCNAMEAndAddressCannotBecomeAnEmptyObservation(t *testing.T) {
	wire := response(t)
	wire[7] = 2
	wire = append(wire, 0xc0, 12, 0, 5, 0, 1, 0, 0, 0, 30, 0, 8, 5)
	wire = append(wire, []byte("alias")...)
	wire = append(wire, 0xc0, 17)
	q := question()
	q.AllowedNames = append(q.AllowedNames, "alias.duskweave.test")
	if _, err := Decode(wire, q); err == nil {
		t.Fatal("conflicting owner data became an admitted observation")
	}
}

func TestDuplicateAddressesRetainLowestTTL(t *testing.T) {
	wire := response(t)
	wire[7] = 2
	wire = append(wire, 0xc0, 12, 0, 1, 0, 1, 0, 0, 0, 3, 0, 4, 198, 51, 100, 7)
	p, err := Decode(wire, question())
	if err != nil || len(p.Answers) != 1 || p.Answers[0].TTL != 3 {
		t.Fatal("duplicate addresses were not reconciled conservatively")
	}
}

func TestValidateHttpsLabAndLegacyBranchesMapB(t *testing.T) {
	now := time.Now()
	id := "00000000-0000-0000-0000-000000000001"
	valid := frame.Request{
		Identity:      frame.Identity{Schema: frame.Schema, Kind: "REQUEST", Attempt: id, Generation: 1, Sequence: 0},
		Engagement:    id,
		Campaign:      id,
		Decision:      id,
		Profile:       "dns-routing-v2",
		Mode:          "live",
		RunProfile:    "autonomous-https-genai-lab-v1",
		Release:       strings.Repeat("a", 64),
		Qualification: id,
		Name:          "head.duskweave.test",
		Resolver:      "192.0.2.53",
		Transport:     "udp",
		QueryID:       7,
		AllowedNames:  []string{"head.duskweave.test"},
		Deadline:      now.Add(2 * time.Second).UnixMilli(),
		Duration:      2000,
	}
	if err := Validate(valid, now); err != nil {
		t.Fatalf("Map B: valid HTTPS LAB DNS request rejected: %v", err)
	}
	// Legacy DNS LAB retains suffix rule.
	legacyLab := valid
	legacyLab.RunProfile = "autonomous-dns-genai-lab-v1"
	legacyLab.Name = "other.duskweave.test"
	legacyLab.AllowedNames = []string{"other.duskweave.test"}
	if err := Validate(legacyLab, now); err != nil {
		t.Fatalf("legacy DNS LAB suffix request rejected: %v", err)
	}
	// Negative mutations for HTTPS LAB singleton scope and common validation.
	for name, mutate := range map[string]func(*frame.Request){
		"wrong_resolver": func(q *frame.Request) { q.Resolver = "192.0.2.54" },
		"suffix_only_host": func(q *frame.Request) {
			q.Name = "other.duskweave.test"
			q.AllowedNames = []string{"other.duskweave.test"}
		},
		"outside_domain": func(q *frame.Request) {
			q.Name = "head.example.invalid"
			q.AllowedNames = []string{"head.example.invalid"}
		},
		"multiple_allowed":  func(q *frame.Request) { q.AllowedNames = []string{"head.duskweave.test", "extra.duskweave.test"} },
		"duplicate_allowed": func(q *frame.Request) { q.AllowedNames = []string{"head.duskweave.test", "head.duskweave.test"} },
		"empty_allowed":     func(q *frame.Request) { q.AllowedNames = []string{} },
		"uppercase_name": func(q *frame.Request) {
			q.Name = "Head.duskweave.test"
			q.AllowedNames = []string{"Head.duskweave.test"}
		},
		"trailing_dot": func(q *frame.Request) {
			q.Name = "head.duskweave.test."
			q.AllowedNames = []string{"head.duskweave.test."}
		},
		"wrong_mode":          func(q *frame.Request) { q.Mode = "controlled_fixture" },
		"wrong_profile":       func(q *frame.Request) { q.Profile = "dns-routing-fixture-v1" },
		"wrong_schema":        func(q *frame.Request) { q.Schema = "m1_https_ipc_v1" },
		"wrong_transport":     func(q *frame.Request) { q.Transport = "tcp" },
		"unknown_run_profile": func(q *frame.Request) { q.RunProfile = "unknown-profile-v1" },
	} {
		q := valid
		mutate(&q)
		if Validate(q, now) == nil {
			t.Fatalf("Map B: accepted invalid DNS mutation %s", name)
		}
	}
}
