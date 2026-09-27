# DuskWeave bounded packet template

Use only when authoring a packet. Fill every field from verified DuskWeave sources.
Do not publish placeholders as ready-to-execute instructions.

PROJECT: DuskWeave
MODE: DESIGN | IMPLEMENT | FIX
PACKET: exact identifier
WORKSPACE: verified path
EXPECTED_BASE_SHA: verified full commit
DELIVERY: exact requested artifacts

Execute the assigned packet directly. Do not return another implementation plan.

## Preconditions
- Exact base, branch/worktree rules, and verified current stage.
- Required accepted authority and sealed dependencies.
- Relevant conflicting work check, if required.
- Required tools and verification capability.

## Authority read
List only applicable files in the repository-mandated order.
Distinguish accepted authority from documents this packet is tasked to draft.

## Outcome and bounded context
State one reviewable outcome, ownership, and behavior.
Define necessary assumptions and failure semantics.
Give exact acceptance criteria; define what evidence proves completion.

## Allowed files
List exact paths. Include required test/consumer changes established by inspection.
Do not authorize arbitrary wildcard edits.

## Required work
Use concise, directly executable steps.
Keep product semantics in PRDs and architectural decisions in permitted ADRs.
Do not duplicate entire authority documents in the prompt.

## Non-goals
List later-stage work and adjacent changes that must not be performed.

## Validation
DESIGN: scope, document conventions, semantic consistency, links, invariants.
IMPLEMENT/FIX: behavior/negative controls and exact repository check commands.
Do not invent test commands for nonexistent runtime scaffolding.

## Review
One adversarial pass, fix valid in-scope findings, final changed-result review.
Distinguish false positives from valid out-of-scope blockers.

## STOP
Pin exact base-drift, authority-conflict, dependency, scope, and budget conditions.
Use SPLIT_REQUIRED / AUTHORITY_CONFLICT / DESIGN_DRIFT as appropriate.

## Completion
Report base/head, actual artifacts, actual verification, findings, and blockers.
State whether work is authored, reviewed, accepted, merged, or sealed.
Do not grant acceptance or claim seal transitions implicitly.
Stop. Do not execute the next packet.
