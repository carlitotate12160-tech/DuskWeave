package main

import (
	"crypto/sha256"
	"crypto/tls"
	"crypto/x509"
	"duskweave/boundary/internal/dns"
	"duskweave/boundary/internal/frame"
	"encoding/binary"
	"encoding/hex"
	"encoding/json"
	"errors"
	"io"
	"net"
	"net/http"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"
)

func TestLabRootPEMMatchesCommittedFixture(t *testing.T) {
	stored, err := os.ReadFile("../../fixtures/https-lab-root-v1.pem")
	if err != nil {
		t.Fatal(err)
	}
	if string(stored) != LabRootPEM {
		t.Fatalf("embedded LabRootPEM differs from fixtures/https-lab-root-v1.pem")
	}
	sum := sha256.Sum256(stored)
	const wantSHA = "af8714b55edd3a0a47cf24a1b24a6e99d9026ed4dd074b5a972a60618103d1dc"
	if hex.EncodeToString(sum[:]) != wantSHA {
		t.Fatalf("unexpected root SHA %x", sum)
	}
}

func labTestClient(t *testing.T, serverName string) *http.Client {
	t.Helper()
	cert, _, err := loadSyntheticRoot()
	if err != nil {
		t.Fatal(err)
	}
	pool := x509.NewCertPool()
	pool.AddCert(cert)
	return &http.Client{
		Timeout: 2 * time.Second,
		CheckRedirect: func(*http.Request, []*http.Request) error {
			return http.ErrUseLastResponse
		},
		Transport: &http.Transport{
			TLSClientConfig: &tls.Config{
				RootCAs:    pool,
				ServerName: serverName,
				MinVersion: tls.VersionTLS12,
			},
		},
	}
}

func startLoopbackService(t *testing.T, scenario, counters string) *Service {
	t.Helper()
	svc, err := Start("127.0.0.1:0", scenario, counters)
	if err != nil {
		t.Fatalf("Start(%s): %v", scenario, err)
	}
	if scenario == "timeout" {
		svc.timeoutHold = 400 * time.Millisecond
	}
	go svc.Serve()
	t.Cleanup(func() { _ = svc.Close() })
	return svc
}

func TestScenariosAndCounterIsolation(t *testing.T) {
	tmp := t.TempDir()
	counters := filepath.Join(tmp, "counters.json")
	svc := startLoopbackService(t, "200", counters)
	client := labTestClient(t, labHostname)
	resp, err := client.Head("https://" + svc.Addr() + "/")
	if err != nil {
		t.Fatalf("200 HEAD failed: %v", err)
	}
	_ = resp.Body.Close()
	if resp.StatusCode != 200 {
		t.Fatalf("want 200 got %d", resp.StatusCode)
	}
	if err := svc.Close(); err != nil {
		t.Fatalf("Close 200: %v", err)
	}
	raw, err := os.ReadFile(counters)
	if err != nil {
		t.Fatal(err)
	}
	var got Counters
	if err := json.Unmarshal(raw, &got); err != nil {
		t.Fatal(err)
	}
	if got.Scenario != "200" || got.Connections != 1 || got.HeadRequests != 1 || got.TLSHandshakes != 1 {
		t.Fatalf("unexpected counters %+v", got)
	}

	svc302 := startLoopbackService(t, "302", filepath.Join(tmp, "c302.json"))
	r302, err := client.Head("https://" + svc302.Addr() + "/")
	if err != nil {
		t.Fatalf("302 HEAD failed: %v", err)
	}
	_ = r302.Body.Close()
	if r302.StatusCode != 302 {
		t.Fatalf("want 302 got %d", r302.StatusCode)
	}
	if err := svc302.Close(); err != nil {
		t.Fatalf("Close 302: %v", err)
	}
	c302Bytes, _ := os.ReadFile(filepath.Join(tmp, "c302.json"))
	if strings.Contains(string(c302Bytes), "other.duskweave.test") {
		t.Fatalf("counter file leaked redirect host")
	}
}

