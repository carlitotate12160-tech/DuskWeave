# AGENTS.md

## Mission

Build DuskWeave as a persistent campaign reasoning and adversary-emulation platform with strict separation between cognition, deterministic authority, execution, observation, and proof.

The repository is not a collection of attack scripts.

It is a campaign operating system.

---

## Project isolation and skill routing

Use only DuskWeave sources as project authority. Do not load BlackBread skills
or import another project's decisions, role names, contracts, or milestones.
Unrelated memory is not DuskWeave authority.

Authority order:
PRODUCT / authoritative PRD > ACCEPTED ADR > DOMAIN CONTRACT > QUALITY_BAR.md
> AGENTS.md > SKILL.md > IMPLEMENTATION.
Drafts do not override accepted authority.

Use .agents/skills/duskweave-engineering/SKILL.md for architecture and packet
preparation; .agents/skills/build-duskweave/SKILL.md for executing assigned
DESIGN, IMPLEMENT, and FIX packets; and
.agents/skills/duskweave-adversarial-review/SKILL.md for a distinct review pass.
All three are subordinate workflow aids.
Review-only requests do not authorize edits. Installed copies do not override
current repository authority; report material mismatches.

For document-only DESIGN work, validate document conventions, exact file scope,
links, terminology, transitions, ownership, and cross-document invariants.
Runtime TDD/checks apply to implementation, not to nonexistent runtime scaffolding.
Do not self-accept draft documents or change seals merely because checks pass.
Follow the assigned packet's file map and stop at its boundary.
The engineering partner resolves architecture before handing a cohesive packet
to the IDE. The IDE executes its bounded outcome, makes routine local choices,
and reports missing decisions; it does not redesign product architecture or
replace implementation with another plan.

Startup guidance: docs/workflows/START_HERE.md.
Project Instructions text: docs/workflows/PROJECT_INSTRUCTIONS.md.

## Required reading order

Before making changes, read:

```text
1. AGENTS.md
2. docs/ENGINEERING_STATE.md
3. docs/BUILD_ORDER.md
4. relevant PRD
5. relevant accepted ADR
6. relevant domain contracts
7. QUALITY_BAR.md
8. relevant skill under .agents/skills/ and the assigned packet
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

Apply PRD-000 INV-007: blind campaign reasoning may adapt to genuinely campaign-visible effects and defender telemetry legitimately obtained from a validated, authorized campaign position. Privileged defender/Observer/Grader feeds remain outside blind reasoning; separately authorized defender-informed exercises are labeled and evaluated apart.

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

## Wiring and reachability for every IDE delivery

For each runtime IMPLEMENT or FIX packet, trace every changed production
component from an existing authorized entrypoint through real caller/consumer
boundaries to an observable result. Include its registration/import,
construction/injection, data or event flow, and failure path where applicable.
A unit test or export alone does not prove production reachability.
Write an integration or contract test through the real entrypoint/consumer
when the behavior can be exercised at this stage.

Audit the changed-scope symbols, modules, event producers/consumers, migrations,
configuration flags, and adapters for dead code and disconnected islands.
Remove unused changes or wire them within the packet's file map. If required
wiring or tests need files outside that map, STOP with SPLIT_REQUIRED and name
the exact missing files; do not add test-only wiring or speculative scaffolding.
Do not declare runtime delivery complete with an orphaned component.
DESIGN packets validate document links, registry/authority references, and
cross-document semantics; runtime reachability is N/A until runtime exists.

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
