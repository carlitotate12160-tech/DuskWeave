# QUALITY_BAR.md — DuskWeave Quality Standards & Budgets

## Purpose

This document defines the software engineering quality standards, complexity thresholds, module size budgets, and testing discipline that all code in the **DuskWeave** repository must meet.

---

## 1. Module & File Size Budgets

Readability, maintainability, and domain-responsibility isolation are enforced through file size limits:

| Metric | Ideal Threshold | Hard Cap | Action When Exceeded |
| :--- | :--- | :--- | :--- |
| **Module/File size** | `< 300 LOC` | `400 LOC` | Must be split by domain responsibility (*split by responsibility*). |
| **Runtime diff per PR/slice** | `< 300 LOC` | `400 LOC` | Must be split into incremental slices (*SPLIT_REQUIRED*). |
| **Cyclomatic complexity (McCabe)** | `<= 7` per function | `7` per function | A value `> 7` violates the budget; refactor by cohesive responsibility. |

Recorded delivery-budget exceptions (scoped, non-recurring; duplicate records are invalid):

M0A_RUNTIME_DIFF_EXCEPTION: DW-IMPLEMENT-M0A; base=a9da047b2bf0bf4822536187ab3e1734ac56fc12; cap=1200; initial-only
(History: originally pinned to base f52cfcc53c183f7c852fba191f9da14858c7ae5d; repointed to a9da047 after protected master advanced via documentation-only PR #6 containing no runtime changes.)

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
  A budget exception requires explicit scoped authorization, not executor discretion.
- Keep existing caps. Never shrink required behavior, remove explanatory comments,
  compress formatting or perform unrelated/out-of-map cleanup to satisfy a counter.
  Small files and low complexity are checks, not proof of sound ownership.
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
