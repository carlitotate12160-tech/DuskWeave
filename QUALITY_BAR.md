# QUALITY_BAR.md — DuskWeave Quality Standards & Budgets

## Purpose

This document defines the software engineering quality standards, complexity thresholds, module size budgets, and testing discipline that all code in the **DuskWeave** repository must meet.

---

## 1. Module & File Size Budgets

Readability, maintainability, and domain-responsibility isolation are informed by file size and complexity metrics. Two numerical rules are enforced ratchets — first-party source-file LOC growth and per-function McCabe, each governed by its ratchet below. Function span, cumulative runtime diff and Markdown size remain advisory review reference points, not hard ceilings:

| Metric | Preferred Reference | Review Reference Point | Action When Exceeded |
| :--- | :--- | :--- | :--- |
| **Production/tooling file** | `< 300 LOC` | `400 LOC` | **Enforced growth ratchet** for in-scope source files: a new over-threshold file, a compliant file crossing it, or net growth of retained debt fails the required gate; retained debt stays visible and may shrink. |
| **Test/benchmark file** | Readable, cohesive tests | `500 LOC` | Same enforced growth ratchet at the test/benchmark threshold. |
| **Markdown document** | One purpose; navigable | `600 LOC` | Advisory warning; review document purpose. Narrower document-specific limits still apply. |
| **Runtime diff per PR/slice** | `< 300 LOC`; review trigger `> 400` | `600 LOC` | Advisory warning; review cohesion and ownership. Retain with a concrete reason or refactor in the same PR. No separate exception or new packet required. |
| **Function length** | `<= 50` physical lines | Review trigger, not a universal cap | Above 50, record cohesion/readability disposition; split only by meaningful responsibility without helper fragmentation. |
| **Business/other function McCabe** | `<= 7` | `7` | **Enforced ratchet.** A new function above 7 fails the required gate; a previously compliant function crossing above 7 fails; retained debt may stay or decrease but never increase (24->25 fails, 24->23 passes and stays reported). |
| **Pure dispatch McCabe** | `<= 7` preferred | `10` | Enforced ratchet at cap 10 for the single reviewed `src/main.rs::run` direct unit child only; every other function keeps 7. |

Counts are physical lines including comments and blanks. File size uses actual
lines, with or without a final newline; only LF, CRLF (one delimiter) and
lone CR terminate a physical line — U+2028/U+2029, form feed, vertical tab,
NEL and other Unicode/control characters inside text are content, and both
the advisory inventory and the ratchet share this counting convention.
`.sql` files are in the structure
inventory and take the production/tooling threshold; versioned SQL under
`migrations/` stays advisory for size, including new migration files, and
historical migrations are never split or rewritten for a count. Test files are under
`tests/` or `benches/`
or named `tests.rs`; this classification does not grant a new Rust-hygiene exemption.
The test allowance is for test-only code. Naming or placing production code as
a test to evade its cap is forbidden; review verifies actual compilation/wiring.
The runtime diff counts added plus deleted lines in production paths, SQL migrations
and named manifests. Docs, `tests/`, lockfiles and CI-only tooling remain outside
that diff; test modules placed under `src/` still count as source-path changes.

The 600 runtime diff and Markdown file size values are advisory reference
points, not hard ceilings. A crossing produces a warning and a reviewer
decision: retain with a concrete cohesion/readability reason, or refactor in
the same PR. Do not require a new packet, separate authorization, or budget
exception ledger for a numerical crossing. Git history preserves earlier
exception approvals. Architecture, ownership, dependency direction and
readability still matter; an advisory metric does not excuse a God Object or
tangled responsibilities. In-scope file LOC growth and McCabe are the two
enforced numerical ratchets below — do not extract merely to move branches
into forwarding helpers, and do not delete comments, blanks or formatting to
satisfy a number.

### Packet feasibility and early feedback

