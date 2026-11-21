package capture

import (
	"bytes"
	"crypto/tls"
	"crypto/x509"
	"errors"
	"io"
	"net"
	"strconv"
	"strings"
	"time"
)

var errNetCap = errors.New("network_limit")

type capConn struct {
	net.Conn
	left    int
	used    int
	tripped bool
}

func (c *capConn) Read(p []byte) (int, error) {
	if len(p) == 0 {
		return 0, nil
	}
	if c.left <= 0 {
		var one [1]byte
		n, err := c.Conn.Read(one[:])
		if n > 0 || (err != nil && !errors.Is(err, io.EOF)) {
			c.tripped = true
			return 0, errNetCap
		}
		return 0, io.EOF
	}
	if len(p) > c.left {
		p = p[:c.left]
	}
	n, err := c.Conn.Read(p)
	if n > 0 {
		c.left -= n
		c.used += n
	}
	return n, err
}

func dialPinned(ip string, port uint16, deadline time.Time) (net.Conn, error) {
	dialer := net.Dialer{Deadline: deadline}
	return dialer.Dial("tcp4", net.JoinHostPort(ip, strconv.Itoa(int(port))))
}

func handshake(conn net.Conn, pool *x509.CertPool, hostname string, deadline time.Time) (*tls.Conn, string, string) {
	if conn.SetDeadline(deadline) != nil {
		return nil, "timed_out", "none"
	}
	tlsConn := tls.Client(conn, &tls.Config{ServerName: hostname, RootCAs: pool, MinVersion: tls.VersionTLS12, NextProtos: []string{"http/1.1"}})
	if err := tlsConn.Handshake(); err != nil {
		if errors.Is(err, errNetCap) {
			return nil, "network_limit", "none"
		}
		if ne, ok := err.(net.Error); ok && ne.Timeout() {
			return nil, "timed_out", "none"
		}
		return nil, "tls_failed", tlsCategory(err)
	}
	if proto := tlsConn.ConnectionState().NegotiatedProtocol; proto != "" && proto != "http/1.1" {
		return nil, "unsupported_http", "none"
	}
	return tlsConn, "", "none"
}

func tlsCategory(err error) string {
	var unknown x509.UnknownAuthorityError
	var host x509.HostnameError
	var invalid x509.CertificateInvalidError
	switch {
	case errors.As(err, &unknown):
		return "unknown_root"
	case errors.As(err, &host):
		return "hostname_mismatch"
	case errors.As(err, &invalid):
		if invalid.Reason == x509.Expired {
			return "expired_or_not_yet_valid"
		}
		if invalid.Reason == x509.IncompatibleUsage {
			return "protocol"
		}
		return "other_validation"
	default:
		var alert tls.AlertError
		if errors.As(err, &alert) {
			return "protocol"
		}
		return "other_validation"
	}
}

func writeHead(conn net.Conn, host string, deadline time.Time) error {
	if err := conn.SetDeadline(deadline); err != nil {
		return err
	}
	body := "HEAD / HTTP/1.1\r\nHost: " + host + "\r\nConnection: close\r\n\r\n"
	n, err := io.WriteString(conn, body)
	if err != nil || n != len(body) {
		if err != nil {
			return err
		}
		return io.ErrShortWrite
	}
	return nil
}

func readHeader(conn net.Conn, max int, deadline time.Time) ([]byte, string) {
	if conn.SetDeadline(deadline) != nil {
		return nil, "timed_out"
	}
	buf := make([]byte, 0, max)
	one := make([]byte, 1)
	for {
		if bytes.HasSuffix(buf, []byte("\r\n\r\n")) && len(buf) <= max {
			return buf, ""
		}
		if len(buf) >= max {
			return nil, "header_limit"
		}
		n, err := conn.Read(one)
		if n == 1 {
			buf = append(buf, one[0])
		}
		if err != nil {
			if errors.Is(err, errNetCap) {
				return nil, "network_limit"
			}
			if ne, ok := err.(net.Error); ok && ne.Timeout() {
				return nil, "timed_out"
			}
			if len(buf) == 0 {
				return nil, "connection_failed"
			}
			return nil, "malformed_response"
		}
	}
}

