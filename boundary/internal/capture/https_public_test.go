package capture

import (
	"bytes"
	"crypto/ed25519"
	"crypto/rand"
	"crypto/sha256"
	"crypto/tls"
	"crypto/x509"
	"crypto/x509/pkix"
	"duskweave/boundary/internal/frame"
	"encoding/hex"
	"encoding/json"
	"encoding/pem"
	"errors"
	"io"
	"math/big"
	"net"
	"os"
	"path/filepath"
	"sync"
	"testing"
	"time"
)

func samplePublicCaptureReq(now time.Time) frame.HttpsRequest {
	q := sampleCaptureReq(now, frame.LabProfile)
	q.RunProfile = frame.PublicRunProfile
	q.Hostname = "probe.example.invalid"
	q.IPv4 = "93.184.216.34"
	q.Resolver = "1.1.1.1"
	return q
}

func publicSyntheticCAAndLeaf(t *testing.T, host string, notAfter time.Time, trusted bool) (*x509.CertPool, *tls.Config) {
	t.Helper()
	_, caPriv, err := ed25519.GenerateKey(rand.Reader)
	mustOK(t, err)
	caTmpl := &x509.Certificate{
		SerialNumber: big.NewInt(1), Subject: pkix.Name{CommonName: "Test Public Synthetic CA"},
		NotBefore: time.Now().Add(-time.Hour), NotAfter: time.Now().Add(time.Hour),
		IsCA: true, BasicConstraintsValid: true, KeyUsage: x509.KeyUsageCertSign,
	}
	caDER, err := x509.CreateCertificate(rand.Reader, caTmpl, caTmpl, caPriv.Public(), caPriv)
	mustOK(t, err)
	caCert, err := x509.ParseCertificate(caDER)
	mustOK(t, err)
	_, leafPriv, err := ed25519.GenerateKey(rand.Reader)
	mustOK(t, err)
	leafTmpl := &x509.Certificate{
		SerialNumber: big.NewInt(2), Subject: pkix.Name{CommonName: host}, DNSNames: []string{host},
		NotBefore: time.Now().Add(-2 * time.Hour), NotAfter: notAfter,
		KeyUsage: x509.KeyUsageDigitalSignature, ExtKeyUsage: []x509.ExtKeyUsage{x509.ExtKeyUsageServerAuth},
	}
	leafDER, err := x509.CreateCertificate(rand.Reader, leafTmpl, caCert, leafPriv.Public(), caPriv)
	mustOK(t, err)
	pool := x509.NewCertPool()
	if trusted {
		pool.AddCert(caCert)
	}
	return pool, &tls.Config{Certificates: []tls.Certificate{{Certificate: [][]byte{leafDER}, PrivateKey: leafPriv}}, NextProtos: []string{"http/1.1"}}
}

