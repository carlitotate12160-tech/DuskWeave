package main

import (
	"bufio"
	"crypto/tls"
	"duskweave/boundary/internal/dns"
	"encoding/binary"
	"encoding/json"
	"errors"
	"net"
	"net/http"
	"os"
	"path/filepath"
	"strings"
	"sync"
	"sync/atomic"
	"time"
)

var validScenarios = map[string]bool{
	"200":              true,
	"302":              true,
	"connection_close": true,
	"wrong_hostname":   true,
	"wrong_root":       true,
	"expired":          true,
	"http2":            true,
	"oversize_header":  true,
	"timeout":          true,
}

// IsValidScenario reports whether name belongs to the closed HTTPS LAB scenario set.
func IsValidScenario(name string) bool {
	return validScenarios[name]
}

// Options configures the synthetic HTTPS LAB TLS/HEAD and exact DNS responder.
type Options struct {
	ListenAddr    string
	DNSListenAddr string
	Scenario      string
	CounterPath   string
	RunID         string
	DNSTTL        uint32
	DNSRefreshTTL uint32
}

// Counters stores strictly non-secret atomic counts for one synthetic LAB service run.
type Counters struct {
	RunID         string `json:"run_id,omitempty"`
	Scenario      string `json:"scenario"`
	DNSQueries    uint64 `json:"dns_queries"`
	DNSAnswered   uint64 `json:"dns_answered"`
	DNSRefused    uint64 `json:"dns_refused"`
	Connections   uint64 `json:"connections"`
	TLSHandshakes uint64 `json:"tls_handshakes"`
	HeadRequests  uint64 `json:"head_requests"`
}

// Service hosts one bounded TLS+HTTP/1.1 synthetic endpoint and optional exact UDP DNS responder.
type Service struct {
	listener      net.Listener
	dnsConn       *net.UDPConn
	tlsCfg        *tls.Config
	scenario      string
	counterPath   string
	runID         string
	dnsTTL        uint32
	dnsRefreshTTL uint32
	timeoutHold   time.Duration
	dnsQueries    atomic.Uint64
	dnsAnswered   atomic.Uint64
	dnsRefused    atomic.Uint64
	connections   atomic.Uint64
	handshakes    atomic.Uint64
	headRequests  atomic.Uint64
	writeMu       sync.Mutex
	wg            sync.WaitGroup
}

func validRunID(id string) bool {
	if len(id) > 64 {
		return false
	}
	for _, b := range []byte(id) {
		if !((b >= 'a' && b <= 'z') || (b >= 'A' && b <= 'Z') || (b >= '0' && b <= '9') || b == '-' || b == '_') {
			return false
		}
	}
	return true
}

// Start binds listenAddr and prepares the synthetic LAB TLS/HEAD responder without ambient state.
func Start(listenAddr, scenario, counterPath string) (*Service, error) {
	return StartWithOptions(Options{
		ListenAddr:  listenAddr,
		Scenario:    scenario,
		CounterPath: counterPath,
		DNSTTL:      30,
	})
}

