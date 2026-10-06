# ADR-011: Non-Proof Acquisition Sensitive Boundary

| Metadata | Value |
| --- | --- |
| Status | ACCEPTED — product owner, 2026-10-06; M1 selected lane only |
| Date | 2026-10-06 |
| Revision | R1 — scoped architecture accepted by the product owner on 2026-10-06 |
| Authoring base | `086936d2903b8958c0e5bc69af9891d9710e092f` |
| Product authority | Accepted PRD-000..010 and M1 product contract; no new product breadth |
| Direct dependencies | accepted PRD-010 and ADR-008/009; ADR-010 prerequisite rebound only by accepted M1 enabling section 2 for non-proof CT/DNS/HEAD |
| Scope | [M1 enabling lane](../contracts/M1-enabling-architecture.md); no full-stage acceptance |

## 1. Context and problem statement

Public CT records, DNS and HEAD responses can contain secrets, personal identifiers,
unexpected bodies or diagnostic fragments. Selecting headers after core ingestion is too late.
M1 has no proof derivation or secret custody. It needs a pre-core non-proof capture boundary.

## 2. Decision drivers

Purpose-bound minimal egress, no raw ordinary-surface exposure, qualified containment,
honest disposal/contamination, and no incidental proof/key/custody implementation.

## 3. Considered options

| Option | Benefit | Conflict / disposition |
| --- | --- | --- |
| Parse in core then redact | Fewer processes | Raw content reaches ordinary memory/errors; reject |
| Retain raw encrypted quarantine | Debugging | Prohibited retention and unnecessary custody; reject |
| Isolated attempt-local Go capture with closed egress projection | Protects selected lane | Select; isolation must be qualified before acquisition |

## 4. Decision outcome

### Actual surfaces and admissible output

Raw Go acquisition/parser memory, TLS/HTTP/DNS buffers and unexpected input remain inside
ADR-015's attempt-scoped capture/egress isolation unit. Capture cannot write the ordinary
core result pipe. A distinct fixed Go egress process, with no network authority or shared
UID/PID namespace, owns that pipe and admits bounded candidate projections before export.
The launcher's raw-facing transport also belongs to this sensitive handling boundary;
ordinary core never receives capture stdout or decodes capture Docker-attach frames.

Purpose configuration expressly classifies query identifiers, discovery/disclosure host
classes or exact names, and allowed routing aliases as non-sensitive for named consumers.
Syntax/public availability and hint-only status do not establish that. Choose exact canonical
names or DNS-label suffix classes with exclusions, at most 8 rules per purpose. Matching is
label-aware after lowercase ASCII normalization and removal of one terminal dot; reject
empty/overlong labels, control bytes and invalid LDH host syntax. A suffix rule explicitly
sets whether its apex is included; descendants alone never imply apex permission. It is
not regex, substring or arbitrary code; wildcard certificate names are not expanded.
Unknown-to-core names may exit only when independently admitted under that existing class.
Outside-class, uncertain or sensitive names remain withheld, with safe limitation/count only.

Separate the approved policy from the acquired candidate list. A CT result may contain at
most 8 newly admitted names, each <=253 ASCII bytes. Core assigns stable hint references
after admission and applies the same rule independently. A safe hint may remain contact-
prohibited; it never enlarges Mission's separately declared hostname contact authority.
Known query/endpoint references remain indexed where available. There is no requirement
to pre-enumerate every discoverable hostname in the ordinary core.

CT egress: admitted hostname or existing reference, source record ID only when purpose-safe,
certificate validity times when known, source-family key, retrieval time and completeness.
No subjects, emails, raw SAN arrays or full row. Only completely parsed records before an
intake bound may contribute safe hints; a prefix/aborted response never claims completeness.
DNS egress: approved query reference, transport, RCODE/truncation, admitted IPv4 values/TTL,
bounded alias steps containing independently purpose-admitted names/references or withheld
markers, and completeness. A withheld required lineage leaves the routing premise incomplete.
No TXT or raw packet. An admitted third-party alias is not independently contactable.
HTTPS egress: admitted endpoint reference, transport/TLS category, status and redirect
disposition (none/same permitted route/outside/withheld/invalid). No Location, full URL,
query/fragment, body, arbitrary header/certificate, cookie, auth challenge or peer error.