Apply these five checks to every runtime IMPLEMENT/FIX packet, even when the
request does not mention size or decomposition. Record only task-specific
decisions in the existing file map, checks and review; add no separate artifact.
1. Inspect actual destination sizes, responsibilities, consumers and dependencies
   before assigning editable paths. Resolve known placement conflicts before handoff.
2. For new or extracted modules, name responsibility, owning types/state, real
   caller, narrow visibility, dependency direction and replaced code. Authorize
   necessary cohesive paths up front; do not widen access merely to make a split compile.
3. Use the preferred below-300 production size as planning headroom, not another
   hard cap. Avoid deliberately creating near-cap fragments or speculative modules.
4. After the first complete entrypoint-to-consumer path, run focused compile/
   contract and local LOC checks before multiplying scenarios. Preserve assertions,
   integration-target identity and fixture/resource custody; reuse actual fixture owners.
5. Before handoff, challenge packet feasibility; after implementation, review
   whether a small next change would cross unrelated owners, forwarding chains or
   oversized shared fixtures. Numeric compliance alone does not prove good architecture.

For a routine local change, one maintenance line confirming existing ownership
and no trigger suffices.
Expand only when a destination already exceeds or is expected to cross its category
reference, new responsibilities/state/dependencies/operator steps are introduced,
a mechanism is replaced, or a concrete duplication/ownership/readability risk is
affected. Reassess if actual work reveals a trigger.
When triggered, record four brief facts in the existing packet:

- Selected mechanism versus the simplest viable alternative and the reason.
- Estimated additions/removals for runtime, tests, tooling and docs; distinguish
  measured baselines from estimates. Net LOC alone hides review/replacement cost.
- New responsibilities, or none.
- Mechanism to retire with replacement evidence, none if nothing is replaced,
  or the concrete reason an existing consumer still needs the old mechanism.

Reuse decisions already stated; do not invent an alternative or an essay to fill
a form. For oversized-file growth, address the specific changed scenario and its
simplest viable placement; "same responsibility" or "test-only" alone is insufficient.
Review tests, fixtures and qualification scripts as maintained code under Q1-Q4.
Reject concrete regressions and gate failures; do not demand net-negative
LOC, delete required assertions or fragment files to satisfy a count.
Existing size debt alone does not block delivery. Optional cleanup is nonblocking
and may be deferred; a current correctness or invariant dependency cannot.
Incomplete prose is corrected in the same packet. Only a missing material decision
or required out-of-map edit warrants a scope return; estimates and routine local
choices alone do not. Retain the other genuine STOP conditions below.

### LOC growth ratchet enforcement

`scripts/check_file_loc.py --base <commit>` measures the candidate against a
required resolvable base commit and fails on prohibited first-party
source-file growth. Enforced scope: first-party source/tooling files with
extensions `.rs`, `.go`, `.zig`, `.c`, `.h`, `.cc`, `.cpp`, `.hpp`, `.py`,
`.ps1`, `.sh`, `.sql` — 400 physical lines for production/tooling, 500 under
the existing test/benchmark classification. For LOC exemption only, a
versioned SQL migration is a direct child of `migrations/` whose
POSIX repository-relative path fully matches
`migrations/[0-9]{4}_[A-Za-z0-9][A-Za-z0-9_]*\.sql` (ASCII digits and the
lowercase `.sql` suffix exactly, no descendant directories); any other SQL
file is ordinary in-scope source measured at its category cap. The pinned
`third_party/genai/` snapshot is outside first-party enforcement under its
existing provenance/hygiene controls and grants no other `third_party/` path
a vendor exemption. Markdown, declarative configuration/manifests/workflows,
function span and the cumulative runtime diff remain advisory under their
existing references. `scripts/check_structure.py` keeps its advisory
size/hygiene inventory unchanged; this ratchet removes reviewer discretion
to accept NEW_OVER, CROSSED or REGRESSED growth.

