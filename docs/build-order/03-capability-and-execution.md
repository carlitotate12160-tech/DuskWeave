# 13. Stage 11 — Capability system

PRD:

```text
PRD-011-capability-system.md
```

ADRs:

```text
ADR-013-capability-contract.md
ADR-014-capability-registry.md
ADR-015-execution-boundary.md
```

Introduce:

```text
CapabilitySpec
CapabilityRequest
CapabilityResult
```

Still no arbitrary shell.

---

# 14. Stage 12 — Execution architecture

PRDs:

```text
PRD-012-runtime-isolation.md
PRD-013-native-execution.md
```

ADRs:

```text
ADR-016-execution-broker.md
ADR-017-cross-process-contract.md
ADR-018-native-helper-boundary.md
ADR-019-polyglot-admission-policy.md
```

Runtime ownership:

```text
Rust:
    core
    execution broker
    authority

Go:
    adapters
    collectors
    integrations

Zig:
    selective native helper only

C/C++:
    required FFI only

Python/Nim:
    research plane initially
```

---

# 15. Stage 13 — First safe tool adapters

Only after capability/execution contracts are stable.

First wave:

```text
Nmap
httpx
DNS
osquery
```

Second wave:

```text
Amass
Subfinder
Nuclei
identity graph ingestion
```

Every adapter:

```text
tool-specific input
       ↓
adapter
       ↓
normalized result
       ↓
Observation
```

Tool output never becomes truth directly.

---

# M1 fixed lane — accepted dependency rebind

The [accepted enabling amendment](../contracts/M1-enabling-architecture.md) selects only
CT-existing-v2, parent-selected UDP/TCP DNS-v2 and fixed HTTPS HEAD profiles as the
externally visible effect vocabulary,
Linux capture/egress isolation requiring qualification, current authority/effect fencing, stop-only
fallback, private Rust–Go IPC and durable budget/unknown-effect recovery.
ADR-013/015/016/017 R1 were ACCEPTED by the owner on 2026-10-06 for that lane. The amendment explicitly
binds accepted PRD/M1 product inputs and static profiles instead of claiming that
PRD-011..013 or a dynamic ADR-014 registry already exist or are accepted.
The R2 amendment — direction approved 2026-10-07, published as a candidate — adds the
bounded generated-implementation lane: immutable Go source bundles with declared manifests
are built under isolated toolchain control, qualified against the fixed vocabulary,
campaign-admitted, and dispatched only as typed nested effect requests with current
per-effect admission; a generated worker never becomes the trusted capture/exporter.
The bounded candidate catalog responsibility substitutes for a dynamic registry here;
ADR-014 stays reserved and unwritten.
Full Stage 11/12 prerequisites above remain outside this accepted M1 amendment.
No native helper, extra language, credential/proof handling or first-tool breadth
is authorized. IMPLEMENT remains unissued until accepted-authority publication
and a measured complete runtime packet. Native Windows acquisition
is not qualified by the existing M0 tests or the source worktree location.