Safe diagnostics use the closed phase vocabulary setup/source/DNS/connect/TLS/HEAD/egress/
persistence/disposal and fixed codes for timeout, truncation, limit, TLS validation failure,
unsupported response, permit/authority expiry, dependency missing, process/IPC failure,
history pending and handling unresolved. Include bounded elapsed time, bytes/records and
reserved/possibly-consumed operation counts with uncertainty. No free-text detail, dynamic
metric labels, raw debug mode or values encoded into codes/counters. Operator text derives
from these admitted fields; infrastructure errors cannot serve as a raw fallback channel.

The egress process receives issued safe context independently from the parent and a bounded
candidate channel from capture. It reconstructs allowed output, rechecks every key/value,
purpose/class, range, attempt/phase/mode and actual request reference, and exports only the
closed profile projection. Capture cannot forge parent control or select arbitrary output
bytes. Invalid candidate bytes stop/discard inside this boundary and yield category-only
unknown/withheld. Independent core validation remains defense in depth, not first sanitization.

### Containment and disposal assumptions

Capture has no disk/temp/cache mounts, swap, packet/body logging, tracing/profiling,
heap/crash dump, runtime debug endpoint or telemetry exporter. No Go panic text leaves
the boundary; diagnostic output is not a fallback result channel.
ADR-015 specifies the OS/container qualification and restricted environment.

Assurance covers normal/malformed/timeout/OOM/cancel/crash behavior, hostile network input
and an unprivileged capture process attempting arbitrary output. Malicious substitution of
the released exporter and compromise of that trusted egress implementation are residual
TCB risks, not risks disproved by an image digest. Host administrator/kernel/hypervisor
compromise and physical RAM forensics remain outside this initial model. Binary provenance,
parser qualification and synthetic sink checks are required; no universal covert-channel
or zero-exposure guarantee is made. A credible egress escape is contamination, not safe data.

Dispose controlled buffers where possible; terminate and reap the entire worker boundary,
close pipes/sockets, and validate the actual no-swap/no-dump/no-storage configuration and
absence of captured bytes in observable ordinary sinks. Process exit alone is insufficient.
Go allocator/stack/TLS copies prevent a claim of forensic byte erasure.
A qualified disposition may state access ended with no ordinary retention under the accepted
coverage; never claim absolute irretrievability. Failure to establish that bounded disposition,
a lost watchdog, unexpected snapshot/dump or credible escape creates an unresolved incident.

### Contamination and remediation authority

Stop affected propagation, reads, reasoning and exports immediately. Append only a random
incident ID, attempt/profile references, affected sink/claim references, category and disposition.
Revoke affected material eligibility and request each owner to correct dependent use.
Do not copy or fingerprint the suspected sensitive value into incident records.

Campaign runtime has no remediation or audit-edit privilege. A separately authorized
maintenance identity, outside campaign execution, may neutralize prohibited bytes in an
identified surface only while preserving safe linked accountability. M1 supplies the hold/
incident contract; it provides no automated database/audit/backup purge capability.
If copies, backups or external recipients are unavailable, leave residual risk unresolved.
That blocks incident closure and claims depending on validated disposal, not unrelated history.

Proof fingerprints/keys and operational custody remain unimplemented. The accepted lane
rebinds ADR-011's ADR-010 prerequisite only for this non-proof outcome; it neither accepts
ADR-010/012 nor supplies later custody or proof protection.

## 5. Consequences

Closed projections discard potentially useful detail but preserve the M1 mission question.
Deployment qualification is a real acquisition prerequisite. Native Windows qualification
and broader raw-content/proof/custody handling require separate accepted coverage.
Synthetic sensitive sentinels and fixture-only raw diagnostics use synthetic data, never
client capture. No field whitelist or 16 KiB truncation alone makes a future GET/title safe.

## 6. Invariant compliance matrix

| Invariant | Obligation |
| --- | --- |
| INV-001 | The acquisition egress boundary has no model, collection or audit authority. No universal manager or workflow engine. |
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
