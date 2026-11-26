package capture

import (
	"crypto/sha256"
	"crypto/x509"
	"encoding/base64"
	"encoding/hex"
	"encoding/pem"
	"errors"
	"net"
	"os"
	"sync"
	"time"

	"duskweave/boundary/internal/frame"
)

const RootVersion = "duskweave-https-lab-root-v1"
const RootSHA = "af8714b55edd3a0a47cf24a1b24a6e99d9026ed4dd074b5a972a60618103d1dc"
const RootPath = "/etc/duskweave/https-lab-root-v1.pem"

const rootPEM = "" +
	"-----BEGIN CERTIFICATE-----\n" +
	"MIIBMDCB46ADAgECAgEBMAUGAytlcDAuMSwwKgYDVQQDDCNEdXNrV2VhdmUgSFRU\n" +
	"UFMgTEFCIHN5bnRoZXRpYyBDQSB2MTAeFw0yNjAxMDEwMDAwMDBaFw0zNjAxMDEw\n" +
	"MDAwMDBaMC4xLDAqBgNVBAMMI0R1c2tXZWF2ZSBIVFRQUyBMQUIgc3ludGhldGlj\n" +
	"IENBIHYxMCowBQYDK2VwAyEA9S2rYOxlb05g+iGfpsz0lN4t3QstCmPeK6CvlbJx\n" +
	"6EajJjAkMBIGA1UdEwEB/wQIMAYBAf8CAQAwDgYDVR0PAQH/BAQDAgEGMAUGAytl\n" +
	"cANBAOhEstA3uNRD0FpTw+z2PqY888Nz7OuQq5cyIObzaeNARHQegUDx981yjloJ\n" +
	"yV2OaKbiFpepxxKR2ENft9CBUwc=\n" +
	"-----END CERTIFICATE-----\n"

func LabPool(stored []byte) (*x509.CertPool, error) {
	if string(stored) != rootPEM || len(stored) != 473 {
		return nil, errors.New("trust")
	}
	sum := sha256.Sum256(stored)
	if hex.EncodeToString(sum[:]) != RootSHA {
		return nil, errors.New("trust")
	}
	block, rest := pem.Decode(stored)
	if block == nil || len(rest) != 0 || block.Type != "CERTIFICATE" {
		return nil, errors.New("trust")
	}
	cert, err := x509.ParseCertificate(block.Bytes)
	if err != nil || cert.Subject.CommonName != "DuskWeave HTTPS LAB synthetic CA v1" {
		return nil, errors.New("trust")
	}
	pool := x509.NewCertPool()
	pool.AddCert(cert)
	return pool, nil
}

func LoadLabPool() (*x509.CertPool, error) {
	stored, err := os.ReadFile(RootPath)
	if err != nil {
		return nil, errors.New("trust")
	}
	return LabPool(stored)
}

type httpsDeps struct {
	now      func() time.Time
	loadPool func(string) (*x509.CertPool, error)
	dial     func(string, uint16, time.Time) (net.Conn, error)
}

func runHTTPS(control *frame.Reader, raw *frame.Writer, payload []byte) error {
	return runHTTPSWithDeps(control, raw, payload, httpsDeps{
		now:      time.Now,
		loadPool: loadTrustPool,
		dial:     dialPinned,
	})
}

