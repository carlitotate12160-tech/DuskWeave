# QUALITY_BAR.md — DuskWeave Quality Standards & Budgets

## Purpose

This document defines the software engineering quality standards, complexity thresholds, module size budgets, and testing discipline that all code in the **DuskWeave** repository must meet.

---

## 1. Module & File Size Budgets

Readability, maintainability, and domain-responsibility isolation are enforced through file size limits:

| Metric | Ideal Threshold | Hard Cap | Action When Exceeded |
| :--- | :--- | :--- | :--- |
| **Production/tooling file** | `< 300 LOC` | `400 LOC` | Split by cohesive responsibility. |
| **Test/benchmark file** | Readable, cohesive tests | `500 LOC` | Split by scenario/responsibility. |
| **Markdown document** | One purpose; navigable | `600 LOC` | Split by purpose; narrower document-specific limits still apply. |
| **Runtime diff per PR/slice** | `< 300 LOC`; review trigger `> 400` | `600 LOC` | Above 400 requires explicit cohesion/ownership review; above 600 is *SPLIT_REQUIRED*. |
| **Function length** | `<= 50` physical lines | Review trigger, not a universal cap | Above 50, record cohesion/readability disposition; split only by meaningful responsibility. |
| **Business/other function McCabe** | `<= 7` | `7` | Refactor a measured violation by cohesive responsibility. |
| **Pure dispatch McCabe** | `<= 7` preferred | `10`, only when qualified below | Review classification and reject business decisions hidden in dispatch. |

Counts are physical lines including comments and blanks. File size uses actual
lines, with or without a final newline. Test files are under `tests/` or `benches/`
or named `tests.rs`; this classification does not grant a new Rust-hygiene exemption.
The test allowance is for test-only code. Naming or placing production code as
a test to evade its cap is forbidden; review verifies actual compilation/wiring.
The runtime diff counts added plus deleted lines in production paths, SQL migrations
and named manifests. Docs, `tests/`, lockfiles and CI-only tooling remain outside
that diff; test modules placed under `src/` still count as source-path changes.

The 600 runtime ceiling is an initial DuskWeave calibration, not an industry
standard or a target to fill. A >400 result is a review trigger, not a budget
exception. The existing distinct review must explicitly address one coherent
behavior, owner boundaries, file footprint, failure/recovery tests and a safe
system after merge. CI reports the trigger; green numerical checks do not resolve
that review or grant acceptance. Source-module and McCabe caps remain independent.

Recorded delivery-budget exceptions (scoped, non-recurring; duplicate records are invalid):

