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
	"sync"
	"testing"
	"time"
)

func mustOK(t *testing.T, err error) {
	t.Helper()
	if err != nil {
		t.Fatal(err)
	}
}

func TestPinnedRootDigest(t *testing.T) {
	if RootVersion != "duskweave-https-lab-root-v1" || RootPath != "/etc/duskweave/https-lab-root-v1.pem" || len(RootSHA) != 64 {
		t.Fatal("unexpected root constants")
	}
	stored, _ := readCAAndPool(t)
	mutated := strings.Replace(string(stored), "A", "B", 1)
	if _, err := LabPool([]byte(mutated)); err == nil {
		t.Fatal("mutated root accepted")
	}
	if _, err := LabPool(append(bytes.Clone(stored), '\n')); err == nil {
		t.Fatal("root with extra byte accepted")
	}
}

func TestImageBuiltRootCAReadAsWorkerUID(t *testing.T) {
	if os.Getenv("DW_TEST_IMAGE_ROOT_CA") != "1" {
		t.Skip("opt-in built-image CA read proof")
	}
	if uid, gid, euid, egid := os.Getuid(), os.Getgid(), os.Geteuid(), os.Getegid(); uid != 65532 || gid != 65532 || euid != 65532 || egid != 65532 {
		t.Fatalf("unexpected worker ids uid=%d gid=%d euid=%d egid=%d", uid, gid, euid, egid)
	}
	pool, err := LoadLabPool()
	if err != nil || pool == nil || pool.Equal(x509.NewCertPool()) {
		t.Fatalf("LoadLabPool failed at %s: %v", RootPath, err)
	}
}

type memConn struct {
	net.Conn
	data        []byte
	reads       int
	deadlineErr error
}

func (m *memConn) Read(p []byte) (int, error) {
	m.reads++
	if len(m.data) == 0 {
		return 0, io.EOF
	}
	n := copy(p, m.data)
	m.data = m.data[n:]
	if len(m.data) == 0 {
		return n, io.EOF
	}
	return n, nil
}
func (m *memConn) Write(b []byte) (int, error)      { return len(b), nil }
func (m *memConn) Close() error                     { return nil }
func (m *memConn) LocalAddr() net.Addr              { return &net.TCPAddr{} }
func (m *memConn) RemoteAddr() net.Addr             { return &net.TCPAddr{} }
func (m *memConn) SetDeadline(time.Time) error      { return m.deadlineErr }
func (m *memConn) SetReadDeadline(time.Time) error  { return nil }
func (m *memConn) SetWriteDeadline(time.Time) error { return nil }

type spyCapConn struct {
	net.Conn
	mu                            sync.Mutex
	reads, bytesRead, writes, wdl int
	writeTimes                    []time.Time
	closed, afterHead             bool
}

func (s *spyCapConn) Read(p []byte) (int, error) {
	n, err := s.Conn.Read(p)
	s.mu.Lock()
	s.reads++
	s.bytesRead += n
	s.mu.Unlock()
	return n, err
}
func (s *spyCapConn) Write(p []byte) (int, error) {
	s.mu.Lock()
	if s.afterHead {
		s.writes++
		s.writeTimes = append(s.writeTimes, time.Now())
	}
	s.mu.Unlock()
	return s.Conn.Write(p)
}
func (s *spyCapConn) Close() error {
	s.mu.Lock()
	s.closed = true
	s.mu.Unlock()
	return s.Conn.Close()
}
func (s *spyCapConn) SetWriteDeadline(t time.Time) error {
	s.mu.Lock()
	if s.afterHead {
		s.wdl++
	}
	s.mu.Unlock()
	return s.Conn.SetWriteDeadline(t)
}