func runHTTPSWithDeps(control *frame.Reader, raw *frame.Writer, payload []byte, deps httpsDeps) error {
	var q frame.HttpsRequest
	if frame.Decode(payload, &q) != nil || frame.ValidateHTTPS(q, deps.now()) != nil {
		return frame.ErrFrame
	}
	readyAt := deps.now()
	if raw.Write(frame.PhaseReady{Identity: frame.HttpsID(q.Identity, "READY", 1), Phase: "connect"}, frame.ControlLimit) != nil {
		return frame.ErrFrame
	}
	if !admitStart(control, q, "connect", 2, readyAt, deps.now) {
		return frame.ErrFrame
	}
	var preloadedPool *x509.CertPool
	if q.RunProfile == frame.PublicRunProfile {
		var err error
		preloadedPool, err = deps.loadPool(q.RunProfile)
		if err != nil || preloadedPool == nil {
			return writePrivate(raw, q, "connect", "execution_refused", "none", 0, "", deps.now)
		}
	}
	deadline := time.UnixMilli(q.Deadline)
	conn, err := deps.dial(q.IPv4, q.Port, deadline)
	if err != nil {
		return writePrivate(raw, q, "connect", dialStatus(err), "none", 0, "", deps.now)
	}
	var closeOnce sync.Once
	closeConn := func() {
		closeOnce.Do(func() {
			_ = conn.Close()
		})
	}
	defer closeConn()
	capped := &capConn{Conn: conn, left: int(q.Limits.NetworkInboundBytes)}
	readyAt = deps.now()
	if raw.Write(frame.PhaseReady{Identity: frame.HttpsID(q.Identity, "READY", 3), Phase: "tls"}, frame.ControlLimit) != nil {
		return frame.ErrFrame
	}
	if !admitStart(control, q, "tls", 4, readyAt, deps.now) {
		return frame.ErrFrame
	}
	pool := preloadedPool
	if pool == nil {
		pool, err = deps.loadPool(q.RunProfile)
		if err != nil || pool == nil {
			closeConn()
			return writePrivate(raw, q, "tls", "execution_refused", "none", uint32(capped.used), "", deps.now)
		}
	}
	tlsConn, status, category := handshake(capped, pool, q.Hostname, deadline)
	if status != "" {
		closeConn()
		return writePrivate(raw, q, "tls", status, category, uint32(capped.used), "", deps.now)
	}
	readyAt = deps.now()
	if raw.Write(frame.PhaseReady{Identity: frame.HttpsID(q.Identity, "READY", 5), Phase: "head"}, frame.ControlLimit) != nil {
		return frame.ErrFrame
	}
	if !admitStart(control, q, "head", 6, readyAt, deps.now) {
		return frame.ErrFrame
	}
	if err = writeHead(tlsConn, q.Hostname, deadline); err != nil {
		closeConn()
		return writePrivate(raw, q, "head", ioStatus(err), "none", uint32(capped.used), "", deps.now)
	}
	header, status := readHeader(tlsConn, int(q.Limits.HeaderBytes), deadline)
	if status != "" {
		closeConn()
		return writePrivate(raw, q, "head", status, "none", uint32(capped.used), "", deps.now)
	}
	_, _, status = classifyHeader(header)
	encoded := ""
	if status == "received" {
		encoded = base64.StdEncoding.EncodeToString(header)
	}
	closeConn()
	return writePrivate(raw, q, "head", status, "none", uint32(capped.used), encoded, deps.now)
}

func admitStart(control *frame.Reader, q frame.HttpsRequest, phase string, seq uint32, readyAt time.Time, nowFn func() time.Time) bool {
	var start frame.PhaseStart
	if control.Read(&start, frame.ControlLimit) != nil {
		return false
	}
	now := nowFn()
	return frame.SameAttempt(start.Identity, q.Identity) && start.Kind == "START" && start.Sequence == seq &&
		start.Phase == phase && start.Deadline == q.Deadline && now.Sub(readyAt) <= time.Second &&
		now.Before(time.UnixMilli(q.Deadline))
}

func writePrivate(raw *frame.Writer, q frame.HttpsRequest, phase, status, tlsCat string, network uint32, header string, nowFn func() time.Time) error {
	handleEnd, ok1 := frame.AddMs(q.Deadline, 1000)
	earliest, ok2 := frame.AddMs(q.Deadline, -int64(q.Duration))
	if !ok1 || !ok2 || network > q.Limits.NetworkInboundBytes {
		return frame.ErrFrame
	}
	observed := nowFn().UnixMilli()
	if observed < earliest || observed > handleEnd {
		return frame.ErrFrame
	}
	if status != "tls_failed" {
		tlsCat = "none"
	}
	if status != "received" {
		header = ""
	}
	if observed > q.Deadline {
		if status == "execution_refused" {
			return frame.ErrFrame
		}
		status, tlsCat, header = "timed_out", "none", ""
	}
	return raw.Write(frame.PrivateResult{
		Identity: frame.HttpsID(q.Identity, "RESULT", 7), Phase: phase, Status: status,
		Observed: observed, Network: network, Header: header, TLS: tlsCat,
	}, frame.Limit)
}

func dialStatus(err error) string {
	if ne, ok := err.(net.Error); ok && ne.Timeout() {
		return "timed_out"
	}
	return "connection_failed"
}

func ioStatus(err error) string {
	if errors.Is(err, errNetCap) {
		return "network_limit"
	}
	if ne, ok := err.(net.Error); ok && ne.Timeout() {
		return "timed_out"
	}
	return "connection_failed"
}
