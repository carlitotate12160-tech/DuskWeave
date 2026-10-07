# ADR-017: M1 Private Rust–Go IPC

| Metadata | Value |
| --- | --- |
| Status | ACCEPTED — product owner, 2026-10-06; M1 selected lane only |
| Date | 2026-10-06 |
| Revision | R1 — scoped architecture accepted by the product owner on 2026-10-06; R2 distinguishes the untrusted candidate request channel from the trusted safe-result channel, published under the owner-approved 2026-10-07 direction with exact-text acceptance recorded separately |
| Authoring base | `086936d2903b8958c0e5bc69af9891d9710e092f` |
| Product authority | Accepted PRD-000..010 and M1 product contract; no new product breadth |
| Direct dependencies | accepted ADR-009/011/013/015/016 |
| Scope | [M1 enabling lane](../contracts/M1-enabling-architecture.md); no full-stage acceptance |

## 1. Context and problem statement

Cross-language serialization cannot establish source authority, safe output or current permission.
A worker must neither inherit infrastructure secrets nor accept commands through a public endpoint.

## 2. Decision drivers

Parent/child origin binding, finite framing, separate effect control and admitted results,
strict compatibility and no raw diagnostics/response channel into core.

## 3. Considered options

| Option | Benefit | Conflict / disposition |
| --- | --- | --- |
| Local HTTP service/shared spool | Familiar APIs | Listener/credential/durable raw surface; unnecessary |
| Arbitrary tool stdout | Easy integration | Unbounded/error/raw propagation; reject |
| Exclusive anonymous pipes, strict length-prefixed messages | Small private boundary | Select; OS handle provenance and result validation required |

## 4. Decision outcome

Rust Broker/launcher creates exclusive anonymous parent/child pipe routes for ADR-015's
fixed capture/egress pair: safe issued controls/context to the pair; bounded capture output
only to isolated exporter; admitted exporter result/control only to ordinary Rust. No TTY,
public listener, shared spool or cross-attempt handles. Raw-facing Docker framing removal
is within the qualified sensitive transport/exporter boundary; capture has no ordinary
result descriptor. Only exporter attach output reaches the ordinary adapter. Both stderr
channels discard unlogged. No client/runtime DSN or provider token is transferred.
Handle provenance, distinct child roles/image and independently issued context bind origin;
self-reported producer/profile/authorized strings never establish trust. Exporter cannot
manufacture parent START, and capture cannot impersonate exporter or parent control.

Each message is a four-byte unsigned big-endian length followed by one strict UTF-8 JSON
object. Limits are ADR-013's effective bounds checked before allocation.
Reject duplicate/unknown keys, excess nesting/collections, unsupported version, trailing
data, numeric overflow, missing required fields, wrong attempt/session/profile/digest,
phase violations and extra result frames. No codec auto-upgrade or lossy defaults.

Fixed phase messages: REQUEST -> READY -> START -> RESULT -> ACK; READY/START repeats
only for declared TCP/TLS/HTTP subphases, with CANCEL permitted
at any live phase. Request and all ordinary fields are safe before serialization.
Start is a one-use parent-origin command for the exact issued phase; worker cannot create one.
Phase permits for additional fixed network operations consume the same attempt's finite
counters; they are not generic socket commands. Total frame ceiling includes controls.
If more control phases would exceed that ceiling, reject before the next operation.
The 500 ms renewal updates the host egress lease through the fixed host control path,
not repeated worker IPC frames. Worker phase permits retain the original absolute deadline;
renewal cannot extend them. A stop uses bounded CANCEL and host egress removal.

Request carries identifiers/context/purpose/mode, declared endpoint/query dictionary
references, approved path, pinned address/resolver, relevant owner revisions/evaluation time,
profile/schema/release binding, absolute UTC deadline plus bounded monotonic duration and
effective limits. Session-local sequence and issued attempt bind every control/result.
Use monotonic elapsed time for local timeout/lease; UTC expiry is rechecked by core authority.
Clock uncertainty/rollback blocks start or makes result timing uncertain, never extends authority.

Result variants are closed profile objects: approved references or purpose-admitted
canonical hostname fields, IPv4/TTL, times, bounded counters and fixed phase/disposition
enums under ADR-011. No arbitrary payload/map/error/detail field. New hostname output must
pass independently issued purpose/class/exclusions in exporter and core; hint admission
does not make it contactable. Core assigns stable hint refs after validating that context.
Validate address, phase, source/vantage/mode, counters and completeness against the request.
No result creates accepted claim/evidence sufficiency; receiving owner reconciles independently.

Unexpected bytes or an invalid frame are never logged, previewed, fingerprinted or quarantined
by core. Stop and discard the stream; retain attempt/category only. Malformed capture output is handled by the isolated exporter, before ordinary receipt.
The exporter is trusted release code under ADR-015; raw escape from its ordinary output is
a contamination incident even if core rejects it. Rejection after ordinary receipt cannot
prove isolation. Qualification includes a capture process emitting forged/raw candidate
bytes and sensitive sentinels: they must stay in the boundary with category-only failure.
Compatibility validation in core is defense in depth, not the first safe-egress decision.

Transport loss after possible start leaves UNKNOWN. Only safe durable core records may be
redelivered. IPC retry/worker restart cannot repeat the acquisition or retain raw input.
EOF, cancellation, oversize, parser error and process failure remain explicit safe dispositions.

### Generated-worker channels

A generated worker's typed effect-request channel is untrusted candidate input:
self-reported producer, digest, identity or authority strings never establish trust. Each
request binds engagement/campaign, session, its own attempt identity, candidate executable
digest, declared effect interface and admitted boundary schema, and is admitted only
through the owning authority/Gateway/Broker path — never through this worker channel
itself. The worker's output returns on the trusted safe-result path: only qualified
exporter/trusted release code produces ordinary results, and a generated worker holds no
ordinary result descriptor. A forged digest, result identity or unexpected frame rejects
as a bounded safe failure; malformed candidate output stays inside the isolation boundary
with category-only failure, following the same discard-and-disposition rules above.

## 5. Consequences

Private pipes avoid a new service or authenticated remote RPC scheme. They do not isolate
a malicious host administrator or authenticate the external provider's truth.
A different host topology/transport or untrusted executable requires new accepted coverage.
JSON schemas and process implementation belong to the future measured runtime packet;
the fixed field semantics/encoding here are resolved, not IDE architecture choices.

## 6. Invariant compliance matrix

| Invariant | Obligation |
| --- | --- |
| INV-001 | IPC carries one typed issued attempt with no model mutation or arbitrary dispatch channel. No universal manager or workflow engine. |
| INV-002 | Terrain, Pathing and Trajectory retain distinct owners; no Access/Objective mutation. |
| INV-003 | Proposals, evidence, history and worker results never confer dispatch authority. |
| INV-004 | Narrow source-qualified claims, explicit uncertainty and owner reconciliation. |
| INV-005 | Only purpose-admitted safe semantics cross ordinary boundaries; no custody/proof implementation. |
| INV-006 | Runtime histories append; sensitive remediation has separate authority and safe accountability. |
| INV-007 | Campaign/mode/source isolation survives reads, derivation, correction and recovery. |

## 7. Verification and acceptance boundary

DESIGN checks links, ownership, dependencies and counterexamples under QUALITY_BAR section 7.
The cases above are future assertions, not executed runtime tests. The product owner
accepted R1 for the selected M1 lane on 2026-10-06. Runtime requires an issued bounded
packet; acceptance grants no target permission, qualified deployment, merge or seal.
