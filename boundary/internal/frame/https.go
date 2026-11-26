package frame

import (
	"encoding/binary"
	"math"
	"net/netip"
	"strings"
	"time"
)

const HttpsSchema = "m1_https_ipc_v1"
const LabHost = "head.duskweave.test"
const LabIPv4 = "198.51.100.10"
const LabResolver = "192.0.2.53"
const LabRunProfile = "autonomous-https-genai-lab-v1"
const PublicRunProfile = "autonomous-https-genai-live-v1"
const LabProfile = "https-head-v1"

var specialIPv4 = [][2]uint32{
	{0x00000000, 0xff000000},
	{0x0a000000, 0xff000000},
	{0x64400000, 0xffc00000},
	{0x7f000000, 0xff000000},
	{0xa9fe0000, 0xffff0000},
	{0xac100000, 0xfff00000},
	{0xc0000000, 0xffffff00},
	{0xc0000200, 0xffffff00},
	{0xc0586300, 0xffffff00},
	{0xc0a80000, 0xffff0000},
	{0xc6120000, 0xfffe0000},
	{0xc6336400, 0xffffff00},
	{0xcb007100, 0xffffff00},
	{0xe0000000, 0xe0000000},
}

type Limits struct {
	NetworkInboundBytes uint32 `json:"network_inbound_bytes"`
	HeaderBytes         uint32 `json:"header_bytes"`
	FramePayloadBytes   uint32 `json:"frame_payload_bytes"`
	ControlPayloadBytes uint32 `json:"control_payload_bytes"`
	MaxWorkerFrames     uint32 `json:"max_worker_frames"`
	MinTLSVersion       string `json:"min_tls_version"`
	HTTPVersion         string `json:"http_version"`
	MaxConnects         uint32 `json:"max_connects"`
	MaxRequests         uint32 `json:"max_requests"`
}

type HttpsRequest struct {
	Identity
	Engagement      string `json:"engagement_id"`
	Campaign        string `json:"campaign_id"`
	Decision        string `json:"decision_id"`
	Run             string `json:"run_id"`
	StepIndex       uint32 `json:"step_index"`
	AttemptKind     string `json:"attempt_kind"`
	Result          string `json:"result_id"`
	Profile         string `json:"profile"`
	Mode            string `json:"execution_mode"`
	RunProfile      string `json:"run_profile"`
	Release         string `json:"release_sha256"`
	Qualification   string `json:"qualification_ref"`
	Asset           string `json:"asset_ref"`
	Endpoint        string `json:"endpoint_ref"`
	Hostname        string `json:"hostname"`
	IPv4            string `json:"ipv4"`
	Port            uint16 `json:"port"`
	Path            string `json:"approved_path"`
	Method          string `json:"method"`
	ParentClaim     string `json:"parent_claim_id"`
	ParentRevision  uint32 `json:"parent_claim_revision"`
	Resolver        string `json:"resolver_ipv4"`
	Vantage         string `json:"vantage_ref"`
	Permission      string `json:"permission_id"`
	MissionRevision uint64 `json:"expected_mission_revision"`
	EvaluatedAt     int64  `json:"authority_evaluated_at_ms"`
	ClaimUntil      int64  `json:"claim_valid_until_s"`
	EpisodeDeadline int64  `json:"episode_deadline_at_ms"`
	Deadline        int64  `json:"deadline_at_ms"`
	Duration        uint32 `json:"max_duration_ms"`
	Limits          Limits `json:"limits"`
}

type PhaseReady struct {
	Identity
	Phase string `json:"phase"`
}

type PhaseStart struct {
	Identity
	Phase    string `json:"phase"`
	Deadline int64  `json:"deadline_at_ms"`
}

type HttpsAck struct {
	Identity
}

type PrivateResult struct {
	Identity
	Phase    string `json:"phase"`
	Status   string `json:"status"`
	Observed int64  `json:"observed_at_ms"`
	Network  uint32 `json:"network_bytes"`
	Header   string `json:"header_bytes"`
	TLS      string `json:"tls_category"`
}

type SafeResult struct {
	Identity
	Outcome     string   `json:"outcome"`
	Observed    int64    `json:"observed_at_ms"`
	Network     uint32   `json:"network_bytes"`
	Header      uint32   `json:"header_bytes"`
	StatusCode  uint16   `json:"status_code"`
	TLS         string   `json:"tls_category"`
	Redirect    string   `json:"redirect"`
	Limitations []string `json:"limitations"`
}

func HttpsID(src Identity, kind string, sequence uint32) Identity {
	return Identity{Schema: HttpsSchema, Kind: kind, Attempt: src.Attempt, Generation: src.Generation, Sequence: sequence}
}

func SameAttempt(id, src Identity) bool {
	return id.Schema == src.Schema && id.Attempt == src.Attempt && id.Generation == src.Generation
}

