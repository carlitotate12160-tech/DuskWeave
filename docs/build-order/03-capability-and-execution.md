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
