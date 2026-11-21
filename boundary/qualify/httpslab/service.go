package main

import (
	"bufio"
	"crypto/tls"
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

// Counters stores strictly non-secret atomic counts for one synthetic LAB service run.
type Counters struct {
	Scenario      string `json:"scenario"`
	Connections   uint64 `json:"connections"`
	TLSHandshakes uint64 `json:"tls_handshakes"`
	HeadRequests  uint64 `json:"head_requests"`
}

// Service hosts one bounded TLS+HTTP/1.1 synthetic endpoint on a pinned socket.
type Service struct {
	listener     net.Listener
	tlsCfg       *tls.Config
	scenario     string
	counterPath  string
	timeoutHold  time.Duration
	connections  atomic.Uint64
	handshakes   atomic.Uint64
	headRequests atomic.Uint64
	writeMu      sync.Mutex
	wg           sync.WaitGroup
}

// Start binds listenAddr and prepares the synthetic LAB responder without ambient state.
func Start(listenAddr, scenario, counterPath string) (*Service, error) {
	if !IsValidScenario(scenario) {
		return nil, errors.New("invalid_scenario")
	}
	tlsCfg, err := buildTLSConfig(scenario)
	if err != nil {
		return nil, err
	}
	ln, err := net.Listen("tcp4", listenAddr)
	if err != nil {
		return nil, err
	}
	svc := &Service{
		listener:    ln,
		tlsCfg:      tlsCfg,
		scenario:    scenario,
		counterPath: counterPath,
		timeoutHold: 8 * time.Second,
	}
	if err := svc.flushCounters(); err != nil {
		_ = ln.Close()
		return nil, err
	}
	return svc, nil
}

// Addr returns the concrete bound TCP address.
func (s *Service) Addr() string {
	return s.listener.Addr().String()
}

// Snapshot returns safe connection, TLS handshake, and HEAD request counters.
func (s *Service) Snapshot() Counters {
	return Counters{
		Scenario:      s.scenario,
		Connections:   s.connections.Load(),
		TLSHandshakes: s.handshakes.Load(),
		HeadRequests:  s.headRequests.Load(),
	}
}

// Serve accepts incoming connections until the listener closes.
func (s *Service) Serve() {
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

// Close stops accepting connections, waits for active handlers, and flushes counters.
func (s *Service) Close() error {
	err := s.listener.Close()
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