func isTchar(b byte) bool {
	if b >= 'a' && b <= 'z' || b >= 'A' && b <= 'Z' || b >= '0' && b <= '9' {
		return true
	}
	switch b {
	case '!', '#', '$', '%', '&', '\'', '*', '+', '-', '.', '^', '_', '`', '|', '~':
		return true
	}
	return false
}

func validHeaderText(b []byte) bool {
	for _, c := range b {
		if c == '\r' || c == '\n' || c == 0x7f || (c < 0x20 && c != '\t') {
			return false
		}
	}
	return true
}

func classifyHeader(header []byte) (uint16, string, string) {
	if len(header) == 0 || len(header) > 8192 || bytes.IndexByte(header, 0) >= 0 {
		return 0, "none", "malformed_response"
	}
	end := bytes.Index(header, []byte("\r\n\r\n"))
	if end < 0 || end+4 != len(header) {
		return 0, "none", "malformed_response"
	}
	lines := bytes.Split(header[:end], []byte("\r\n"))
	if len(lines) == 0 {
		return 0, "none", "malformed_response"
	}
	status := lines[0]
	if bytes.HasPrefix(status, []byte("HTTP/2")) || bytes.HasPrefix(status, []byte("HTTP/3")) ||
		(bytes.HasPrefix(status, []byte("HTTP/")) && !bytes.HasPrefix(status, []byte("HTTP/1.1 "))) {
		return 0, "none", "unsupported_http"
	}
	if !bytes.HasPrefix(status, []byte("HTTP/1.1 ")) || len(status) < 13 || status[12] != ' ' || !validHeaderText(status[13:]) {
		return 0, "none", "malformed_response"
	}
	for i := 9; i < 12; i++ {
		if status[i] < '0' || status[i] > '9' {
			return 0, "none", "malformed_response"
		}
	}
	code, err := strconv.Atoi(string(status[9:12]))
	if err != nil || code < 100 || code > 599 {
		return 0, "none", "malformed_response"
	}
	if code < 200 {
		return 0, "none", "unsupported_http"
	}
	var clVal string
	var hasCL, hasTE bool
	for _, line := range lines[1:] {
		if len(line) == 0 || line[0] == ' ' || line[0] == '\t' || bytes.ContainsAny(line, "\r\n") {
			return 0, "none", "malformed_response"
		}
		colon := bytes.IndexByte(line, ':')
		if colon <= 0 {
			return 0, "none", "malformed_response"
		}
		name, rawVal := line[:colon], line[colon+1:]
		for _, c := range name {
			if !isTchar(c) {
				return 0, "none", "malformed_response"
			}
		}
		val := bytes.Trim(rawVal, " \t")
		if !validHeaderText(val) {
			return 0, "none", "malformed_response"
		}
		lower := strings.ToLower(string(name))
		switch lower {
		case "content-length":
			hasCL = true
			if !validContentLength(string(val), &clVal) {
				return 0, "none", "malformed_response"
			}
		case "transfer-encoding":
			hasTE = true
			if !validTransferEncoding(string(val)) {
				return 0, "none", "malformed_response"
			}
		}
	}
	if hasCL && hasTE {
		return 0, "none", "malformed_response"
	}
	if code >= 300 && code < 400 {
		return uint16(code), "refused_redirect", "received"
	}
	return uint16(code), "none", "received"
}

func validContentLength(raw string, seen *string) bool {
	parts := strings.Split(raw, ",")
	for _, p := range parts {
		s := strings.Trim(p, " \t")
		if s == "" {
			return false
		}
		for _, b := range []byte(s) {
			if b < '0' || b > '9' {
				return false
			}
		}
		if *seen == "" {
			*seen = s
		} else if *seen != s {
			return false
		}
	}
	return true
}

func validTransferEncoding(raw string) bool {
	parts := strings.Split(raw, ",")
	if len(parts) == 0 {
		return false
	}
	chunkedSeen := false
	for i, p := range parts {
		tok := strings.ToLower(strings.Trim(p, " \t"))
		if tok == "" {
			return false
		}
		for _, b := range []byte(tok) {
			if !isTchar(b) {
				return false
			}
		}
		if chunkedSeen {
			return false
		}
		if tok == "chunked" {
			if i != len(parts)-1 {
				return false
			}
			chunkedSeen = true
		}
	}
	return true
}
