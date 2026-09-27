# Start DuskWeave

## Setup scope

This setup configures engineering workflows only.
It does not accept PRDs, create ADRs, scaffold runtime, or advance a foundation seal.
The canonical current state remains docs/ENGINEERING_STATE.md.
Read docs/BUILD_ORDER.md for sequencing.

## ChatGPT Project

1. Use a separate Project named DuskWeave.
2. Paste the complete contents of PROJECT_INSTRUCTIONS.md into Project Instructions.
3. Attach/connect current DuskWeave authority; avoid unrelated project sources.
4. Choose the installed duskweave-engineering skill for design and review.
5. Start one conversation per bounded outcome.

Project instructions control guidance; they do not guarantee memory isolation.
Use the memory controls available in your account/workspace.
Treat any unrelated recalled project context as non-authoritative.
The setup file does not automatically modify the ChatGPT Project settings.

## IDE

Open D:/DuskWeave as its own workspace.
Read AGENTS.md and .agents/skills/build-duskweave/SKILL.md explicitly if your
IDE does not discover repository skills automatically.
For architecture/review use .agents/skills/duskweave-engineering/SKILL.md.
Both skill directories are self-contained. No other project's skill is needed.

## Source ownership

- Project instructions: scope and durable collaboration behavior.
- AGENTS.md: repository contributor protocol.
- PRDs/accepted ADRs/contracts: product and architecture authority.
- QUALITY_BAR.md: quality gates and budgets.
- Build order: dependency and stage navigation.
- Engineering state: actual acceptance/seal status.
- Skills: how to carry out one kind of work.
- Packet: exact bounded authorization for the current task.

Repository copies are canonical for repository procedure.
Installed skill copies enable use in Work but cannot override current repository
authority. If a material version difference appears, report it before execution.
Update the installed copy deliberately when repository workflow changes.

## Next permitted packet

DW-DESIGN-001 drafts, in dependency order:
1. docs/prd/PRD-000-product-thesis.md
2. docs/prd/PRD-001-campaign-lifecycle.md
3. docs/prd/PRD-002-cyber-terrain.md

Use DW-DESIGN-001.md with an entry prompt pinned to a verified clean HEAD.
The packet does not create ADRs or implement code.
The original pre-setup SHA is historical; do not reuse it as the new execution base.

## Completion and handoff

The IDE reports artifacts, base/head, document checks, invariant review,
findings, and open questions, then stops.
Review the three PRDs against their authority and exact diff.
Acceptance and registry/state updates are a separate authorized step because
DW-DESIGN-001 allows only the three PRD files.
Proceed to DW-DESIGN-002 only after the required acceptance is recorded.

Use a compact handoff: verified base/head, active packet, files changed,
checks actually run, acceptance status, blockers, and next permitted action.
Do not substitute conversation memory for repository state.

## Setup verification

Check skill frontmatter, resource links, exact file scope, size limits,
cross-document consistency, and preservation of INV-001..007.
No runtime tests apply to this documentation-only setup.
No remote CI, branch protection, PR, or merge is implied by local verification.