func TestNegativeScenarios(t *testing.T) {
	for _, tc := range []struct {
		scenario string
		wantAny  bool
	}{
		{"wrong_hostname", true},
		{"wrong_root", true},
		{"expired", true},
		{"connection_close", true},
		{"timeout", true},
	} {
		t.Run(tc.scenario, func(t *testing.T) {
			svc := startLoopbackService(t, tc.scenario, "")
			client := labTestClient(t, labHostname)
			client.Timeout = 120 * time.Millisecond
			_, err := client.Head("https://" + svc.Addr() + "/")
			if err == nil {
				t.Fatalf("expected error for %s", tc.scenario)
			}
		})
	}

	t.Run("oversize_header", func(t *testing.T) {
		svc := startLoopbackService(t, "oversize_header", "")
		cert, _, _ := loadSyntheticRoot()
		pool := x509.NewCertPool()
		pool.AddCert(cert)
		conn, err := tls.Dial("tcp", svc.Addr(), &tls.Config{RootCAs: pool, ServerName: labHostname})
		if err != nil {
			t.Fatal(err)
		}
		defer conn.Close()
		if _, err := conn.Write([]byte("HEAD / HTTP/1.1\r\nHost: " + labHostname + "\r\nConnection: close\r\n\r\n")); err != nil {
			t.Fatal(err)
		}
		buf, _ := io.ReadAll(conn)
		if len(buf) <= 8192 {
			t.Fatalf("expected >8192 header bytes, got %d", len(buf))
		}
	})

	t.Run("http2", func(t *testing.T) {
		svc := startLoopbackService(t, "http2", "")
		cert, _, _ := loadSyntheticRoot()
		pool := x509.NewCertPool()
		pool.AddCert(cert)
		conn, err := tls.Dial("tcp", svc.Addr(), &tls.Config{
			RootCAs: pool, ServerName: labHostname, NextProtos: []string{"http/1.1", "h2"},
		})
		if err != nil {
			t.Fatal(err)
		}
		defer conn.Close()
		if conn.ConnectionState().NegotiatedProtocol != "h2" {
			t.Fatalf("expected h2 ALPN")
		}
		buf, err := io.ReadAll(conn)
		if err != nil && !errors.Is(err, io.EOF) {
			t.Fatal(err)
		}
		if !strings.HasPrefix(string(buf), "HTTP/2.0 ") {
			t.Fatalf("expected HTTP/2.0 prefix, got %q", string(buf))
		}
	})

	if _, err := Start("127.0.0.1:0", "invalid_scenario", ""); err == nil {
		t.Fatalf("expected invalid scenario error")
	}
}

func sendUDPQuery(t *testing.T, addr string, pkt []byte) []byte {
	t.Helper()
	conn, err := net.DialTimeout("udp4", addr, time.Second)
	if err != nil {
		t.Fatalf("dial udp4 %s: %v", addr, err)
	}
	defer conn.Close()
	_ = conn.SetDeadline(time.Now().Add(time.Second))
	if _, err := conn.Write(pkt); err != nil {
		t.Fatalf("write udp: %v", err)
	}
	buf := make([]byte, 512)
	n, err := conn.Read(buf)
	if err != nil {
		t.Fatalf("read udp: %v", err)
	}
	return buf[:n]
}

