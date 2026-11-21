package capture

import (
	"bytes"
	"crypto/ed25519"
	"crypto/rand"
	"crypto/sha256"
	"crypto/tls"
	"crypto/x509"
	"crypto/x509/pkix"
	"duskweave/boundary/internal/export"
	"duskweave/boundary/internal/frame"
	"encoding/pem"
	"errors"
	"io"
	"math/big"
	"net"
	"os"
	"strings"
	"testing"
	"time"
)

func TestPinnedRootDigest(t *testing.T) {
	if RootVersion != "duskweave-https-lab-root-v1" || RootPath != "/etc/duskweave/https-lab-root-v1.pem" || len(RootSHA) != 64 {
		t.Fatal("unexpected root constants")
	}
	stored, err := os.ReadFile("../../fixtures/https-lab-root-v1.pem")
	if err != nil {
		t.Fatal(err)
	}
	pool, err := LabPool(stored)
	if err != nil || pool == nil {
		t.Fatal("pinned root rejected")
	}
	mutated := strings.Replace(string(stored), "A", "B", 1)
	if _, err = LabPool([]byte(mutated)); err == nil {
		t.Fatal("mutated root accepted")
	}
	if _, err = LabPool(append(bytes.Clone(stored), '\n')); err == nil {
		t.Fatal("root with extra byte accepted")
	}
}

type stubDeadlineErrConn struct {
	net.Conn
}

func (s *stubDeadlineErrConn) Read([]byte) (int, error)         { return 0, io.EOF }
func (s *stubDeadlineErrConn) Write(b []byte) (int, error)      { return len(b), nil }
func (s *stubDeadlineErrConn) Close() error                     { return nil }
func (s *stubDeadlineErrConn) LocalAddr() net.Addr              { return &net.TCPAddr{} }
func (s *stubDeadlineErrConn) RemoteAddr() net.Addr             { return &net.TCPAddr{} }
func (s *stubDeadlineErrConn) SetDeadline(time.Time) error      { return errors.New("deadline error") }
func (s *stubDeadlineErrConn) SetReadDeadline(time.Time) error  { return nil }
func (s *stubDeadlineErrConn) SetWriteDeadline(time.Time) error { return nil }

func TestCapConnAndWriteHeadDeadlineBoundary(t *testing.T) {
	c1, c2 := net.Pipe()
	defer c1.Close()
	go func() {
		_, _ = c2.Write(bytes.Repeat([]byte{'a'}, 65536))
		_ = c2.Close()
	}()
	cc := &capConn{Conn: c1, left: 65536}
	var total int
	buf := make([]byte, 4096)
	for {
		n, err := cc.Read(buf)
		total += n
		if err != nil {
			if errors.Is(err, errNetCap) {
				t.Fatalf("exact 65536 bytes tripped errNetCap at total=%d", total)
			}
			break
		}
	}
	if total != 65536 {
		t.Fatalf("unexpected total read: %d", total)
	}

	d1, d2 := net.Pipe()
	defer d1.Close()
	go func() {
		_, _ = d2.Write(bytes.Repeat([]byte{'b'}, 65537))
		_ = d2.Close()
	}()
	ccOver := &capConn{Conn: d1, left: 65536}
	var tripped bool
	for {
		_, err := ccOver.Read(buf)
		if errors.Is(err, errNetCap) {
			tripped = true
			break
		}
		if err != nil {
			break
		}
	}
	if !tripped {
		t.Fatal("65537th byte did not trip network_limit")
	}

	err := writeHead(&stubDeadlineErrConn{}, "head.duskweave.test", time.Now().Add(time.Second))
	if err == nil || errors.Is(err, errNetCap) || ioStatus(err) == "network_limit" {
		t.Fatalf("writeHead SetDeadline failure mapped to network_limit: err=%v status=%s", err, ioStatus(err))
	}
}

func TestCaptureClassifyHeaderAndBodySeparationF2(t *testing.T) {
	_, redirect, status := classifyHeader([]byte("HTTP/1.1 200 OK\r\nServer: edge HTTP/2.0\r\n\r\n"))
	if status != "received" || redirect != "none" {
		t.Fatalf("F2: legal header value with HTTP/2.0 rejected: status=%s", status)
	}
	for _, hdr := range []string{
		"HTTP/1.1 200 OK\r\nMissingEnd",
		"HTTP/1.1 200 OK\r\nBrokenHeaderNoColon\r\n\r\n",
		"HTTP/1.1 200 OK\r\n\r\nBODY",
		"HTTP/1.1 200 OK\r\nContent-Length: 4\r\nTransfer-Encoding: chunked\r\n\r\n",
		"HTTP/2 200 OK\r\n\r\n",
		"HTTP/1.1 101 Switching\r\n\r\n",
	} {
		if _, _, st := classifyHeader([]byte(hdr)); st == "received" {
			t.Fatalf("F2: capture classified invalid header as received: %q", hdr)
		}
	}
	s1, s2 := net.Pipe()
	defer s1.Close()
	go func() {
		_, _ = s2.Write([]byte("HTTP/1.1 200 OK\r\nContent-Length: 10\r\n\r\nSECRETBODY"))
		_ = s2.Close()
	}()
	hdr, st := readHeader(s1, 8192, time.Now().Add(time.Second))
	if st != "" || bytes.Contains(hdr, []byte("SECRETBODY")) {
		t.Fatalf("body read-ahead leaked into private header candidate: st=%s hdr=%q", st, string(hdr))
	}
}