func TestCapConnAndWriteHeadDeadlineBoundary(t *testing.T) {
	c1, c2 := net.Pipe()
	defer c1.Close()
	go func() { _, _ = c2.Write(bytes.Repeat([]byte{'a'}, 65536)); _ = c2.Close() }()
	spyExact := &spyCapConn{Conn: c1}
	cc := &capConn{Conn: spyExact, left: 65536}
	var total int
	buf := make([]byte, 4096)
	for cc.left > 0 {
		n, err := cc.Read(buf)
		total += n
		if errors.Is(err, errNetCap) {
			t.Fatalf("exact 65536 bytes tripped errNetCap at total=%d", total)
		}
		if err != nil {
			break
		}
	}
	if total != 65536 || cc.used != 65536 || spyExact.bytesRead != 65536 {
		t.Fatalf("unexpected total read: total=%d used=%d raw=%d", total, cc.used, spyExact.bytesRead)
	}
	exactHdr := []byte("HTTP/1.1 200 OK\r\nX-Cap: 1\r\n\r\n")
	rawExact := &memConn{data: exactHdr}
	hdr, st := readHeader(&capConn{Conn: rawExact, left: len(exactHdr)}, len(exactHdr), time.Now().Add(time.Second))
	if st != "" || string(hdr) != string(exactHdr) || rawExact.reads != len(exactHdr) {
		t.Fatalf("F5: exact ceiling header failed: status=%q reads=%d/%d hdr=%q", st, rawExact.reads, len(exactHdr), string(hdr))
	}
	d1, d2 := net.Pipe()
	defer d1.Close()
	go func() {
		_, _ = d2.Write(bytes.Repeat([]byte{'b'}, 65536))
		_, _ = d2.Write([]byte{'x'})
		_ = d2.Close()
	}()
	spyOver := &spyCapConn{Conn: d1}
	ccOver := &capConn{Conn: spyOver, left: 65536}
	for ccOver.left > 0 {
		if _, err := ccOver.Read(buf); err != nil {
			t.Fatalf("unexpected read err: %v", err)
		}
	}
	readsAtCap := spyOver.reads
	_, err := ccOver.Read(make([]byte, 16))
	if !errors.Is(err, errNetCap) || !ccOver.tripped || spyOver.reads != readsAtCap || spyOver.bytesRead != 65536 {
		t.Fatalf("F5: read past 65536 cap: err=%v tripped=%v reads=%d->%d rawBytes=%d", err, ccOver.tripped, readsAtCap, spyOver.reads, spyOver.bytesRead)
	}
	err = writeHead(&memConn{deadlineErr: errors.New("deadline error")}, "head.duskweave.test", time.Now().Add(time.Second))
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

func readCAAndPool(t *testing.T) ([]byte, *x509.CertPool) {
	t.Helper()
	stored, err := os.ReadFile("../../fixtures/https-lab-root-v1.pem")
	mustOK(t, err)
	pool, err := LabPool(stored)
	if err != nil || pool == nil {
		t.Fatal("pinned root rejected")
	}
	return stored, pool
}

func testLeafTLS(t *testing.T, nextProtos []string) (*x509.CertPool, *tls.Config) {
	t.Helper()
	seed := sha256.Sum256([]byte("duskweave-https-lab-v1-public-synthetic-ca-key"))
	caPriv := ed25519.NewKeyFromSeed(seed[:])
	stored, pool := readCAAndPool(t)
	block, _ := pem.Decode(stored)
	caCert, err := x509.ParseCertificate(block.Bytes)
	mustOK(t, err)
	pub, leafPriv, err := ed25519.GenerateKey(rand.Reader)
	mustOK(t, err)
	tmpl := &x509.Certificate{
		SerialNumber: big.NewInt(2), Subject: pkix.Name{CommonName: frame.LabHost}, DNSNames: []string{frame.LabHost},
		NotBefore: time.Now().Add(-time.Hour), NotAfter: time.Now().Add(time.Hour),
		KeyUsage: x509.KeyUsageDigitalSignature, ExtKeyUsage: []x509.ExtKeyUsage{x509.ExtKeyUsageServerAuth},
	}
	der, err := x509.CreateCertificate(rand.Reader, tmpl, caCert, pub, caPriv)
	mustOK(t, err)
	return pool, &tls.Config{Certificates: []tls.Certificate{{Certificate: [][]byte{der}, PrivateKey: leafPriv}}, NextProtos: nextProtos}
}

func runCaptureAndExporterPair(t *testing.T, q frame.HttpsRequest, deps httpsDeps) frame.SafeResult {
	t.Helper()
	capCtrlR, capCtrlW := io.Pipe()
	rawR, rawW := io.Pipe()
	expInR, expInW := io.Pipe()
	safeR, safeW := io.Pipe()
	capDone, expDone := make(chan error, 1), make(chan error, 1)
	go func() {
		r, w := frame.Reader{Source: capCtrlR}, frame.Writer{Sink: rawW}
		if p, err := r.Take(frame.Limit); err != nil {
			capDone <- err
		} else {
			r.Count++
			capDone <- runHTTPSWithDeps(&r, &w, p, deps)
		}
		_ = rawW.Close()
	}()
	go func() { expDone <- export.Run(expInR, safeW); _ = safeW.Close() }()
	cc, cr, ec, sr := frame.Writer{Sink: capCtrlW}, frame.Reader{Source: rawR}, frame.Writer{Sink: expInW}, frame.Reader{Source: safeR}
	if cc.Write(q, frame.Limit) != nil || ec.Write(q, frame.Limit) != nil {
		t.Fatal("request write")
	}
	seqMap := map[string]uint32{"connect": 2, "tls": 4, "head": 6}
	var res frame.SafeResult
	for {
		p, err := cr.Take(frame.Limit)
		mustOK(t, err)
		cr.Count++
		if kind, _ := frame.Field(p, "kind"); kind == "READY" {
			var er, rdy frame.PhaseReady
			if frame.Decode(p, &rdy) != nil || (rdy.Phase == "connect" && sr.Read(&er, frame.ControlLimit) != nil) {
				t.Fatal("READY decode")
			}
			st := frame.PhaseStart{Identity: frame.HttpsID(q.Identity, "START", seqMap[rdy.Phase]), Phase: rdy.Phase, Deadline: q.Deadline}
			if ec.Write(st, frame.ControlLimit) != nil || cc.Write(st, frame.ControlLimit) != nil {
				t.Fatal("START write")
			}
			continue
		} else if kind != "RESULT" {
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
	mustOK(t, <-capDone)
	mustOK(t, <-expDone)
	return res
}

func tlsDialPair(cfg *tls.Config, onHead func(*spyCapConn)) (func(string, uint16, time.Time) (net.Conn, error), **spyCapConn) {
	var spy *spyCapConn
	return func(string, uint16, time.Time) (net.Conn, error) {
		c, s := net.Pipe()
		spy = &spyCapConn{Conn: c}
		go func() {
			ts := tls.Server(s, cfg)
			defer ts.Close()
			if ts.Handshake() == nil {
				_, _ = ts.Read(make([]byte, 256))
				onHead(spy)
			}
		}()
		return spy, nil
	}, &spy
}

func TestCaptureToExporterHeadCloseAndPostDeadlineTimeoutF3F4(t *testing.T) {
	now := time.Now()
	q := sampleCaptureReq(now, frame.LabProfile)
	pool, tlsCfg := testLeafTLS(t, []string{"http/1.1"})
	dialClose, _ := tlsDialPair(tlsCfg, func(*spyCapConn) {})
	resConnFail := runCaptureAndExporterPair(t, q, httpsDeps{
		now: time.Now, loadPool: func(string) (*x509.CertPool, error) { return pool, nil }, dial: dialClose,
	})
	if resConnFail.Outcome != "connection_failed" {
		t.Fatalf("F3 E2E: expected connection_failed, got %+v", resConnFail)
	}
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

func TestCaptureTimeoutClosesDirectlyWithoutCloseNotifyF7(t *testing.T) {
	now := time.Now()
	q := sampleCaptureReq(now, frame.LabProfile)
	q.Duration = 120
	q.Deadline = now.Add(120 * time.Millisecond).UnixMilli()
	cutoff := time.UnixMilli(q.Deadline)
	pool, tlsCfg := testLeafTLS(t, []string{"http/1.1"})
	dialStall, spyPtr := tlsDialPair(tlsCfg, func(s *spyCapConn) {
		s.mu.Lock()
		s.afterHead = true
		s.mu.Unlock()
		time.Sleep(350 * time.Millisecond)
	})
	start := time.Now()
	res := runCaptureAndExporterPair(t, q, httpsDeps{
		now: time.Now, loadPool: func(string) (*x509.CertPool, error) { return pool, nil }, dial: dialStall,
	})
	if elapsed := time.Since(start); res.Outcome != "timed_out" || res.Observed < q.Deadline || res.Observed > q.Deadline+1000 || elapsed > 700*time.Millisecond {
		t.Fatalf("F7 E2E: outcome=%s obs=%d elapsed=%v", res.Outcome, res.Observed, elapsed)
	}
	spy := *spyPtr
	spy.mu.Lock()
	defer spy.mu.Unlock()
	if !spy.closed || spy.writes != 0 || spy.wdl != 0 {
		t.Fatalf("F7: expected direct Close without close_notify/writeDeadline (closed=%v writes=%d wdl=%d)", spy.closed, spy.writes, spy.wdl)
	}
	for _, wt := range spy.writeTimes {
		if wt.After(cutoff) {
			t.Fatalf("F7: post-cutoff write at %v > %v", wt, cutoff)
		}
	}
}