A file at or below its threshold may grow up to it. NEW_OVER (new
over-threshold file), CROSSED (compliant file crossing it) or REGRESSED (net
growth of retained over-threshold debt) fails the gate. An unchanged
oversized file is RETAINED and a smaller one IMPROVED; both remain reported
as debt. Retained debt may cross a pure rename only when exactly one deleted
source and one added destination share identical normalized text and the same
category/threshold; copies, modified moves, ambiguous identities and
category-changing moves are new files at their destination threshold. Without
`--head`, the working tree is measured with real bytes (staged, unstaged and
nonignored untracked files, deletions included); with `--head` an immutable
committed candidate is measured. Exit 0 passes with debt still reported, 1 is
a size regression and 2 is an input/measurement failure — never a zero-count
pass. The required `metrics` job runs it on the same resolved base as the
McCabe ratchet.

A numeric LOC failure is corrected within the allowed outcome by cohesive
extraction or shrinking the added code; it is not itself a new packet or
architecture STOP. Same-size replacement still requires semantic review —
LOC is not a God-Object proof.

### Function and dispatch classification

Count a function from its declaration through its closing brace, including
internal comments, blank lines and nested closures; exclude preceding attributes
and documentation. Apply the 50-line review trigger to production/tooling
functions and methods. Document a longer cohesive function or split a genuinely
mixed responsibility; do not introduce forwarding helpers merely to lower counts.

The 10 McCabe reference point applies to a function that selects an already-owned
handler from a discriminator and forwards its bounded input/result. It must not
validate authority, decide business eligibility, mutate domain state, perform I/O,
add retry/recovery policy, or hide those decisions in match guards. Uncertain or
mixed classification uses 7. The called business handlers retain their own 7 reference point;
review the complete call path so moving branches does not hide complexity.
Only the reviewed `src/main.rs::run` direct unit child currently holds the
dispatch cap; a same-named or nested `run` elsewhere keeps 7, and when the
function changes the review must re-prove pure routing before the cap counts.
No source-digest allowance or hash-pinning is required.
Do not equate match-arm count with McCabe or substitute cognitive complexity.
Report the measurement method/tool/version, source span and exact candidate;
unavailable measurement or analyzer failure is an error, not a numerical PASS.

### McCabe ratchet enforcement

`scripts/check_complexity.py --base <commit>` measures the candidate worktree
and the validated base commit with the same pinned analyzer
(`rust-code-analysis-cli 0.0.25`, checksum-verified in CI) and ratchets per
function/closure. Base sources and inventory come from Git into an isolated
temporary tree; the worktree is never checked out and user-authored baseline
reports are never trusted. Matching uses file + lexical owning scope +
function name — never line numbers or sibling indices. A cross-revision
match is unambiguous only when the function's own key and every enclosing
function/closure key are unique in both revisions; ambiguity propagates to
descendants, so a unique leaf key under an ambiguous ancestor is still
ambiguous. Ambiguous matches (duplicated keys, for example several
anonymous closures under one parent, or members below such ancestry)
inherit no individual allowance. An ambiguous member at or below its
threshold passes. An ambiguous over-threshold member is retained debt only
when its nearest enclosing named function/method that is itself
unambiguous in both revisions — named owners below ambiguous function
ancestry are skipped — has unchanged source span content between base and
candidate (line endings normalized, positions ignored); a changed, absent
or non-unique owner, or an unavailable proof, is blocked ambiguity and
fails the gate. Totals, maxima or sorted-value comparisons never prove
individual members unchanged. Rename/move that cannot be matched is new
code. The gate exits 0 on pass, 1 on regression (new over-cap function,
previously-compliant crossing, increased retained debt, or blocked
ambiguity), and 2 on any input, analyzer, report, or base failure. All retained and improved debt is
still reported for mandatory review, and reviewers must not let an old
allowance justify new responsibility in a hotspot.

A candidate needing a fresh business exception goes back to Work with the
function, reason and smallest remedy; the executor must not raise a
threshold to make its own PR green.

