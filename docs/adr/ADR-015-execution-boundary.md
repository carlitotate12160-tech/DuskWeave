# ADR-015: Qualified M1 Worker Isolation

| Metadata | Value |
| --- | --- |
| Status | ACCEPTED — product owner, 2026-10-06; M1 selected lane only |
| Date | 2026-10-06 |
| Revision | R1 — scoped architecture accepted by the product owner on 2026-10-06 |
| Authoring base | `086936d2903b8958c0e5bc69af9891d9710e092f` |
| Product authority | Accepted PRD-000..010 and M1 product contract; no new product breadth |
| Direct dependencies | accepted ADR-002/004/011/013; ADR-014 prerequisite rebound only by accepted M1 enabling section 2 |
| Scope | [M1 enabling lane](../contracts/M1-enabling-architecture.md); no full-stage acceptance |

## 1. Context and problem statement

A Go child under the Windows core's account is not a security/sensitive boundary.
M1 raw capture needs OS-enforced separation from core state, credentials and diagnostics.
The source worktree choice does not establish the deployment platform.

## 2. Decision drivers

Protect core/ordinary surfaces, bind released worker identity, contain resources and egress,
and qualify abnormal termination without building a general sandbox platform.

## 3. Considered options

| Option | Benefit | Cost / disposition |
| --- | --- | --- |
| Same-user native child | Easy local launch | Ambient file/network/dump access; reject as qualified capture |
| Windows restricted token/Job/WFP boundary | Native operation | New unqualified platform-specific mechanism; defer |
| Linux namespaces/cgroups with fixed worker image | Concrete bounded isolation | Select for M1 acquisition; qualification remains mandatory |

## 4. Decision outcome

M1 uses a qualified single-tenant Linux host/guest: Linux core/Broker, attempt-local Go
capture and egress containers, and existing loopback PostgreSQL. The Windows source
worktree stays unchanged; Windows M0 evidence is reused only for unchanged behavior.
Native Windows M1 is unqualified. Deployment architecture is accepted; host provisioning is not assigned here.

Select one disposable **capture/egress pair per attempt**, using a preloaded reviewed image
pinned by digest and two fixed compiled entrypoints. Capture receives the exact issued
request and has only its admitted network endpoint. Exporter has no network and validates
bounded candidate output against independently issued safe context before exporting.
Reject same-process projection as the sole boundary against capture compromise. The pair
is a narrow execution adapter, not two public services, a registry or model owners.

The privileged fixed host launcher creates separate non-root UIDs, PID/IPC namespaces and
private parent-created pipe routes. Capture has no handle to exporter stdout/core memory
or ordinary results; exporter alone owns ordinary result egress. Capture attach output is
forwarded only into the isolated exporter input. Raw-facing attach/frame decoding and its
bounded transit buffers are part of the sensitive boundary, never the ordinary core adapter.
Qualification covers Docker transport/logging/daemon crash sinks as well as Go processes.
Launcher administration may bind exact instance/endpoint/limits and STOP but exposes no
shell, arbitrary image/path/mount/rule or generic Docker API to runtime clients.

Both containers: read-only root filesystem, no writable volume/temp/cache/host bind,
Docker socket, DSN/config/credential mount or shared namespaces; drop all capabilities,
no-new-privileges and reviewed default seccomp; minimal safe environment with no inherited
tokens/proxy/debug configuration. No pull/install or user executable during campaigns.
Capture and exporter each have 128 MiB memory and equal memory+swap (no swap), 0.5 CPU,
32 PID/thread and 64 FD ceilings. Pair total is bounded by 256 MiB and 1 CPU; these are
unbenchmarked ceilings, not a throughput claim. No reusable raw worker across attempts.

Core-dump limit zero and dumpability off; qualify host coredump/kdump, snapshots/hibernate,
profilers and Docker logs for every raw-facing process/transport. Logging driver none;
capture/exporter stderr is unlogged discard. Unsupported no-swap/no-dump/storage control
blocks acquisition. Safe structured phase/counter diagnostics replace raw error capture.

