package export

import (
	"bytes"
	"encoding/base64"
	"strconv"
	"strings"
	"time"

	"duskweave/boundary/internal/frame"
)

func runHTTPS(parent *frame.Reader, safe *frame.Writer, payload []byte) error {
	return runHTTPSWithClock(parent, safe, payload, time.Now)
}

func runHTTPSWithClock(parent *frame.Reader, safe *frame.Writer, payload []byte, nowFn func() time.Time) error {
	var q frame.HttpsRequest
	if frame.Decode(payload, &q) != nil || frame.ValidateHTTPS(q, nowFn()) != nil {
		return frame.ErrFrame
	}
	readyAt := nowFn()
	if safe.Write(frame.PhaseReady{Identity: frame.HttpsID(q.Identity, "READY", 1), Phase: "connect"}, frame.ControlLimit) != nil {
		return frame.ErrFrame
	}
	granted, raw, err := readChainWithClock(parent, q, readyAt, nowFn)
	if err != nil || !acceptPrivateAt(q, granted, raw, nowFn().UnixMilli()) {
		return frame.ErrFrame
	}
	projected, ok := projectHTTPS(q, raw)
	if !ok || safe.Write(projected, frame.Limit) != nil {
		return frame.ErrFrame
	}
	var ack frame.HttpsAck
	if parent.Read(&ack, frame.ControlLimit) != nil || !frame.SameAttempt(ack.Identity, q.Identity) || ack.Kind != "ACK" || ack.Sequence != 8 {
		return frame.ErrFrame
	}
	return nil
}

func readChainWithClock(parent *frame.Reader, q frame.HttpsRequest, readyAt time.Time, nowFn func() time.Time) (string, frame.PrivateResult, error) {
	handleEnd, ok := frame.AddMs(q.Deadline, 1000)
	if !ok {
		return "", frame.PrivateResult{}, frame.ErrFrame
	}
	expect := []struct {
		phase string
		seq   uint32
	}{{"connect", 2}, {"tls", 4}, {"head", 6}}
	granted, step := "", 0
	for {
		payload, err := parent.Take(frame.Limit)
		if err != nil {
			return "", frame.PrivateResult{}, frame.ErrFrame
		}
		kind, err := frame.Field(payload, "kind")
		if err != nil {
			return "", frame.PrivateResult{}, frame.ErrFrame
		}
		parent.Count++
		if kind == "RESULT" {
			if granted == "" || nowFn().UnixMilli() > handleEnd {
				return "", frame.PrivateResult{}, frame.ErrFrame
			}
			var raw frame.PrivateResult
			if frame.Decode(payload, &raw) != nil {
				return "", frame.PrivateResult{}, frame.ErrFrame
			}
			return granted, raw, nil
		}
		if kind == "START" && len(payload) > int(frame.ControlLimit) {
			return "", frame.PrivateResult{}, frame.ErrFrame
		}
		if kind != "START" || step >= len(expect) {
			return "", frame.PrivateResult{}, frame.ErrFrame
		}
		var start frame.PhaseStart
		if frame.Decode(payload, &start) != nil || !admitAt(start, q, expect[step].phase, expect[step].seq, readyAt, step == 0, nowFn()) {
			return "", frame.PrivateResult{}, frame.ErrFrame
		}
		granted = expect[step].phase
		step++
		readyAt = nowFn()
	}
}

func admitAt(start frame.PhaseStart, q frame.HttpsRequest, phase string, seq uint32, readyAt time.Time, gap bool, now time.Time) bool {
	if !frame.SameAttempt(start.Identity, q.Identity) || start.Kind != "START" || start.Sequence != seq || start.Phase != phase {
		return false
	}
	if start.Deadline != q.Deadline || !now.Before(time.UnixMilli(q.Deadline)) {
		return false
	}
	return !gap || now.Sub(readyAt) <= time.Second
}

func acceptPrivate(q frame.HttpsRequest, granted string, raw frame.PrivateResult, _ time.Time) bool {
	return acceptPrivateAt(q, granted, raw, time.Now().UnixMilli())
}

func acceptPrivateAt(q frame.HttpsRequest, granted string, raw frame.PrivateResult, nowMs int64) bool {
	if !frame.SameAttempt(raw.Identity, q.Identity) || raw.Kind != "RESULT" || raw.Sequence != 7 || raw.Phase != granted {
		return false
	}
	earliest, ok1 := frame.AddMs(q.Deadline, -int64(q.Duration))
	handleEnd, ok2 := frame.AddMs(q.Deadline, 1000)
	if !ok1 || !ok2 || nowMs > handleEnd || raw.Observed < earliest || raw.Observed > nowMs || raw.Network > q.Limits.NetworkInboundBytes {
		return false
	}
	if raw.Status == "timed_out" {
		if raw.Observed > handleEnd {
			return false
		}
	} else if raw.Observed > q.Deadline {
		return false
	}
	header, err := base64.StdEncoding.DecodeString(raw.Header)
	if raw.Header != "" && (err != nil || uint32(len(header)) > q.Limits.HeaderBytes) {
		return false
	}
	switch raw.Status {
	case "received":
		return raw.Phase == "head" && raw.TLS == "none" && len(header) > 0
	case "connection_failed":
		return (raw.Phase == "connect" || raw.Phase == "head") && raw.TLS == "none" && raw.Header == ""
	case "tls_failed":
		return raw.Phase == "tls" && raw.Header == "" && tlsKnown(raw.TLS)
	case "timed_out", "execution_refused":
		return (raw.Phase == "connect" || raw.Phase == "tls" || raw.Phase == "head") && raw.TLS == "none" && raw.Header == ""
	case "network_limit", "unsupported_http":
		return (raw.Phase == "tls" || raw.Phase == "head") && raw.TLS == "none" && raw.Header == ""
	case "header_limit", "malformed_response":
		return raw.Phase == "head" && raw.TLS == "none" && raw.Header == ""
	default:
		return false
	}
}

