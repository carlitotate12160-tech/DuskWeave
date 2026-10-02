# DuskWeave bounded packet template

Use only when authoring a packet. Fill every field from verified DuskWeave sources.
Do not publish placeholders as ready-to-execute instructions.
Write task-specific instructions; reference permanent authority instead of
repeating it. Keep concrete contract, failure assertions, protections and gates.
This template adds no prompt-length gate or separate planning deliverable.
These are professional engineering controls, not enterprise certification;
add no arbitrary docstring quotas, extra approval tiers or unrelated
full-suite repetition.

PROJECT: DuskWeave
MODE: DESIGN | IMPLEMENT | FIX
PACKET: exact identifier
WORKSPACE: verified path
BRANCH: actual delivery branch
EXPECTED_BASE_SHA: verified full commit
EXPECTED_START_HEAD: verified full commit
DELIVERY: exact requested artifacts

Execute the assigned packet directly. Do not return another implementation plan.

## Preconditions
- Exact base, branch/worktree rules, and verified current stage.
- Actual worktree identity: inspect `git worktree list` before treating a
  dirty or stale main folder as the active candidate; a recorded path is not
  proof. State how unrelated worktrees and drafts are preserved.
- Required accepted authority and sealed dependencies.
- Relevant conflicting work check, if required.
- Required tools and verification capability.
- Supported/tested OS, architecture, toolchain, database and runtime role.
- Concrete process-scoped artifact paths: ordinary target directory,
  per-packet coverage path and report path, plus the cache ownership rule.
  One owner holds a shared target for the whole verification sequence,
  including CLI subprocess tests; instrumented and ordinary directories stay
  separate. Resolve drafting fields to concrete paths before handoff.
- Actual owned local service state and its qualification: reuse a compatible
  owned service rather than recreating it, name a fresh disposable database
  and restricted login per runtime packet, serialize fixtures that mutate
  cluster-wide roles, and keep credentials in the authorized environment.
  Absent authorized DSNs block a required database gate; they do not waive it.
- Measured formatted cumulative size, exact packet ceiling (at most 600 runtime
  lines under ordinary policy), and room for corrections; normally reserve
  15-20% during planning. Keep production files <=400 and tests <=500.
  Above 400 runtime lines, include the cohesion/ownership review trigger and
  required disposition. That trigger alone does not need an exception or STOP.
  Resolve known infeasibility before handoff; do not promise an unmeasured fit.

## Authority read
List only applicable files in the repository-mandated order.
Distinguish accepted authority from documents this packet is tasked to draft.
Establish context once; on continuation recheck identity/HEAD/status and authority
deltas, then reread changed/newly affected boundaries. Rebuild lost context.

## Outcome and bounded context
State one reviewable outcome, ownership, and behavior.
Define necessary assumptions and failure semantics.
Give exact acceptance criteria; define what evidence proves completion.
Keep the packet cohesive across its necessary files; avoid prompts per module.
Preserve the authorized offensive outcome, meaningful failure behavior and
low unnecessary footprint. Do not replace active validation with a stub or
defensive-only alternative and claim the original requirement is satisfied.

## Allowed files
List exact paths. Include required test/consumer changes established by inspection.
Do not authorize arbitrary wildcard edits.
List read-only authority/callers/consumers separately. Existing wiring need not
be editable. Include only actual required fixture/config/CI changes in edit scope.

## Required work
Use concise, directly executable steps.
The IDE implements the decisions already captured by authority and packet.
Do not ask it to choose a new architecture or return another plan.
Keep product semantics in PRDs and architectural decisions in permitted ADRs.
Do not duplicate entire authority documents in the prompt.
Separate new behavior from existing reuse. Schedule focused tests and an early
formatted cumulative-budget check once the complete production path exists,
before polishing tests/docs. This does not replace pre-handoff feasibility.

## Non-goals
List later-stage work and adjacent changes that must not be performed.

