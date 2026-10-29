# AGENTS.md

## Mission

Build DuskWeave as a persistent campaign reasoning and adversary-emulation platform with strict separation between cognition, deterministic authority, execution, observation, and proof.

Preserve authorized offensive campaign behavior: active validation, access,
expansion, adaptation and objective proof under the accepted product contracts.
Non-destructive does not mean read-only. Low unnecessary footprint does not
turn the platform into passive monitoring or a fully defensive product.
Current milestone and campaign authority still bound implementation/execution.

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

## Repository prose

Write IDE execution packets and new or edited repository documentation,
comments, and docstrings in English. Conversation with the owner may use
Indonesian. Translate conversational instructions before placing them in the
repository; preserve technical identifiers and accepted semantics.

## Walking skeleton delivery

Build the smallest complete entrypoint-to-history behavior first, then replace
external-effect adapters incrementally. The selected autonomous sequence is in
[the existing MVP roadmap](docs/MVP_AND_DEFERRED_SCOPE.md#12-autonomous-campaign-delivery-order-owner-direction-2026-10-19).
The application supervisor, authority checks, owner reconciliation and restricted
PostgreSQL path stay real; deterministic provider/effect fixtures are explicit,
isolated test doubles. Fixture success proves orchestration, not LLM cognition,
live acquisition or a milestone seal. Keep that runnable regression in later slices.

Measure slices by observable behavior and failure completeness. Target one
cohesive PR for the first skeleton; file/diff LOC alone cannot force precursor
PRs or helper fragmentation. Retain the enforced McCabe ratchet.
Prefer owner-local unit tests and integration tests through application contracts
for internal logic. Do not expose private APIs or add public CLI commands only
for testing. Keep a real-entrypoint test for operational commands such as m1-run;
fixed internal child-phase commands are not extra operator workflow steps.
Retire an old command only after its actual operational/recovery consumers and
accepted compatibility obligations have a tested replacement.

Use bounded synthetic spikes to resolve empirical uncertainty before claiming
capacity or latency fit. Record sample size, configuration, complete wire bytes,
timing phases and uncertainty; observed percentiles are not legal input bounds.
Policy ceilings may be selected conservatively before calibration but must be
identified as policy, never measured fit. No spike reuses old live consent.

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

Establish these sources at the start of a task/session. On subsequent edits,
recheck repository identity, HEAD, worktree status and relevant authority changes;
reuse unchanged verified context and reread changed/newly affected boundaries.
After context loss, rebuild the required context. Do not reread unrelated stages.
Current acceptance/seal status comes from docs/ENGINEERING_STATE.md; old memory,
startup examples and portable skill copies cannot establish current authority.

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

Never place raw client content or operational authentication material in core state, ordinary persistence, logs, evidence, operator surfaces, or reasoning context. An explicitly authorized operational secret may be retained and reused only inside isolated campaign-scoped secret custody under PRD-000 INV-005; the core receives only an opaque reference and safe metadata.

Apply PRD-000 INV-007: blind campaign reasoning may adapt to genuinely campaign-visible effects and defender telemetry legitimately obtained from an authorized current campaign position, subject to source, epistemic status, and sensitive-data rules. Privileged defender/Observer/Grader feeds remain outside blind reasoning; separately authorized defender-informed exercises are labeled and evaluated apart.

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

Production/tooling file review reference point:

```text
400 LOC
```

Large modules should be split by cohesive responsibility. Test/benchmark files
have a 500-line threshold; Markdown has a separate 600-line reference point, subordinate to any
narrower document-specific rule. See QUALITY_BAR.md for canonical policy.

First-party source-file LOC growth is enforced, not advisory:
`python scripts/check_file_loc.py --base <commit>` fails on a new
over-threshold file, a compliant file crossing its threshold, or net growth
of retained over-threshold debt, and runs in the required metrics CI job.
Retained oversized files may stay or shrink and remain reported as debt.
Markdown, declarative configuration/manifests/workflows, function span and
the cumulative runtime diff stay advisory. Prefer runtime diffs below 300;
above 400 is an advisory review trigger; 600 is a reference point. An
advisory crossing produces a warning and a reviewer decision: retain with a
concrete cohesion/readability reason, or refactor in the same PR. Numerical
crossings alone do not require an exception, a new packet, or SPLIT_REQUIRED.

Apply QUALITY_BAR.md's conditional maintenance disposition before handoff and in
review. The architect resolves material mechanism, consumer/file scope and retirement;
the executor retains routine local choices. Do not silently change those decisions
or add dependencies, operator commands or operational steps.

McCabe complexity is the second enforced ratchet: a per-function ratchet, not
an advisory metric. New functions above threshold 7 fail the required CI gate,
previously compliant functions crossing above 7 fail, and retained debt may
stay or decrease but never increase. The single reviewed pure-dispatch
`src/main.rs::run` holds cap 10. QUALITY_BAR.md is canonical for the ratchet
rules; do not raise a threshold to make a PR green.

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

## Evidence-based review

Apply QUALITY_BAR.md section 7 in author self-review and the existing distinct
adversarial review: ownership, dependencies, simplicity, single truth,
requirement fit, reachability, failure integrity, and trust/claims. Record
candidate-bound evidence and concrete N/A reasons; distinguish findings from
missing proof. Apply the quality bar's function-length review trigger and pure
dispatch classification without count-driven helper fragmentation. Review fix
deltas unless changed boundaries or unresolved risk invalidates earlier evidence.
QUALITY_BAR.md also owns the canonical evidence rules: behavior changes
demonstrate a relevant failing assertion, refactors report passing baselines,
documentation uses document/authority checks, and local iteration runs focused
affected checks while reusing unchanged exact-candidate evidence. Required
committed-head CI and coverage floors stay mandatory as configured. Cite
existing accepted decisions rather than re-litigating them without new
evidence; one cohesive outcome may close several related issues. Authority
loading stays incremental under the required reading order above.

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

## Source workspace and candidate identity

Before interpreting repository state, inspect `git worktree list` and verify
repository identity, the actual worktree, its branch and HEAD against the
packet's expected base.

For ordinary repository changes, Git is candidate custody: the actual base
commit, committed PR head, git diff and CI associated with that candidate
identify the work. Requirements for index-tree identities, patch hashes,
projected source snapshots, candidate manifests and duplicated source archives
are removed. Retain external evidence only where Git cannot reproduce the
observation, such as a live provider result, kept sanitized and identifying
the tested commit. Local checks on uncommitted changes are local evidence;
they do not qualify a later committed candidate automatically.

Preserve unrelated work: unless a packet explicitly assigns the cleanup, do
not clean, reset, stash, stage, promote or delete drafts or worktrees. A
healthy clean worktree may be reused for sequential delivery after its branch
and base are reverified; creating a new folder for every slice is not itself
a quality requirement.

Local machine configuration such as `.cargo/config.toml` belongs to its own
worktree. Do not read, print, copy, stage, delete or overwrite it, and do not
propagate local configuration between worktrees.

---

## Documentation ownership and delivery tracking

Maintain one owner for each kind of information:
- PR state, candidate SHA and CI results: GitHub.
- Active work, next action and milestone/seal evidence index: docs/ENGINEERING_STATE.md.
- Product scope, priorities and retirement triggers: docs/MVP_AND_DEFERRED_SCOPE.md.
- Dependency navigation: docs/BUILD_ORDER.md; decisions remain in their PRD/ADR/contract.
- Supported operator procedures: the relevant RUN_* guide.

Link to the owner instead of repeating status, rules or merge narratives.
Update tracking only when its owned information changes. At preflight, verify
predecessor merges from GitHub; do not create a separate reconciliation PR for
every merge. Every packet declares UPDATE with exact files/effects or NO_CHANGE
with a reason. An unmerged candidate never records its own merge or HEAD SHA.
Preserve accepted Foundation/M0 seal facts and referenced evidence.

MVP documentation freeze (owner direction, 2026-10-26): create no new repository
documentation files, packet records, trackers or guides until the owner changes
this freeze. Keep execution instructions and review evidence in the existing PR.
Correct existing owners only for changed behavior, required operation or material
authority conflicts; remove superseded duplication. Existing accepted authority
and unique evidence remain valid. Future document authoring follows the roadmap;
missing future-stage documents do not become prerequisites for the accepted M1 lane.

After delivery, freeze retained packet records as historical. Remove obsolete
executable instructions and patch chronology from the current tree once unique
requirements/assertions have a durable owning reference. Keep a short outcome,
implementation/PR reference, tested-source/evidence provenance, limitations and
an immutable link to the full historical text. Preserve or repair inbound anchors.
Do not keep appending new episodes to closed packets or maintain a merge log here.
Moving intact text to archive/ alone is not simplification. Retire a RUN_* guide
only after its actual operational consumers have a supported replacement.

Review documentation growth in the existing maintenance review; no new gate,
tracker or mandatory document per change. Reduce repeated prose, not required
authority, assertions, incident evidence or readable formatting. Doc/code LOC
ratios are diagnostics, never quotas: do not add code or erase useful documents
to improve a ratio. Skills carry workflow guidance, not project history.

---

## Local build and coverage artifacts

Compiled artifacts are reusable local resources, separate from source
isolation. On the current Windows x86_64 MSVC baseline they follow a
process-scoped convention, not global machine state:

```text
ordinary build/test artifacts: D:/DuskWeave-build/rust-1.94.1/normal
instrumented artifacts:        D:/DuskWeave-build/rust-1.94.1/coverage/<packet-id>
cache qualification reports:   D:/DuskWeave-build/rust-1.94.1/reports/<packet-id>/
```

Normal commands set `CARGO_TARGET_DIR` explicitly; coverage commands
additionally set `CARGO_LLVM_COV_TARGET_DIR` explicitly. Each issued packet
resolves these drafting fields to a concrete coverage path, report path and
cache ownership rule. Never mix instrumented and ordinary artifact
directories. A different platform, toolchain or configuration uses its own
explicitly qualified path.

One owner holds the shared ordinary target directory for the entire
verification sequence, including native CLI subprocess tests and direct
binary use; Cargo's compilation lock alone does not prove whole-sequence
isolation. Do not build, clean or overwrite that target concurrently from
another worktree. A packet's coverage path likewise has a single owner per
run and must not mix concurrent runs.

Reuse valid compiled artifacts; do not routinely `cargo clean`. Preserve
`cargo llvm-cov` profile/workspace cleanup for the selected isolated coverage
path, then run the current candidate's tests. `--no-run` output or an old
report is not current verification, and no cache-reuse guarantee or speedup
claim is valid without measurement.

Set these variables only in the owning process and its child test processes.
The assigned developer entrypoint `scripts/dev.ps1` may configure and restore
process-local settings under these ownership rules; see README.md. Do not use
`setx`, modify user/system environment or local `.cargo/config.toml`, or add
unassigned wrappers/CI changes. Preserve target folders and report effective
paths and incomplete qualification honestly.

---

## Local services and test evidence

A local PostgreSQL instance is a reusable service; test evidence is not
reusable. Inspect the actual owned service before provisioning another:
retain a compatible running container even when its name belongs to an
earlier slice, and start a compatible stopped container with bounded
readiness/qualification. A missing service requires the packet's authorized
pinned setup; a version or ownership mismatch or a port collision is a
blocker, not permission to delete another container. A service is owned when
the issuing packet names a previously provisioned container for reuse or
authorizes new pinned setup; ambiguous ownership is a blocker. Reuse requires
the container to reside on this Docker host with its container/volume
retained. Do not change restart policy or
host configuration outside a packet that authorizes it, and do not install a
container-management subsystem.

Separate test isolation from service lifetime. Each runtime packet names a
fresh disposable database and restricted login, with administrative access
only for fixture setup; reuse fixtures only within their qualified unchanged
configuration. Execute real SQL, migrations, role qualification and scenario
assertions on every current-candidate run; never reuse a production or client
database, blindly import old branch data, or reuse a previous PASS or
coverage report as current evidence. Fixtures that mutate cluster-wide roles
serialize: allow only one test owner for the owned cluster while
role/permission fixtures run, because a separate database alone does not
isolate cluster-role mutations.

Keep credentials inside the authorized environment/secret boundary; never
echo DSNs or password values into prompts, logs or reports. Missing
authorized DSNs are BLOCKED for a required local database gate. No automatic
database, role, container or volume deletion or bulk reset; any later cleanup
requires its own bounded ownership and authorization.

CI provides its own hosted database lifecycle; persistent local reuse does
not establish cross-job CI caches or container persistence.

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

Minimum client content used for proof must remain inside an isolated ephemeral
proof boundary and be discarded after the active derivation attempt.

Separately authorized operational authentication material may be retained and
reused only inside isolated campaign-scoped secret custody under PRD-000
INV-005. The core and every ordinary surface receive only an opaque reference
and safe non-secret metadata; custody is non-durable by default and may survive
an authorized pause or restart only when resumability expressly requires it.

Neither proof content nor an operational secret value may enter:

```text
core or operational-model state
ordinary persistence, cache, event, or retry payload
logs, tracing, or crash diagnostics
operator or report surfaces
LLM or reasoning context
```

Persistent proof must use approved opaque proof material. Campaign termination,
authorization withdrawal, revocation, invalidation, or the applicable expiry
ends secret eligibility and requires an honest custody disposition.

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