Enforcement wiring:
`scripts/check_complexity.py` (ratchet, exit 0/1/2),
`scripts/check_file_loc.py` (ratchet, exit 0/1/2) and
`scripts/check_runtime_budget.py` (advisory, exit 0 on measurement) run in
the `metrics` job of `.github/workflows/doc-check.yml`, which is required
through `ci-ok`. CI resolves the ratchet base before measurement — PR:
`pull_request.base.sha`; push: `before`; schedule or zero/absent push base:
`HEAD^` when it resolves — and fails explicitly when no base resolves.

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

1. **Evidence-proportional change verification**:
   - A behavior change or fix demonstrates a relevant failing assertion before the fix; the fix then makes it pass.
   - A refactor of already-correct behavior reports passing-baseline characterization and preserved regression evidence; never fabricate RED history.
   - Documentation-only work uses document/authority checks (links, structure, terminology, cross-document consistency); runtime tests are N/A.
   - A test-code change that affects behavior coverage demonstrates the actual weakened or strengthened assertion.
   - Do not fabricate RED for already-correct behavior or require RED for spelling, navigation or unchanged semantics.
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
  fixtures, CI, supported environments and scope. Resolve known
  infeasibility before coding; distinguish estimates from measured counts.
  Each packet identifies its scope, exact file map and estimated diff. >400 is an
  advisory review trigger; 600 is a reference point. Exceeding a reference point
  produces a warning and reviewer decision: retain with a concrete cohesion/readability
  reason, or refactor in the same PR. Do not require a new packet or separate
  authorization for a numerical crossing.
- Architecture, ownership, dependency direction and readability matter. Never shrink
  required behavior, remove explanatory comments, compress formatting or perform
  unrelated/out-of-map cleanup to satisfy a counter. Small files and low complexity
  are checks, not proof of sound ownership.
- During execution, stop for material scope/ownership drift, unresolved authority,
  or a violated architectural invariant; crossing the 400 review trigger or 600 reference
  point alone is not SPLIT_REQUIRED. Never add a precursor solely to save a few counted lines.
  A refactor must be cohesive, directly consumed and supported by behavior-preserving evidence.
- Git is candidate custody for ordinary repository changes: the actual base commit,
  committed PR head, git diff and CI associated with that candidate identify the work.
  Requirements for index-tree identities, patch hashes, projected source snapshots,
  candidate manifests and duplicated source archives are removed. Retain external evidence
  only where Git cannot reproduce the observation, such as a live provider result,
  kept sanitized and identifying the tested commit.
- Bind material acceptance criteria to requirement, owner, observable assertion,
  failure boundary, environment and candidate commit. Report actual commands/results;
  distinguish static review, local execution and exact-candidate CI evidence.
- Exercise real persistence/roles for transaction behavior. For lost acknowledgment,
  assert durable commit before caller failure, then scoped identity/effect count
  after fresh recovery. Pre-commit faults prove a different case.
- Verify environment-sensitive behavior on platforms claimed as supported.
  Record OS/architecture, toolchain, DB and runtime privilege context. An untested
  platform remains unverified; a missing local service is not itself a product bug.
- Local iteration runs focused affected checks. Reuse unchanged exact-candidate
  evidence; rerun when source, inputs/environment or a failure affects the
  claim. Required committed-head CI and the production coverage floors remain
  mandatory as currently configured; do not modify workflows or describe an
  unmerged CI proposal as live.
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
- Reuse machine outputs for LOC/McCabe/coverage and report material warnings,
  decisions and actual test outcomes; no duplicated tables merely for ceremony,
  new ledger, risk taxonomy, decision registry or proof-of-all-invariants claim.
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

Under Q1-Q4, compare the actual change with the applicable maintenance decision.
Inspect its effect on the owning code, including test setup/synchronization/cleanup.
A blocking finding names the concrete regression or required missing evidence and
smallest remedy; optional legacy cleanup is nonblocking. Reuse this review/report.

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