func TestPublicRootBundleAndTamperRefusal(t *testing.T) {
	raw, err := os.ReadFile("../../trust/cacert-2026-11-05.pem")
	mustOK(t, err)
	if len(raw) != PublicRootSize {
		t.Fatalf("unexpected PEM size %d", len(raw))
	}
	sum := sha256.Sum256(raw)
	if hex.EncodeToString(sum[:]) != PublicRootSHA {
		t.Fatalf("unexpected PEM sha256 %x", sum)
	}
	var prov struct {
		BundleVersion string `json:"bundle_version"`
		SHA256        string `json:"sha256"`
		SizeBytes     int    `json:"size_bytes"`
		CertCount     int    `json:"certificate_count"`
		ImagePath     string `json:"image_path"`
	}
	provRaw, err := os.ReadFile("../../trust/public-roots-v1.json")
	mustOK(t, err)
	mustOK(t, json.Unmarshal(provRaw, &prov))
	if prov.BundleVersion != PublicRootVersion || prov.SHA256 != PublicRootSHA ||
		prov.SizeBytes != PublicRootSize || prov.CertCount != PublicRootCertCount || prov.ImagePath != PublicRootPath {
		t.Fatalf("provenance mismatch: %+v", prov)
	}
	pool, err := PublicPool(raw)
	if err != nil || pool == nil {
		t.Fatalf("PublicPool rejected pinned bundle: %v", err)
	}
	tampered := bytes.Clone(raw)
	tampered[len(tampered)-2] ^= 0x01
	labPEM, err := os.ReadFile("../../fixtures/https-lab-root-v1.pem")
	mustOK(t, err)
	for name, bad := range map[string][]byte{
		"empty": nil, "truncated": raw[:len(raw)-1], "appended": append(bytes.Clone(raw), '\n'),
		"tampered": tampered, "lab_root": labPEM,
		"non_cert": pem.EncodeToMemory(&pem.Block{Type: "PUBLIC KEY", Bytes: []byte{1, 2, 3}}),
	} {
		if _, err := PublicPool(bad); err == nil {
			t.Fatalf("PublicPool accepted %s", name)
		}
	}
	dir := t.TempDir()
	exactPath := filepath.Join(dir, "exact.pem")
	mustOK(t, os.WriteFile(exactPath, raw, 0o444))
	if got, err := readBoundedFixedFile(exactPath, PublicRootSize); err != nil || len(got) != PublicRootSize {
		t.Fatalf("readBoundedFixedFile failed on exact file: %v", err)
	}
	overPath := filepath.Join(dir, "over.pem")
	mustOK(t, os.WriteFile(overPath, append(bytes.Clone(raw), 'x'), 0o444))
	if _, err := readBoundedFixedFile(overPath, PublicRootSize); err == nil {
		t.Fatal("readBoundedFixedFile accepted oversized file")
	}
	if _, err := readBoundedFixedFile(filepath.Join(dir, "missing.pem"), PublicRootSize); err == nil {
		t.Fatal("readBoundedFixedFile accepted missing file")
	}
	if _, err := loadTrustPool("unknown-profile"); err == nil {
		t.Fatal("loadTrustPool accepted unknown profile")
	}
}

func TestPublicPoolExcludesLabSyntheticRoot(t *testing.T) {
	raw, err := os.ReadFile("../../trust/cacert-2026-11-05.pem")
	mustOK(t, err)
	pubPool, err := PublicPool(raw)
	mustOK(t, err)
	_, labCfg := testLeafTLS(t, []string{"http/1.1"})
	c, s := net.Pipe()
	defer c.Close()
	go func() {
		ts := tls.Server(newAsyncPipeServerConn(s), labCfg)
		defer ts.Close()
		_ = ts.Handshake()
	}()
	_, status, cat := handshake(c, pubPool, frame.LabHost, time.Now().Add(2*time.Second))
	if status != "tls_failed" || cat != "unknown_root" {
		t.Fatalf("expected public pool to reject LAB root with tls_failed/unknown_root, got %s/%s", status, cat)
	}
}

func TestPublicPreDialRootRefusalAndPrePermitZeroDial(t *testing.T) {
	now := time.Now()
	q := samplePublicCaptureReq(now)
	dialCount := 0
	var badIn, badOut bytes.Buffer
	_ = (&frame.Writer{Sink: &badIn}).Write(q, frame.Limit)
	_ = (&frame.Writer{Sink: &badIn}).Write(frame.PhaseStart{
		Identity: frame.HttpsID(frame.Identity{Schema: frame.HttpsSchema, Attempt: "00000000-0000-0000-0000-000000000099", Generation: 1}, "START", 2),
		Phase:    "connect", Deadline: q.Deadline,
	}, frame.ControlLimit)
	r, w := frame.Reader{Source: &badIn}, frame.Writer{Sink: &badOut}
	p, err := r.Take(frame.Limit)
	mustOK(t, err)
	r.Count++
	if err := runHTTPSWithDeps(&r, &w, p, httpsDeps{
		now: time.Now, loadPool: func(string) (*x509.CertPool, error) { return nil, errors.New("trust") },
		dial: func(string, uint16, time.Time) (net.Conn, error) { dialCount++; return nil, errors.New("unexpected") },
	}); err == nil || dialCount != 0 {
		t.Fatalf("pre-permit wrong START did not fail with zero dial: err=%v dials=%d", err, dialCount)
	}

	dialCount = 0
	res := runCaptureAndExporterPair(t, q, httpsDeps{
		now:      time.Now,
		loadPool: func(string) (*x509.CertPool, error) { return nil, errors.New("missing_root") },
		dial:     func(string, uint16, time.Time) (net.Conn, error) { dialCount++; return nil, errors.New("dial_called") },
	})
	if dialCount != 0 || res.Outcome != "execution_refused" ||
		len(res.Limitations) != 2 || res.Limitations[0] != "public_https_single_endpoint_not_whole_host_or_objective" {
		t.Fatalf("expected zero-dial execution_refused with public scope, dials=%d res=%+v", dialCount, res)
	}
}