func AddMs(base, delta int64) (int64, bool) {
	if delta > 0 && base > math.MaxInt64-delta {
		return 0, false
	}
	if delta < 0 && base < math.MinInt64-delta {
		return 0, false
	}
	return base + delta, true
}

func fixedLimits() Limits {
	return Limits{NetworkInboundBytes: 65536, HeaderBytes: 8192, FramePayloadBytes: 16384,
		ControlPayloadBytes: 1024, MaxWorkerFrames: 16, MinTLSVersion: "1.2", HTTPVersion: "HTTP/1.1",
		MaxConnects: 1, MaxRequests: 1}
}

func uuidOK(s string) bool {
	if len(s) != 36 || s == "00000000-0000-0000-0000-000000000000" {
		return false
	}
	for i, c := range []byte(s) {
		if i == 8 || i == 13 || i == 18 || i == 23 {
			if c != '-' {
				return false
			}
			continue
		}
		if !(c >= '0' && c <= '9' || c >= 'a' && c <= 'f') {
			return false
		}
	}
	return true
}

func shaOK(s string) bool {
	if len(s) != 64 {
		return false
	}
	for _, b := range []byte(s) {
		if !(b >= '0' && b <= '9' || b >= 'a' && b <= 'f') {
			return false
		}
	}
	return true
}

func endpointOK(s string) bool {
	if len(s) < len("endpoint_0") || len(s) > 16 || len(s) <= len("endpoint_") {
		return false
	}
	if s[:len("endpoint_")] != "endpoint_" {
		return false
	}
	for _, c := range []byte(s[len("endpoint_"):]) {
		if c < '0' || c > '9' {
			return false
		}
	}
	return true
}

func isGlobalIPv4(raw string) bool {
	ip, err := netip.ParseAddr(raw)
	if err != nil || !ip.Is4() || ip.String() != raw {
		return false
	}
	b := ip.As4()
	n := binary.BigEndian.Uint32(b[:])
	for _, entry := range specialIPv4 {
		if (n & entry[1]) == entry[0] {
			return false
		}
	}
	return true
}

func validPublicHostname(name string) bool {
	if len(name) == 0 || len(name) > 253 || !strings.Contains(name, ".") {
		return false
	}
	if _, err := netip.ParseAddr(name); err == nil {
		return false
	}
	for _, label := range strings.Split(name, ".") {
		if len(label) == 0 || len(label) > 63 || label[0] == '-' || label[len(label)-1] == '-' {
			return false
		}
		for _, ch := range []byte(label) {
			if !(ch >= 'a' && ch <= 'z' || ch >= '0' && ch <= '9' || ch == '-') {
				return false
			}
		}
	}
	return true
}

func validHTTPSScope(q HttpsRequest) bool {
	switch q.RunProfile {
	case LabRunProfile:
		return q.Hostname == LabHost && q.IPv4 == LabIPv4 && q.Resolver == LabResolver
	case PublicRunProfile:
		return validPublicHostname(q.Hostname) && isGlobalIPv4(q.IPv4) && isGlobalIPv4(q.Resolver)
	default:
		return false
	}
}

func ValidateHTTPS(q HttpsRequest, now time.Time) error {
	if !SameAttempt(q.Identity, q.Identity) || q.Schema != HttpsSchema || q.Kind != "REQUEST" || q.Sequence != 0 || q.Generation == 0 {
		return ErrFrame
	}
	ids := []string{q.Engagement, q.Campaign, q.Decision, q.Run, q.Attempt, q.Result, q.Qualification, q.Asset, q.ParentClaim, q.Vantage, q.Permission}
	for _, id := range ids {
		if !uuidOK(id) {
			return ErrFrame
		}
	}
	nowMs := now.UnixMilli()
	maxDeadline, deadlineOK := AddMs(nowMs, int64(q.Duration))
	valid := deadlineOK && q.StepIndex >= 1 && q.StepIndex <= 4 && q.AttemptKind == "head" && q.Profile == LabProfile &&
		q.Mode == "live" && validHTTPSScope(q) && shaOK(q.Release) && endpointOK(q.Endpoint) &&
		q.Port == 443 && q.Path == "/" && q.Method == "HEAD" &&
		q.ParentRevision >= 1 && q.MissionRevision >= 1 && q.EvaluatedAt > 0 &&
		q.EvaluatedAt <= nowMs && q.ClaimUntil > now.Unix() && q.EpisodeDeadline >= q.Deadline &&
		q.Duration >= 1 && q.Duration <= 5000 && q.Deadline > nowMs && q.Deadline <= maxDeadline &&
		q.Limits == fixedLimits()
	if !valid {
		return ErrFrame
	}
	ip, err := netip.ParseAddr(q.IPv4)
	resolver, rerr := netip.ParseAddr(q.Resolver)
	if err != nil || rerr != nil || !ip.Is4() || !resolver.Is4() || ip.String() != q.IPv4 || resolver.String() != q.Resolver {
		return ErrFrame
	}
	return nil
}