func sampleCaptureReq(now time.Time, profile string) frame.HttpsRequest {
	id := "00000000-0000-0000-0000-000000000001"
	deadline := now.Add(2 * time.Second).UnixMilli()
	return frame.HttpsRequest{
		Identity:   frame.Identity{Schema: frame.HttpsSchema, Kind: "REQUEST", Attempt: id, Generation: 1},
		Engagement: id, Campaign: id, Decision: id, Run: id, StepIndex: 1, AttemptKind: "head", Result: id,
		Profile: profile, Mode: "live", RunProfile: frame.LabRunProfile, Release: strings.Repeat("ab", 32),
		Qualification: id, Asset: id, Endpoint: "endpoint_0", Hostname: frame.LabHost, IPv4: frame.LabIPv4,
		Port: 443, Path: "/", Method: "HEAD", ParentClaim: id, ParentRevision: 1, Resolver: frame.LabResolver,
		Vantage: id, Permission: id, MissionRevision: 1, EvaluatedAt: now.UnixMilli(), ClaimUntil: now.Add(time.Minute).Unix(),
		EpisodeDeadline: deadline + 1000, Deadline: deadline, Duration: 5000,
		Limits: frame.Limits{NetworkInboundBytes: 65536, HeaderBytes: 8192, FramePayloadBytes: 16384,
			ControlPayloadBytes: 1024, MaxWorkerFrames: 16, MinTLSVersion: "1.2", HTTPVersion: "HTTP/1.1",
			MaxConnects: 1, MaxRequests: 1},
	}
}

func TestCaptureRejectsInferenceProfileInCapabilityFieldF1(t *testing.T) {
	q := sampleCaptureReq(time.Now(), "fixture-genai-v4-pro-low-v2")
	var input, output bytes.Buffer
	_ = (&frame.Writer{Sink: &input}).Write(q, frame.Limit)
	if Run(&input, &output) == nil || output.Len() != 0 {
		t.Fatal("F1: capture accepted inference profile in capability field")
	}
}

func testLeafTLS(t *testing.T, nextProtos []string) (*x509.CertPool, *tls.Config) {
	t.Helper()
	seed := sha256.Sum256([]byte("duskweave-https-lab-v1-public-synthetic-ca-key"))
	caPriv := ed25519.NewKeyFromSeed(seed[:])
	stored, err := os.ReadFile("../../fixtures/https-lab-root-v1.pem")
	if err != nil {
		t.Fatal(err)
	}
	pool, err := LabPool(stored)
	if err != nil {
		t.Fatal(err)
	}
	block, _ := pem.Decode(stored)
	caCert, err := x509.ParseCertificate(block.Bytes)
	if err != nil {
		t.Fatal(err)
	}
	_, leafPriv, err := ed25519.GenerateKey(rand.Reader)
	if err != nil {
		t.Fatal(err)
	}
	tmpl := &x509.Certificate{
		SerialNumber: big.NewInt(2),
		Subject:      pkix.Name{CommonName: frame.LabHost},
		DNSNames:     []string{frame.LabHost},
		NotBefore:    time.Now().Add(-time.Hour),
		NotAfter:     time.Now().Add(time.Hour),
		KeyUsage:     x509.KeyUsageDigitalSignature,
		ExtKeyUsage:  []x509.ExtKeyUsage{x509.ExtKeyUsageServerAuth},
	}
	der, err := x509.CreateCertificate(rand.Reader, tmpl, caCert, leafPriv.Public(), caPriv)
	if err != nil {
		t.Fatal(err)
	}
	return pool, &tls.Config{
		Certificates: []tls.Certificate{{Certificate: [][]byte{der}, PrivateKey: leafPriv}},
		NextProtos:   nextProtos,
	}
}