type asyncPipeServerConn struct {
	net.Conn
	mu     sync.Mutex
	ch     chan []byte
	closed bool
	once   sync.Once
}

func newAsyncPipeServerConn(c net.Conn) *asyncPipeServerConn {
	a := &asyncPipeServerConn{Conn: c, ch: make(chan []byte, 64)}
	go func() {
		defer c.Close()
		for b := range a.ch {
			if _, err := c.Write(b); err != nil {
				return
			}
		}
	}()
	return a
}

func (a *asyncPipeServerConn) Write(p []byte) (int, error) {
	a.mu.Lock()
	defer a.mu.Unlock()
	if a.closed {
		return 0, io.ErrClosedPipe
	}
	a.ch <- append([]byte(nil), p...)
	return len(p), nil
}

func (a *asyncPipeServerConn) Close() error {
	a.once.Do(func() {
		a.mu.Lock()
		a.closed = true
		close(a.ch)
		a.mu.Unlock()
	})
	return nil
}

func (a *asyncPipeServerConn) SetDeadline(time.Time) error      { return nil }
func (a *asyncPipeServerConn) SetWriteDeadline(time.Time) error { return nil }

func publicTLSDialPair(cfg *tls.Config, onHead func(*tls.Conn, *spyCapConn)) (func(string, uint16, time.Time) (net.Conn, error), **spyCapConn) {
	var spy *spyCapConn
	return func(string, uint16, time.Time) (net.Conn, error) {
		c, s := net.Pipe()
		spy = &spyCapConn{Conn: c}
		go func() {
			ts := tls.Server(newAsyncPipeServerConn(s), cfg)
			defer ts.Close()
			if ts.Handshake() == nil {
				_, _ = ts.Read(make([]byte, 256))
				onHead(ts, spy)
			}
		}()
		return spy, nil
	}, &spy
}