// StartWithOptions binds the synthetic TLS/HEAD listener and optional exact UDP A/IN DNS listener.
func StartWithOptions(opts Options) (*Service, error) {
	if !IsValidScenario(opts.Scenario) {
		return nil, errors.New("invalid_scenario")
	}
	if !validRunID(opts.RunID) {
		return nil, errors.New("invalid_run_id")
	}
	ttl := opts.DNSTTL
	refreshTTL := opts.DNSRefreshTTL
	if opts.DNSListenAddr != "" {
		if refreshTTL == 0 {
			refreshTTL = ttl
		}
		if ttl < 1 || ttl > 300 || refreshTTL < 1 || refreshTTL > 300 {
			return nil, errors.New("invalid_dns_ttl")
		}
	} else if ttl > 300 || refreshTTL > 300 {
		return nil, errors.New("invalid_dns_ttl")
	}
	tlsCfg, err := buildTLSConfig(opts.Scenario)
	if err != nil {
		return nil, err
	}
	ln, err := net.Listen("tcp4", opts.ListenAddr)
	if err != nil {
		return nil, err
	}
	var dnsConn *net.UDPConn
	if opts.DNSListenAddr != "" {
		udpAddr, err := net.ResolveUDPAddr("udp4", opts.DNSListenAddr)
		if err != nil {
			_ = ln.Close()
			return nil, err
		}
		dnsConn, err = net.ListenUDP("udp4", udpAddr)
		if err != nil {
			_ = ln.Close()
			return nil, err
		}
	}
	svc := &Service{
		listener:      ln,
		dnsConn:       dnsConn,
		tlsCfg:        tlsCfg,
		scenario:      opts.Scenario,
		counterPath:   opts.CounterPath,
		runID:         opts.RunID,
		dnsTTL:        ttl,
		dnsRefreshTTL: refreshTTL,
		timeoutHold:   8 * time.Second,
	}
	if err := svc.flushCounters(); err != nil {
		_ = ln.Close()
		if dnsConn != nil {
			_ = dnsConn.Close()
		}
		return nil, err
	}
	return svc, nil
}

// Addr returns the concrete bound TCP address.
func (s *Service) Addr() string {
	return s.listener.Addr().String()
}

// DNSAddr returns the concrete bound UDP DNS address, or empty when DNS is disabled.
func (s *Service) DNSAddr() string {
	if s.dnsConn == nil {
		return ""
	}
	return s.dnsConn.LocalAddr().String()
}

// Snapshot returns safe DNS, connection, TLS handshake, and HEAD request counters.
func (s *Service) Snapshot() Counters {
	return Counters{
		RunID:         s.runID,
		Scenario:      s.scenario,
		DNSQueries:    s.dnsQueries.Load(),
		DNSAnswered:   s.dnsAnswered.Load(),
		DNSRefused:    s.dnsRefused.Load(),
		Connections:   s.connections.Load(),
		TLSHandshakes: s.handshakes.Load(),
		HeadRequests:  s.headRequests.Load(),
	}
}

// Serve accepts incoming connections and optional DNS UDP packets until closed.
func (s *Service) Serve() {
	if s.dnsConn != nil {
		s.wg.Add(1)
		go func() {
			defer s.wg.Done()
			s.serveDNS()
		}()
	}
	for {
		conn, err := s.listener.Accept()
		if err != nil {
			return
		}
		s.connections.Add(1)
		s.wg.Add(1)
		go func(c net.Conn) {
			defer s.wg.Done()
			defer c.Close()
			s.handleConn(c)
			_ = s.flushCounters()
		}(conn)
	}
}

func (s *Service) serveDNS() {
	buf := make([]byte, 512)
	for {
		n, peer, err := s.dnsConn.ReadFromUDP(buf)
		if err != nil {
			return
		}
		s.dnsQueries.Add(1)
		if reply := s.handleDNSQuery(buf[:n]); reply != nil {
			_, _ = s.dnsConn.WriteToUDP(reply, peer)
		}
		_ = s.flushCounters()
	}
}

func (s *Service) handleDNSQuery(pkt []byte) []byte {
	if len(pkt) < 12 || len(pkt) > 512 {
		s.dnsRefused.Add(1)
		return nil
	}
	if isExactLabAQuery(pkt) {
		n := s.dnsAnswered.Add(1)
		ttl := s.dnsTTL
		if n > 1 {
			ttl = s.dnsRefreshTTL
		}
		resp := make([]byte, len(pkt)+16)
		copy(resp, pkt)
		resp[2] = 0x80 | (pkt[2] & 0x01)
		resp[3] = 0x80
		resp[7] = 1
		off := len(pkt)
		copy(resp[off:off+6], []byte{0xc0, 0x0c, 0, 1, 0, 1})
		binary.BigEndian.PutUint32(resp[off+6:off+10], ttl)
		copy(resp[off+10:off+16], []byte{0, 4, 198, 51, 100, 10})
		return resp
	}
	s.dnsRefused.Add(1)
	resp := make([]byte, 12)
	copy(resp[:2], pkt[:2])
	resp[2] = 0x80 | (pkt[2] & 0x01)
	resp[3] = 0x85
	return resp
}

