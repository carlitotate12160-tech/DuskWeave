# DuskWeave bounded packet template

Use only when authoring a packet. Fill every field from verified DuskWeave sources.
Do not publish placeholders as ready-to-execute instructions.

PROJECT: DuskWeave
MODE: DESIGN | IMPLEMENT | FIX
PACKET: exact identifier
WORKSPACE: verified path
EXPECTED_BASE_SHA: verified full commit
EXPECTED_START_HEAD: verified full commit
DELIVERY: exact requested artifacts

Execute the assigned packet directly. Do not return another implementation plan.

## Preconditions
- Exact base, branch/worktree rules, and verified current stage.
- Required accepted authority and sealed dependencies.
- Relevant conflicting work check, if required.
- Required tools and verification capability.
- Supported/tested OS, architecture, toolchain, database and runtime role.
- Measured starting cumulative budget, estimated readable change and remaining
  room. Resolve known infeasibility before issuing the packet, including an
  authorized scoped exception if needed; do not promise an unmeasured fit.

## Authority read
List only applicable files in the repository-mandated order.
Distinguish accepted authority from documents this packet is tasked to draft.

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

## Non-goals
List later-stage work and adjacent changes that must not be performed.

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
State mandatory environment variable names without values; prohibit hardcoded
secret defaults and ordinary-surface secret diagnostics. Separate infrastructure
configuration from campaign custody. Do not claim untested platform support.
Keep required deterministic checks in CI when runtime is introduced; verify
actual current-head results and skipped tests. Do not weaken a gate to fit scope.

## Review
One adversarial pass, fix valid in-scope findings, final changed-result review.
Distinguish false positives from valid out-of-scope blockers.
Distinguish author self-review from an independent session. Evaluate changed
behavior and assertions, not test count or scanner completion alone.

## STOP
Pin exact base-drift, authority-conflict, dependency, scope, and budget conditions.
Use SPLIT_REQUIRED / AUTHORITY_CONFLICT / DESIGN_DRIFT as appropriate.

## Completion
Report base/head, actual artifacts, actual verification, findings, and blockers.
State whether work is authored, reviewed, accepted, merged, or sealed.
Record command/results, candidate SHA, platform/DB/role and local/CI provenance.
Missing evidence is UNVERIFIED/BLOCKED. State residual limits and the exact next
action; do not substitute enterprise/military labels for assurance evidence.
Do not grant acceptance or claim seal transitions implicitly.
Stop. Do not execute the next packet.