func TestPublicTLSAndHeadOutcomesViaPrivateDeps(t *testing.T) {
	now := time.Now()
	q := samplePublicCaptureReq(now)
	pool, cfg200 := publicSyntheticCAAndLeaf(t, q.Hostname, now.Add(time.Hour), true)
	dial200, _ := publicTLSDialPair(cfg200, func(ts *tls.Conn, _ *spyCapConn) {
		_, _ = ts.Write([]byte("HTTP/1.1 200 OK\r\nX-Raw: PUBLIC_RAW_SENTINEL\r\n\r\n"))
	})
	res200 := runCaptureAndExporterPair(t, samplePublicCaptureReq(time.Now()), httpsDeps{
		now: time.Now, loadPool: func(string) (*x509.CertPool, error) { return pool, nil }, dial: dial200,
	})
	raw200, _ := json.Marshal(res200)
	if res200.Outcome != "head_response" || res200.StatusCode != 200 || res200.Redirect != "none" ||
		res200.Limitations[0] != "public_https_single_endpoint_not_whole_host_or_objective" ||
		bytes.Contains(raw200, []byte("PUBLIC_RAW_SENTINEL")) {
		t.Fatalf("public 200 failed: %+v", res200)
	}

	dial302, _ := publicTLSDialPair(cfg200, func(ts *tls.Conn, _ *spyCapConn) {
		_, _ = ts.Write([]byte("HTTP/1.1 302 Found\r\nLocation: https://evil.example/LOC_SENTINEL\r\n\r\n"))
	})
	res302 := runCaptureAndExporterPair(t, samplePublicCaptureReq(time.Now()), httpsDeps{
		now: time.Now, loadPool: func(string) (*x509.CertPool, error) { return pool, nil }, dial: dial302,
	})
	raw302, _ := json.Marshal(res302)
	if res302.Outcome != "head_response" || res302.StatusCode != 302 || res302.Redirect != "refused_redirect" ||
		bytes.Contains(raw302, []byte("LOC_SENTINEL")) {
		t.Fatalf("public 302 failed: %+v", res302)
	}

	wrongHostPool, wrongHostCfg := publicSyntheticCAAndLeaf(t, "other.example.invalid", now.Add(time.Hour), true)
	dialWrongHost, _ := publicTLSDialPair(wrongHostCfg, func(*tls.Conn, *spyCapConn) {})
	resHost := runCaptureAndExporterPair(t, samplePublicCaptureReq(time.Now()), httpsDeps{
		now: time.Now, loadPool: func(string) (*x509.CertPool, error) { return wrongHostPool, nil }, dial: dialWrongHost,
	})
	if resHost.Outcome != "tls_failed" || resHost.TLS != "hostname_mismatch" ||
		len(resHost.Limitations) != 3 || resHost.Limitations[2] != "tls_hostname_mismatch" {
		t.Fatalf("wrong host failed: %+v", resHost)
	}

	_, untrustedCfg := publicSyntheticCAAndLeaf(t, q.Hostname, now.Add(time.Hour), false)
	otherRoot, _ := publicSyntheticCAAndLeaf(t, "unrelated.example.invalid", now.Add(time.Hour), true)
	dialUntrusted, _ := publicTLSDialPair(untrustedCfg, func(*tls.Conn, *spyCapConn) {})
	resRoot := runCaptureAndExporterPair(t, samplePublicCaptureReq(time.Now()), httpsDeps{
		now: time.Now, loadPool: func(string) (*x509.CertPool, error) { return otherRoot, nil }, dial: dialUntrusted,
	})
	if resRoot.Outcome != "tls_failed" || resRoot.TLS != "unknown_root" {
		t.Fatalf("unknown root failed: %+v", resRoot)
	}

	expPool, expCfg := publicSyntheticCAAndLeaf(t, q.Hostname, now.Add(-time.Minute), true)
	dialExp, _ := publicTLSDialPair(expCfg, func(*tls.Conn, *spyCapConn) {})
	resExp := runCaptureAndExporterPair(t, samplePublicCaptureReq(time.Now()), httpsDeps{
		now: time.Now, loadPool: func(string) (*x509.CertPool, error) { return expPool, nil }, dial: dialExp,
	})
	if resExp.Outcome != "tls_failed" || resExp.TLS != "expired_or_not_yet_valid" {
		t.Fatalf("expired cert failed: %+v", resExp)
	}

	dialClose, _ := publicTLSDialPair(cfg200, func(*tls.Conn, *spyCapConn) {})
	resClose := runCaptureAndExporterPair(t, samplePublicCaptureReq(time.Now()), httpsDeps{
		now: time.Now, loadPool: func(string) (*x509.CertPool, error) { return pool, nil }, dial: dialClose,
	})
	if resClose.Outcome != "connection_failed" {
		t.Fatalf("head close failed: %+v", resClose)
	}

	shortNow := time.Now()
	qShort := samplePublicCaptureReq(shortNow)
	qShort.Duration = 120
	qShort.Deadline = shortNow.Add(120 * time.Millisecond).UnixMilli()
	dialStall, spyPtr := publicTLSDialPair(cfg200, func(_ *tls.Conn, s *spyCapConn) {
		s.mu.Lock()
		s.afterHead = true
		s.mu.Unlock()
		time.Sleep(300 * time.Millisecond)
	})
	resTO := runCaptureAndExporterPair(t, qShort, httpsDeps{
		now: time.Now, loadPool: func(string) (*x509.CertPool, error) { return pool, nil }, dial: dialStall,
	})
	spy := *spyPtr
	spy.mu.Lock()
	defer spy.mu.Unlock()
	if resTO.Outcome != "timed_out" || !spy.closed || spy.writes != 0 || spy.wdl != 0 {
		t.Fatalf("public timeout close failed: res=%+v closed=%v writes=%d wdl=%d", resTO, spy.closed, spy.writes, spy.wdl)
	}
}