func isExactLabAQuery(pkt []byte) bool {
	if (pkt[2]&0xfe) != 0 || binary.BigEndian.Uint16(pkt[4:6]) != 1 ||
		binary.BigEndian.Uint16(pkt[6:8]) != 0 || binary.BigEndian.Uint16(pkt[8:10]) != 0 ||
		binary.BigEndian.Uint16(pkt[10:12]) != 0 {
		return false
	}
	want, err := dns.Query(binary.BigEndian.Uint16(pkt[:2]), labHostname)
	return err == nil && len(pkt) == len(want) && string(pkt[12:]) == string(want[12:])
}

// Close stops accepting connections and DNS packets, waits for active handlers, and flushes counters.
func (s *Service) Close() error {
	err := s.listener.Close()
	if s.dnsConn != nil {
		if dErr := s.dnsConn.Close(); err == nil {
			err = dErr
		}
	}
	s.wg.Wait()
	if fErr := s.flushCounters(); err == nil {
		err = fErr
	}
	return err
}

func (s *Service) handleConn(raw net.Conn) {
	_ = raw.SetDeadline(time.Now().Add(10 * time.Second))
	tconn := tls.Server(raw, s.tlsCfg)
	if err := tconn.Handshake(); err != nil {
		return
	}
	s.handshakes.Add(1)
	defer tconn.Close()

	if s.scenario == "http2" {
		_, _ = tconn.Write([]byte("HTTP/2.0 200 OK\r\nContent-Length: 0\r\n\r\n"))
		return
	}
	req, err := http.ReadRequest(bufio.NewReader(tconn))
	if err != nil {
		return
	}
	_ = req.Body.Close()
	s.headRequests.Add(1)
	_ = s.flushCounters()
	if req.Method != http.MethodHead {
		_, _ = tconn.Write([]byte("HTTP/1.1 405 Method Not Allowed\r\nContent-Length: 0\r\n\r\n"))
		return
	}
	if req.URL.Path != "/" {
		_, _ = tconn.Write([]byte("HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n"))
		return
	}
	s.respondHead(tconn)
}

func (s *Service) respondHead(tconn net.Conn) {
	switch s.scenario {
	case "200":
		_, _ = tconn.Write([]byte("HTTP/1.1 200 OK\r\nContent-Length: 0\r\nX-DuskWeave-Lab: ok\r\n\r\n"))
	case "302":
		_, _ = tconn.Write([]byte("HTTP/1.1 302 Found\r\nLocation: https://other.duskweave.test/\r\nContent-Length: 0\r\n\r\n"))
	case "connection_close":
		return
	case "oversize_header":
		pad := strings.Repeat("A", 9000)
		_, _ = tconn.Write([]byte("HTTP/1.1 200 OK\r\nContent-Length: 0\r\nX-Pad: " + pad + "\r\n\r\n"))
	case "timeout":
		time.Sleep(s.timeoutHold)
	}
}

func (s *Service) flushCounters() error {
	if s.counterPath == "" {
		return nil
	}
	s.writeMu.Lock()
	defer s.writeMu.Unlock()
	raw, err := json.Marshal(s.Snapshot())
	if err != nil {
		return err
	}
	raw = append(raw, '\n')
	tmp := s.counterPath + ".tmp"
	if err := os.MkdirAll(filepath.Dir(s.counterPath), 0o755); err != nil {
		return err
	}
	if err := os.WriteFile(tmp, raw, 0o600); err != nil {
		return err
	}
	return os.Rename(tmp, s.counterPath)
}