func tlsKnown(category string) bool {
	switch category {
	case "unknown_root", "hostname_mismatch", "expired_or_not_yet_valid", "protocol", "other_validation":
		return true
	default:
		return false
	}
}

func projectHTTPS(q frame.HttpsRequest, raw frame.PrivateResult) (frame.SafeResult, bool) {
	outcome, code, redirect, headerLen := raw.Status, uint16(0), "none", uint32(0)
	switch raw.Status {
	case "received":
		header, err := base64.StdEncoding.DecodeString(raw.Header)
		if err != nil {
			return frame.SafeResult{}, false
		}
		parsed, parsedRedirect, status := classifyStatus(header)
		if status != "received" {
			return frame.SafeResult{}, false
		}
		outcome, code, redirect, headerLen = "head_response", parsed, parsedRedirect, uint32(len(header))
	case "connection_failed", "tls_failed", "timed_out":
		outcome = raw.Status
	case "execution_refused", "header_limit", "network_limit", "unsupported_http", "malformed_response":
		outcome = "execution_refused"
	default:
		return frame.SafeResult{}, false
	}
	return frame.SafeResult{
		Identity: frame.HttpsID(q.Identity, "RESULT", 7), Outcome: outcome, Observed: raw.Observed,
		Network: raw.Network, Header: headerLen, StatusCode: code, TLS: raw.TLS, Redirect: redirect,
		Limitations: limitationCodes(outcome, raw.Status, raw.TLS),
	}, true
}

func isHeaderTokenByte(b byte) bool {
	if b >= 'a' && b <= 'z' || b >= 'A' && b <= 'Z' || b >= '0' && b <= '9' {
		return true
	}
	switch b {
	case '!', '#', '$', '%', '&', '\'', '*', '+', '-', '.', '^', '_', '`', '|', '~':
		return true
	}
	return false
}

func validFieldText(b []byte) bool {
	for _, c := range b {
		if c == '\r' || c == '\n' || c == 0x7f || (c < 0x20 && c != '\t') {
			return false
		}
	}
	return true
}

func classifyStatus(header []byte) (uint16, string, string) {
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
	if !bytes.HasPrefix(status, []byte("HTTP/1.1 ")) || len(status) < 13 || status[12] != ' ' || !validFieldText(status[13:]) {
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
			if !isHeaderTokenByte(c) {
				return 0, "none", "malformed_response"
			}
		}
		val := bytes.Trim(rawVal, " \t")
		if !validFieldText(val) {
			return 0, "none", "malformed_response"
		}
		switch strings.ToLower(string(name)) {
		case "content-length":
			hasCL = true
			if !validExportContentLength(string(val), &clVal) {
				return 0, "none", "malformed_response"
			}
		case "transfer-encoding":
			hasTE = true
			if !validExportTransferEncoding(string(val)) {
				return 0, "none", "malformed_response"
			}
		}
	}
	if hasCL && hasTE {
		return 0, "none", "malformed_response"
	}
	if code < 400 && code >= 300 {
		return uint16(code), "refused_redirect", "received"
	}
	return uint16(code), "none", "received"
}

func validExportContentLength(raw string, seen *string) bool {
	for _, p := range strings.Split(raw, ",") {
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

func validExportTransferEncoding(raw string) bool {
	parts := strings.Split(raw, ",")
	if len(parts) == 0 {
		return false
	}
	for i, p := range parts {
		tok := strings.ToLower(strings.Trim(p, " \t"))
		if tok == "" {
			return false
		}
		for _, b := range []byte(tok) {
			if !isHeaderTokenByte(b) {
				return false
			}
		}
		if tok == "chunked" && i != len(parts)-1 {
			return false
		}
	}
	return true
}

func limitationCodes(outcome, status, tlsCat string) []string {
	codes := []string{"synthetic_https_lab_acquisition_not_public_live"}
	switch outcome {
	case "head_response":
		codes = append(codes, "head_response_does_not_prove_access_or_objective_completion")
	case "connection_failed":
		codes = append(codes, "single_endpoint_connection_failed_not_whole_host")
	case "tls_failed":
		codes = append(codes, "single_endpoint_tls_failed_not_whole_host")
	case "timed_out":
		codes = append(codes, "single_endpoint_timeout_not_whole_host")
	default:
		codes = append(codes, "single_endpoint_execution_refused_not_whole_host")
	}
	switch {
	case tlsCat == "unknown_root":
		codes = append(codes, "tls_unknown_root")
	case tlsCat == "hostname_mismatch":
		codes = append(codes, "tls_hostname_mismatch")
	case tlsCat == "expired_or_not_yet_valid":
		codes = append(codes, "tls_expired_or_not_yet_valid")
	case tlsCat == "protocol":
		codes = append(codes, "tls_protocol")
	case tlsCat == "other_validation":
		codes = append(codes, "tls_other_validation")
	case status == "header_limit":
		codes = append(codes, "http_header_limit_exceeded")
	case status == "network_limit":
		codes = append(codes, "network_byte_limit_exceeded")
	case status == "unsupported_http":
		codes = append(codes, "unsupported_http_response")
	case status == "malformed_response":
		codes = append(codes, "malformed_http_response")
	}
	return codes
}
