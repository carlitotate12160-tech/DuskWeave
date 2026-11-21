package main

import (
	"crypto/sha256"
	"crypto/tls"
	"crypto/x509"
	"encoding/hex"
	"encoding/json"
	"errors"
	"io"
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