M0A_RUNTIME_DIFF_EXCEPTION: DW-IMPLEMENT-M0A; base=a9da047b2bf0bf4822536187ab3e1734ac56fc12; cap=1200; initial-only
(History: originally pinned to base f52cfcc53c183f7c852fba191f9da14858c7ae5d; repointed to a9da047 after protected master advanced via documentation-only PR #6 containing no runtime changes.)

### Function and dispatch classification

Count a function from its declaration through its closing brace, including
internal comments, blank lines and nested closures; exclude preceding attributes
and documentation. Apply the 50-line review trigger to production/tooling
functions and methods. Document a longer cohesive function or split a genuinely
mixed responsibility; do not introduce forwarding helpers merely to lower counts.

The 10 McCabe allowance applies only to a function that selects an already-owned
handler from a discriminator and forwards its bounded input/result. It must not
validate authority, decide business eligibility, mutate domain state, perform I/O,
add retry/recovery policy, or hide those decisions in match guards. Uncertain or
mixed classification uses 7. The called business handlers retain their own 7 cap;
review the complete call path so moving branches does not hide complexity.
Do not equate match-arm count with McCabe or substitute cognitive complexity.
Report the measurement method/tool/version, source span and exact candidate;
unavailable measurement is UNVERIFIED, not a numerical PASS. Existing fmt/Clippy
success is not proof that these metrics are measured. Analyzer selection and CI
wiring require their own bounded implementation; this policy installs neither.

### Code splitting rules:
- Strictly forbidden to create dumping-ground files such as `utils.rs`, `helpers.rs`, `common.rs`, or `misc.rs`.
- File splits must reflect cohesive sub-domains (for example, separating `transition.rs`, `validation.rs`, `error.rs`).

---

## 2. Invariant & God-Object Prevention

1. **Zero God Objects (INV-001)**:
   - Do not create a struct/class that holds references to more than one major domain aggregate.
   - There must be no `CampaignManager`, `SystemManager`, `GlobalContext`, or similar modules.
2. **Separation of the 5 Operational Models (INV-002)**:
   - `CyberTerrain`, `FootholdGraph`, `AttackPathView`, `ObjectiveState`, and `CampaignTrajectory` have separate owners, mutation rules, and public contracts. Module visibility boundaries and dependency direction must prevent importing another model's implementation or mutable aggregate.
   - Several models may live in one crate or deployable as long as those boundaries remain enforceable; one crate per model is not a requirement. Cross-owner relationships go through typed contracts, bounded views/ID references, or events per ADR-002/003 — not nested mutable structs or direct access to private state.

---

## 3. Testing Discipline (Testing Bar)

1. **Strict Test-Driven Development (TDD)**:
   - Mandatory cycle: **Red (failing test) → Green (minimal implementation) → Refactor**.
   - No feature code may be committed without a test that validates its behavior first.
2. **Invariant Test Coverage**:
   - All invariants (INV-001 through INV-007) must have *negative control tests* (tests proving that a violation of the invariant is definitely rejected).
   - State transition tests must validate both the happy path and illegal state transitions.
3. **Absolute Determinism**:
   - Tests must not depend on arbitrary sleeps/timers, unseeded random execution order, or external networks.
   - Tests must not be left as `skip`, `ignore`, or `allow_failure` without explicit approval.
4. **Line-coverage gate (M0A)**:
   - CI runs the full suite (unit + PostgreSQL 17 integration + permissions + real CLI subprocess tests) once under `cargo llvm-cov` instrumentation; the existing deterministic assertions remain the behavioral authority.
   - Fixed floors: `90%` total production line coverage and `80%` line coverage in every reported production source file. Both are enforced by `tests/check_coverage_gate.py` against that run's JSON report; the thresholds are constants in the gate, not inputs.
   - Denominator: only `src/**/*.rs`. Test files, dependencies, generated code and build output are excluded; a reported file outside `src/` fails the gate. A production source missing from the report fails the gate unless it is declarations-only (no coverable lines).
   - No `#[coverage(off)]`, ignore annotations, or filename exclusions may be used to hide untested production code.
   - Line coverage is not branch coverage and does not establish correctness; the scenario assertions remain mandatory.

---

## 4. Type Safety & Language Design (Rust Focus)

1. **Parse, Don't Validate**:
   - Validate input data at system boundaries; specific types preserve structural invariants through controlled construction and mutation. Freshness, authorization, and environmental conditions are still checked when a decision is used; types do not guarantee the validity of external claims over time.
2. **Typestate Pattern**:
   - Use typestate where it clarifies locally valid transitions, without a generic state machine for the whole campaign. Access candidates are owned by Pathing, not FootholdGraph. A validated-position view does not provide direct execution; every dispatch still passes through current authority, the Capability Gateway, the Execution Broker, and the adapter.
3. **Newtype Pattern**:
   - Prevent ID confusion by wrapping primitive identifiers in strong types (example: `struct EntityId(Uuid)`, `struct FootholdId(Uuid)`).
4. **Explicit Error Handling**:
   - Do not use `unwrap()` or `expect()` on production code paths.
   - All failures must be represented using `Result<T, DomainError>`.

---

## 5. Code Hygiene Standards

- **Formatting**: Must pass `cargo fmt -- --check`.
- **Linting**: Must pass `cargo clippy --all-targets -- -D warnings`.
- **Zero Dead Code / No Islands**: Every runtime change must have a real path from an authorized entrypoint to the new component, a production consumer, and an observable result. An export, an unused registration, or a call that only appears in a unit test is not sufficient.
- **Wiring proof per packet**: The IDE report names entrypoint -> caller/port -> changed component -> consumer/output, plus test evidence exercising that path. Check changed event producers/consumers, adapters, configuration, migrations, and error paths. A genuinely missing path is a blocker; symbols used through dynamic mechanisms need wiring evidence, not a "dead code" dismissal based on text search alone.
- **Scope failure**: Remove unused runtime artifacts or complete wiring within the file map. If the file map does not permit the required consumer/test, STOP with `SPLIT_REQUIRED` and list the exact paths. Do not hide an island in a helper module or write code only to satisfy a test.
- **DESIGN-only**: These checks apply to documents as reference and authority consistency; runtime reachability evidence is N/A until runtime exists.
- **Dependency audit**: External dependencies must be minimal, go through strict curation, and be security-audited (`cargo audit`).


## 6. Assurance evidence and packet feasibility

- Preserve the authorized offensive outcome and PRD-000 invariants. A passive
  substitute or stub is not delivery of an accepted active capability.
- Before issuing a runtime packet, inspect its current source, real consumers,
  fixtures, CI, supported environments and cumulative budget. Resolve known
  infeasibility before coding; distinguish estimates from measured counts.
  Each packet pins its cumulative ceiling at or below 600, an exact file map and
  measured formatted starting/proposed size. Normally reserve 15-20% for corrections
  before handoff; this is planning headroom, not a target to consume. Reassess tight
  estimates before execution. >400 requires the review disposition in section 1,
  not a routine exception. Any exception above the ceiling still needs explicit
  scoped authorization; the executor cannot enlarge it.
- Keep the applicable caps. Never shrink required behavior, remove explanatory comments,
  compress formatting or perform unrelated/out-of-map cleanup to satisfy a counter.
  Small files and low complexity are checks, not proof of sound ownership.
- During execution, stop for material scope/ownership drift or the packet/hard
  ceiling; crossing the 400 review trigger alone is not SPLIT_REQUIRED. Never add
  a precursor solely to save a few counted lines. A refactor must be cohesive,
  directly consumed and supported by behavior-preserving evidence.
- Reassess this initial calibration after five runtime deliveries using measured
  diff/footprint, review findings/rework and stopped slices. Tighten or widen only
  from that evidence, without weakening owner, complexity or failure-test gates.
- Bind material acceptance criteria to requirement, owner, observable assertion,
  failure boundary, environment and candidate SHA. Report actual commands/results;
  distinguish static review, local execution and exact-candidate CI evidence.
- Exercise real persistence/roles for transaction behavior. For lost acknowledgment,
  assert durable commit before caller failure, then scoped identity/effect count
  after fresh recovery. Pre-commit faults prove a different case.
- Verify environment-sensitive behavior on platforms claimed as supported.
  Record OS/architecture, toolchain, DB and runtime privilege context. An untested
  platform remains unverified; a missing local service is not itself a product bug.
- Enforce applicable deterministic runtime checks in CI when runtime is delivered.
  Inspect required-check wiring, skipped jobs/tests and candidate results.
  Green scans do not prove zero alerts or semantic correctness.
- Select additional property/fuzz/concurrency checks for concrete changed risks;
  do not require unrelated tooling or a generic coverage percentage for every slice.
- Keep infrastructure secrets in authorized configuration boundaries; no hardcoded
  defaults or secret-bearing diagnostics. Infrastructure env does not replace
  PRD-010 campaign custody. Preserve safe deterministic test controls without
  presenting them as client pilot proof.
- Treat missing required evidence as UNVERIFIED/BLOCKED, not PASS. Candidate CI
  may supply execution evidence unavailable locally, but cannot waive an explicit
  local gate. Repeat checks when changes or unresolved risks warrant them.
- Separate author review, independent review, owner acceptance, merge and seal.
  Enterprise readiness requires a defined deployment/assurance scope and evidence;
  a prompt or skill cannot certify military compliance.

## 7. Evidence-based architecture review

Apply these eight questions to the changed behavior and its affected callers,
consumers and boundaries in author self-review and the existing distinct
adversarial review. Keep this section canonical; packets and skills reference it.

| ID | Question | Evidence to inspect |
| --- | --- | --- |
| Q1 Ownership | Does each component own one coherent responsibility without absorbing sibling decisions or mutable aggregates? | Named owner, mutation rights, public contracts and actual state access; several small files can still form one God Object. |
| Q2 Dependencies | Is dependency direction explicit, with no cycles, hidden shared mutation or import of a sibling implementation? | Imports, construction/injection and the actual call/data flow; legitimate composition uses narrow ports/views. |
| Q3 Simplicity | Does each helper, wrapper, type and abstraction protect a real boundary or make behavior clearer? | Production use and explanation of the responsibility; reject unnecessary indirection and count-driven fragmentation. |
| Q4 Single truth | Does each business invariant have one owning implementation, with representations and consumers consistent? | Compare decision paths, adapters and persistence; repeated checks at separate trust boundaries may be necessary and are not automatically duplication. |
| Q5 Requirement fit | Does the observable behavior satisfy accepted authority and the sealed packet without scope or semantic drift? | Requirement-to-assertion mapping, allowed files, invariants, compatibility and explicit deferred work. |
| Q6 Reachability | Is every changed runtime component wired from an authorized entrypoint to a real consumer and observable result? | Entrypoint -> caller/port -> component -> consumer/output plus a relevant path test; an export or unit-test call alone is insufficient. |
| Q7 Failure integrity | Do failure, retry, concurrency and recovery preserve the required state and effect invariants? | Applicable deterministic counterexamples for commit/ack boundaries, duplicate/conflicting requests, races, cancellation, migration/permission and resource-limit failures; no blind replay of unknown effects. |
| Q8 Trust and claims | Can any input, stale value, forged artifact or alternate path gain unsupported authority, fact status or sensitive access? | Governing provenance/current-use checks, consumer reachability and applicable INV-003..007 controls; types, digests, coverage and green CI alone do not prove the claim. |

For each applicable question, record a brief disposition and source/assertion
evidence tied to the candidate. Use PASS, FINDING, UNVERIFIED or N/A with a concrete
reason; checkboxes alone are insufficient. Record strongest credible counterexamples
for material claims, not a fixed quota of tests. Distinguish static inspection,
local execution and exact-candidate CI, including their environment and limits.
DESIGN/document-only work checks ownership, semantics, links and authority;
runtime wiring/fault execution is N/A with that reason, not a fabricated test PASS.

Classify suspected findings as VALID, FALSE_POSITIVE or UNVERIFIED and separately
state blocking impact. A valid finding names the violated invariant, reachable
path, consequence and smallest root-cause remedy; a theoretical case is not a
demonstrated defect. Missing required evidence blocks readiness without proving
a product bug. Do not defer a currently false API claim to a future milestone.

Use one complete adversarial pass. After correction, review the delta and affected
claims, rebind evidence to the new candidate and complete required checks. Reopen
the full review only when changed authority/boundaries or unresolved risk invalidates
earlier evidence. Add detailed parser, provenance, concurrency, persistence,
secret-custody or execution cases only for affected responsibilities; do not
re-audit every future milestone. Report READY or NOT READY for the stated review
scope, with actual blockers and residual limits. A structural PASS is not universal
proof of safety, absence of God Objects, or enterprise readiness.
