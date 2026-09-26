# AGENTS.md

## Mission

Build DuskWeave as a persistent campaign reasoning and adversary-emulation platform with strict separation between cognition, deterministic authority, execution, observation, and proof.

The repository is not a collection of attack scripts.

It is a campaign operating system.

---

## Required reading order

Before making changes, read:

```text
1. docs/BUILD_ORDER.md
2. docs/ENGINEERING_STATE.md
3. relevant PRD
4. relevant accepted ADR
5. relevant domain contracts
6. QUALITY_BAR.md
```

Do not infer architectural authority from existing implementation when PRD/ADR says otherwise.

---

## Core architectural invariants

Never create a God Object.

Keep separate:

```text
CyberTerrain
FootholdGraph
AttackPathView
ObjectiveState
CampaignTrajectory
```

Never expose arbitrary execution directly to reasoning workers.

Never treat observations as verified facts without reconciliation/evidence.

Never persist raw credential material, customer records, financial records, or sensitive authentication stores.

Campaign reasoning cannot consume defender detections during active execution.

Campaign capabilities cannot alter authoritative audit evidence.

---

## Language ownership

Production baseline:

```text
Rust:
domain core
campaign engine
terrain
footholds
pathing
objectives
evidence
authority
execution broker

Go:
tool adapters
collectors
network/integration workers
telemetry adapters

Zig:
only when a concrete low-level native-helper requirement justifies it

C/C++:
FFI or unavoidable external SDK integration only

Python/Nim:
research plane unless an accepted ADR promotes a specific component
```

Never introduce another production language without an ADR.

---

## Dependency discipline

Direction:

```text
domain
↑
application
↑
adapters
```

Domain code cannot import tool adapters, databases, HTTP clients, LLM clients, telemetry backends, or OS-specific execution implementations.

Bounded contexts communicate through:

```text
typed contracts
commands
events
ports
```

not internal implementation imports.

---

## No generic dumping grounds

Avoid:

```text
utils/
helpers/
common/
managers/
misc/
```

unless responsibility is narrow and explicit.

`dw-types` may contain only genuine primitives.

Do not move business logic into shared modules to bypass module-size rules.

---

## Size discipline

Preferred source module:

```text
< 300 LOC
```

Architecture review threshold:

```text
400 LOC
```

Large modules must be split by cohesive responsibility.

Test modules are also expected to remain readable and bounded.

---

## Slice discipline

One slice should implement:

```text
one architectural concern
+
one safety-complete vertical behavior
+
tests
```

Do not turn a slice into a multi-domain refactor.

If scope expands materially, return:

```text
SPLIT_REQUIRED
```

---

## Development workflow

Required sequence:

```text
AUTHORITY CHECK
→ TDD
→ IMPLEMENT
→ LOCAL CHECKS
→ SELF-REVIEW
→ ARCHITECTURE CHECK
→ PR
→ ONE ADVERSARIAL REVIEW
→ FIX VALID FINDINGS
→ FINAL CURRENT-HEAD CHECK
→ MERGE
```

---

## Reasoning workers

Reasoning workers should be short-lived specialists.

Examples:

```text
MissionInterpreter
TerrainAnalyst
AccessStrategist
ExpansionPlanner
ObjectiveAnalyst
ChainComposer
AdaptationPlanner
```

Each receives a narrow ContextPack.

Never create a universal `Agent` with access to all repositories, tools, memory, telemetry, and execution.

---

## Execution rule

The permitted flow is:

```text
Reasoning
↓
Proposal
↓
Deterministic validation
↓
Capability Gateway
↓
Execution Broker
↓
Adapter
```

Not:

```text
LLM
↓
shell
```

---

## Sensitive-data rule

Raw sensitive material must remain in an ephemeral sensitive boundary.

It must not be:

```text
serialized
logged
stored
sent to an LLM
included in tracing
written to crash diagnostics
included in reports
```

Persistent proof must use approved opaque proof material.

---

## Scenario rule

Threat scenarios may consume generic capabilities.

They may not become new architecture.

If an APT41-, Lazarus-, or Volt-derived scenario requires adding actor-specific logic into CampaignKernel:

```text
STOP
```

and redesign the generic domain.

---

## STOP conditions

Stop implementation when:

```text
authority is ambiguous
required PRD/ADR does not exist
architecture invariant would be violated
new cross-domain dependency appears
God Object pressure appears
slice expands beyond its sealed purpose
```

Return a concise architecture finding instead of improvising.
