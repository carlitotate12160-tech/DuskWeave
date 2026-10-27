package frame

// Request/response versions are fixed; a mismatch is a refusal, never an
// upgrade path.
const (
	RequestVersion = "dw_boundary_request_v1"
	ResultVersion  = "dw_boundary_result_v1"

	KindCapture  = "capture"
	KindExporter = "exporter"

	PhaseReady   = "ready"
	PhaseStart   = "start"
	PhaseCancel  = "cancel"
	PhaseResult  = "result"
	PhaseAck     = "ack"
	EffectDNSUDP = "dns_udp"
)

// Endpoint is the single admitted network destination.
type Endpoint struct {
	IPv4      string `json:"ipv4"`
	Port      uint16 `json:"port"`
	Transport string `json:"transport"` // "udp"
}

// CaptureRequest is the issued control for the raw-facing worker. It may
// perform exactly one bounded exchange against Endpoint.
type CaptureRequest struct {
	Version    string   `json:"version"`
	Kind       string   `json:"kind"`
	AttemptID  string   `json:"attempt_id"`
	SessionID  string   `json:"session_id"`
	Effect     string   `json:"effect"`
	Qname      string   `json:"qname"`
	Qtype      string   `json:"qtype"`
	Endpoint   Endpoint `json:"endpoint"`
	TxID       uint16   `json:"txid"`
	DeadlineMs int64    `json:"deadline_ms"`
	MaxWire    uint32   `json:"max_wire"`
}

// ExporterRequest carries the independently issued safe context used to
// validate the candidate raw output before any ordinary result.
type ExporterRequest struct {
	Version    string `json:"version"`
	Kind       string `json:"kind"`
	AttemptID  string `json:"attempt_id"`
	SessionID  string `json:"session_id"`
	Effect     string `json:"effect"`
	Qname      string `json:"qname"`
	Qtype      string `json:"qtype"`
	TxID       uint16 `json:"txid"`
	DeadlineMs int64  `json:"deadline_ms"`
	MaxRaw     uint32 `json:"max_raw"`
}

// Ready is emitted by each worker after its issued request validates.
type Ready struct {
	Version   string `json:"version"`
	Phase     string `json:"phase"`
	Kind      string `json:"kind"`
	AttemptID string `json:"attempt_id"`
}

// CaptureReady is the capture worker's readiness frame inside the raw
// channel; it binds the producing process to the issued attempt before any
// payload bytes.
type CaptureReady Ready

// Start is the one-use parent-origin command permitting the issued phase.
type Start struct {
	Version   string `json:"version"`
	Phase     string `json:"phase"`
	AttemptID string `json:"attempt_id"`
}

// Cancel terminates a live phase; workers exit without producing output.
type Cancel struct {
	Version   string `json:"version"`
	Phase     string `json:"phase"`
	AttemptID string `json:"attempt_id"`
}

// Ack is the launcher's terminal control for the exporter.
type Ack struct {
	Version   string `json:"version"`
	Phase     string `json:"phase"`
	AttemptID string `json:"attempt_id"`
}

// Answer is one admitted IPv4/TTL record.
type Answer struct {
	IPv4 string `json:"ipv4"`
	TTL  uint32 `json:"ttl"`
}

// Counters are the bounded, fixed-field diagnostics permitted to cross the
// boundary. Raw payload or error detail is never carried.
type Counters struct {
	RawBytes     uint32 `json:"raw_bytes"`
	RawTruncated bool   `json:"raw_truncated"`
}

// Result is the closed ordinary result produced only by the exporter.
type Result struct {
	Version     string   `json:"version"`
	Phase       string   `json:"phase"`
	AttemptID   string   `json:"attempt_id"`
	Disposition string   `json:"disposition"`
	Rcode       string   `json:"rcode"`
	Truncated   bool     `json:"truncated"`
	Answers     []Answer `json:"answers"`
	Counters    Counters `json:"counters"`
	ElapsedMs   int64    `json:"elapsed_ms"`
}

// Dispositions are the closed enum crossing the boundary.
const (
	DispQualified  = "qualified"
	DispNoAnswers  = "noerror_no_answers"
	DispNXDomain   = "nxdomain"
	DispServFail   = "servfail"
	DispRefused    = "refused"
	DispTruncated  = "truncated"
	DispTimeout    = "timeout"
	DispMalformed  = "malformed"
	DispOversized  = "oversized"
	DispCancelled  = "cancelled"
	DispNoData     = "no_data"
	DispRcodeOther = "negative_or_error_rcode"
)