Capture has a private network namespace, host nftables default-deny and exactly one
admitted resolver UDP/TCP or pinned provider/target TCP endpoint/port. Exporter uses no
network namespace connectivity. No private/metadata/other-container/IPv6/wildcard fallback.
Every outgoing packet, including established traffic, requires the live attempt member
in the host nft timeout set; no conntrack-established accept survives its expiry/removal.
Released capture code enforces fixed methods; OS egress cannot inspect encrypted intent.
The trusted exporter limits capture's output rights, but does not independently prove
network truth or defeat compromise of the exporter itself. State those TCB limits honestly.

### Distinct selected timers and stop control

| Bound | R1 selection / meaning |
| --- | --- |
| READY-to-start consumption | At most 1 second, clamped to current authority/premise deadline; no late initiation |
| In-flight egress lease | At most 2 seconds, clamped to the original attempt/premise deadline; never renew beyond it |
| Renewal interval | 500 ms; parent must finish current-authority check before existing lease expiry |
| Responsive withdrawal | Under admission fence: latch STOP, remove egress and cancel immediately, before history/SQL wait |
| Parent/control loss fallback | Independent host watchdog/timeout closes egress by the existing lease expiry, at most 2 seconds |
| Kill/reap after stop | At most 1 second; inability leaves handling unresolved, not a successful disposal claim |

R1 replaces the prior proposed 100/250 ms coupling. Values provide explicit scheduling
margin but remain selected assumptions. Future profile qualification checks normal renewal
latency/jitter under the supported resource envelope and the worst stop bound; it cannot
claim calibration or sub-millisecond immediate cancellation from this document.
Unavailable DB/current authority or a pending writer prevents renewal/new effects.
The watchdog is independent of core event delivery and never waits on history backlog.

Launcher owns a monotonic campaign/session-generation STOP latch and shared host admission
critical section. Its root-owned fixed stop helper is authorized by the deployment's narrow
privilege policy; safe campaign/generation references map only to registered instances.
Broker and trusted local stop-only CLI can set STOP; workers and start/renew cannot clear it.
Host START/egress grant and STOP serialize here even when the Broker's socket is unavailable.
The host release performs only bounded nonblocking control writes; it never waits on SQL
or target I/O while holding this section. Failed/partial release is conservatively UNKNOWN.
Unreachable Broker withdrawal sets this latch and closes that campaign's egress before
ADR-016's stop-only durable Mission write. Host restart starts default-deny; explicit
no-effects reconciliation is required, never automatic restart/resume. A DB-unavailable
withdrawal has an honest pending receipt and stays stopped until resolved.

Preparation of the pinned pair before READY is allowed with network closed. Warming/reuse
across attempts, batch dispatch, remote launcher and parallel campaign episodes are deferred.
Already-sent external work cannot be rolled back by local stop. Raw disposal applies to
the entire pair and raw-facing transport on success/failure/cancel/parent loss.

## 5. Consequences

The Linux capture boundary adds explicit deployment requirements, not per-slice source
folders or a generic sandbox programme. Resource values are selected ceilings, not
benchmarked capacity. Any unsupported isolation control blocks effects.
Proof/custody, target implants, native helpers and other languages remain deferred.

Technical basis checked 2026-10-06: [Docker resource limits](https://docs.docker.com/engine/containers/resource_constraints/),
[seccomp](https://docs.docker.com/engine/security/seccomp/),
[logging controls](https://docs.docker.com/engine/logging/configure/) and
[Linux coredump configuration](https://www.kernel.org/doc/html/latest/admin-guide/sysctl/kernel.html).
Those facilities do not by themselves establish qualified isolation or erasure.

## 6. Invariant compliance matrix

| Invariant | Obligation |
| --- | --- |
| INV-001 | Worker/launcher know only one issued attempt, never operational models. No universal manager or workflow engine. |
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