## Tracking disposition
Declare UPDATE with exact tracking files and effects, or NO_CHANGE with a
concrete reason. Include docs/ENGINEERING_STATE.md in the editable map when
active delivery status or the next action changes; include
docs/BUILD_ORDER.md only when current navigation changes. Reconcile the
actual predecessor merge at this packet's preflight; do not issue a ready
packet while a required predecessor merge is still open. A candidate does not
record its own merge and does not embed its final commit SHA in a committed
file; the external delivery report carries candidate provenance. A
review-only packet reports required corrections without editing; an unchanged
status records NO_CHANGE rather than meaningless churn. A needed correction
outside the editable map returns SPLIT_REQUIRED with the exact map defect,
not silent scope expansion.

## Validation
DESIGN: scope, document conventions, semantic consistency, links, invariants;
state runtime wiring N/A.
IMPLEMENT/FIX: behavior/negative controls and exact repository check commands.
List entrypoint -> caller/port -> changed production component -> consumer/output.
Include a test through the real consumer path, or explain stage-specific limits.
Inspect all changed symbols, modules, event producers/consumers, configuration,
migrations, adapters, and errors for dead code or orphan islands.
If necessary consumer/test paths are outside the file map, STOP and split.
Do not invent test commands for nonexistent runtime scaffolding.
Map material criteria to evidence: requirement, owner, observable failure or
success assertion, real boundary exercised, environment and candidate SHA.
Name production checks, failure/recovery tests and relevant negative controls.
Select fuzz/property/concurrency tests only where the changed risk warrants them.
For transaction faults, state whether failure is before commit, after durable
commit before acknowledgment, or during recovery; assert state before recovery.
A behavior fix requires a genuine failing assertion before the fix. A refactor
of already-correct behavior reports passing baseline characterization and
preserved regression evidence; never fabricate RED history or introduce a
production defect to create it. A material mismatch with higher authority is
reported, not silently rewritten.
State mandatory environment variable names without values; prohibit hardcoded
secret defaults and ordinary-surface secret diagnostics. Separate infrastructure
configuration from campaign custody. Do not claim untested platform support.
Keep required deterministic checks in CI when runtime is introduced; verify
actual current-head results and skipped tests. Do not weaken a gate to fit scope.
Schedule one final full pass per unchanged candidate/platform/configuration;
an all-targets coverage run executes that same suite. Retain coverage cleanup,
required platform checks and final-head CI; rerun affected checks after fixes.
Build/service reuse never substitutes for current test results or fresh recovery.

## Review
One adversarial pass, fix valid in-scope findings, final changed-result review.
Review fixes as a delta unless changed authority/boundaries or unresolved risk
invalidates earlier evidence. Form counterexamples before the author's verdict.
Distinguish false positives from valid out-of-scope blockers.
Distinguish author self-review from an independent session. For >400 runtime
lines, explicitly assess one coherent behavior, owner boundaries, file footprint,
failure/recovery coverage and safe state after merge. Green CI is not this review.
Evaluate changed
behavior and assertions, not test count or scanner completion alone.

## STOP
Pin exact base-drift, authority-conflict, dependency, scope, and budget conditions.
Use SPLIT_REQUIRED / AUTHORITY_CONFLICT / DESIGN_DRIFT as appropriate.

## Completion
Report repository identity, actual worktree, branch, base/head, actual
artifacts, actual verification, findings, and blockers.
State whether work is authored, reviewed, accepted, merged, or sealed.
State the tracking disposition (UPDATE with exact effects or NO_CHANGE with a
reason), the candidate delivery state and remaining blockers before handoff,
and review consistency between the current-phase summary, next action and
build navigation.
Record command/results, candidate SHA, platform/toolchain/DB/role, service
reuse/qualification, cache/report provenance, and local/CI provenance.
When available, include approximate preparation/implementation/check time and
rework/STOP cause in the delivery report; no new artifact or timing gate.
Missing evidence is UNVERIFIED/BLOCKED. State residual limits and the exact next
action; do not substitute enterprise/military labels for assurance evidence.
Do not grant acceptance or claim seal transitions implicitly.
Stop. Do not execute the next packet.