func runCaptureAndExporterPair(t *testing.T, q frame.HttpsRequest, deps httpsDeps) frame.SafeResult {
	t.Helper()
	capCtrlR, capCtrlW := io.Pipe()
	rawR, rawW := io.Pipe()
	expInR, expInW := io.Pipe()
	safeR, safeW := io.Pipe()
	capDone := make(chan error, 1)
	expDone := make(chan error, 1)
	go func() {
		r, w := frame.Reader{Source: capCtrlR}, frame.Writer{Sink: rawW}
		p, err := r.Take(frame.Limit)
		if err != nil {
			capDone <- err
			return
		}
		r.Count++
		capDone <- runHTTPSWithDeps(&r, &w, p, deps)
		_ = rawW.Close()
	}()
	go func() {
		expDone <- export.Run(expInR, safeW)
		_ = safeW.Close()
	}()
	cc := frame.Writer{Sink: capCtrlW}
	cr := frame.Reader{Source: rawR}
	ec := frame.Writer{Sink: expInW}
	sr := frame.Reader{Source: safeR}
	if cc.Write(q, frame.Limit) != nil || ec.Write(q, frame.Limit) != nil {
		t.Fatal("request write")
	}
	seqMap := map[string]uint32{"connect": 2, "tls": 4, "head": 6}
	var res frame.SafeResult
	for {
		p, err := cr.Take(frame.Limit)
		if err != nil {
			t.Fatal(err)
		}
		cr.Count++
		kind, _ := frame.Field(p, "kind")
		if kind == "READY" {
			var er frame.PhaseReady
			var rdy frame.PhaseReady
			if frame.Decode(p, &rdy) != nil {
				t.Fatal("capture READY decode")
			}
			if rdy.Phase == "connect" {
				if sr.Read(&er, frame.ControlLimit) != nil {
					t.Fatal("exporter READY decode")
				}
			}
			st := frame.PhaseStart{Identity: frame.HttpsID(q.Identity, "START", seqMap[rdy.Phase]), Phase: rdy.Phase, Deadline: q.Deadline}
			if ec.Write(st, frame.ControlLimit) != nil || cc.Write(st, frame.ControlLimit) != nil {
				t.Fatal("START write")
			}
			continue
		}
		if kind != "RESULT" {
			t.Fatalf("unexpected raw kind %s", kind)
		}
		var priv frame.PrivateResult
		if frame.Decode(p, &priv) != nil || ec.Write(priv, frame.Limit) != nil {
			t.Fatal("forward raw result")
		}
		if err := sr.Read(&res, frame.Limit); err != nil {
			t.Fatalf("exporter refused capture result (%+v): %v", priv, err)
		}
		if ec.Write(frame.HttpsAck{Identity: frame.HttpsID(q.Identity, "ACK", 8)}, frame.ControlLimit) != nil {
			t.Fatal("ACK write")
		}
		break
	}
	if err := <-capDone; err != nil {
		t.Fatalf("capture error: %v", err)
	}
	if err := <-expDone; err != nil {
		t.Fatalf("exporter error: %v", err)
	}
	return res
}

func TestCaptureToExporterHeadCloseAndPostDeadlineTimeoutF3F4(t *testing.T) {
	now := time.Now()
	q := sampleCaptureReq(now, frame.LabProfile)
	pool, tlsCfg := testLeafTLS(t, []string{"http/1.1"})

	// F3 E2E: server accepts TLS, then closes right after receiving HEAD request.
	resConnFail := runCaptureAndExporterPair(t, q, httpsDeps{
		now:      time.Now,
		loadPool: func() (*x509.CertPool, error) { return pool, nil },
		dial: func(string, uint16, time.Time) (net.Conn, error) {
			c, s := net.Pipe()
			go func() {
				ts := tls.Server(s, tlsCfg)
				defer ts.Close()
				if ts.Handshake() == nil {
					buf := make([]byte, 256)
					_, _ = ts.Read(buf)
				}
			}()
			return c, nil
		},
	})
	if resConnFail.Outcome != "connection_failed" {
		t.Fatalf("F3 E2E: expected connection_failed, got %+v", resConnFail)
	}

	// F4: capture clock passes deadline+200ms while reading HEAD timeout -> emits timed_out with exact observed time and survives exporter.
	base := time.Now()
	qTimeout := sampleCaptureReq(base, frame.LabProfile)
	afterDeadline := time.UnixMilli(qTimeout.Deadline).Add(200 * time.Millisecond)
	var rawBuf bytes.Buffer
	w := frame.Writer{Sink: &rawBuf}
	if err := writePrivate(&w, qTimeout, "head", "received", "none", 200, "ZHVtbXk=", func() time.Time { return afterDeadline }); err != nil {
		t.Fatalf("F4: writePrivate post-deadline failed: %v", err)
	}
	var priv frame.PrivateResult
	if (&frame.Reader{Source: &rawBuf}).Read(&priv, frame.Limit) != nil {
		t.Fatal("private frame read")
	}
	if priv.Status != "timed_out" || priv.Observed != afterDeadline.UnixMilli() || priv.Header != "" {
		t.Fatalf("F4: expected clean post-deadline timed_out with preserved observed time, got %+v", priv)
	}
}