func TestSyntheticDNSAnswersRefusalsTTLsAndShutdown(t *testing.T) {
	tmp := t.TempDir()
	countersPath := filepath.Join(tmp, "dns-https.json")
	svc, err := StartWithOptions(Options{
		ListenAddr:    "127.0.0.1:0",
		DNSListenAddr: "127.0.0.1:0",
		Scenario:      "200",
		CounterPath:   countersPath,
		RunID:         "run-dns-check-1",
		DNSTTL:        2,
		DNSRefreshTTL: 30,
	})
	if err != nil {
		t.Fatalf("StartWithOptions: %v", err)
	}
	if svc.DNSAddr() == "" {
		t.Fatalf("expected bound UDP DNS address")
	}
	done := make(chan struct{})
	go func() {
		svc.Serve()
		close(done)
	}()

	req := frame.Request{
		QueryID:      42,
		Name:         labHostname,
		AllowedNames: []string{labHostname},
	}
	q1, err := dns.Query(42, labHostname)
	if err != nil {
		t.Fatal(err)
	}
	r1 := sendUDPQuery(t, svc.DNSAddr(), q1)
	p1, err := dns.Decode(r1, req)
	if err != nil {
		t.Fatalf("Decode r1: %v", err)
	}
	if p1.Rcode != 0 || len(p1.Answers) != 1 || p1.Answers[0].IPv4 != "198.51.100.10" || p1.Answers[0].TTL != 2 {
		t.Fatalf("unexpected first DNS projection: %+v", p1)
	}

	// Second answered query uses DNSRefreshTTL=30.
	q2, _ := dns.Query(43, labHostname)
	r2 := sendUDPQuery(t, svc.DNSAddr(), q2)
	req2 := req
	req2.QueryID = 43
	p2, err := dns.Decode(r2, req2)
	if err != nil {
		t.Fatalf("Decode r2: %v", err)
	}
	if p2.Rcode != 0 || len(p2.Answers) != 1 || p2.Answers[0].TTL != 30 {
		t.Fatalf("unexpected refresh DNS projection: %+v", p2)
	}

	// Wrong hostname -> REFUSED (RCODE=5).
	qWrongName, _ := dns.Query(44, "other.duskweave.test")
	rWrongName := sendUDPQuery(t, svc.DNSAddr(), qWrongName)
	if len(rWrongName) < 12 || rWrongName[3]&0x0f != 5 {
		t.Fatalf("expected REFUSED for wrong hostname, got %x", rWrongName)
	}

	// Wrong QTYPE (AAAA=28) -> REFUSED (RCODE=5).
	qAAAA := append([]byte(nil), q1...)
	binary.BigEndian.PutUint16(qAAAA[0:2], 45)
	binary.BigEndian.PutUint16(qAAAA[len(qAAAA)-4:len(qAAAA)-2], 28)
	rAAAA := sendUDPQuery(t, svc.DNSAddr(), qAAAA)
	if len(rAAAA) < 12 || rAAAA[3]&0x0f != 5 {
		t.Fatalf("expected REFUSED for AAAA query, got %x", rAAAA)
	}

	// Wrong QCLASS (CH=3) -> REFUSED (RCODE=5).
	qCH := append([]byte(nil), q1...)
	binary.BigEndian.PutUint16(qCH[0:2], 46)
	binary.BigEndian.PutUint16(qCH[len(qCH)-2:], 3)
	rCH := sendUDPQuery(t, svc.DNSAddr(), qCH)
	if len(rCH) < 12 || rCH[3]&0x0f != 5 {
		t.Fatalf("expected REFUSED for CH class query, got %x", rCH)
	}

	if err := svc.Close(); err != nil {
		t.Fatalf("Close: %v", err)
	}
	select {
	case <-done:
	case <-time.After(2 * time.Second):
		t.Fatalf("Serve did not stop after Close")
	}

	raw, err := os.ReadFile(countersPath)
	if err != nil {
		t.Fatal(err)
	}
	var got Counters
	if err := json.Unmarshal(raw, &got); err != nil {
		t.Fatal(err)
	}
	if got.RunID != "run-dns-check-1" || got.DNSQueries != 5 || got.DNSAnswered != 2 || got.DNSRefused != 3 {
		t.Fatalf("unexpected DNS counters: %+v", got)
	}

	for _, bad := range []Options{
		{ListenAddr: "127.0.0.1:0", DNSListenAddr: "127.0.0.1:0", Scenario: "200", DNSTTL: 0},
		{ListenAddr: "127.0.0.1:0", DNSListenAddr: "127.0.0.1:0", Scenario: "200", DNSTTL: 301},
		{ListenAddr: "127.0.0.1:0", Scenario: "200", RunID: "bad id"},
	} {
		if _, err := StartWithOptions(bad); err == nil {
			t.Fatalf("expected StartWithOptions failure for %+v", bad)
		}
	}
}
