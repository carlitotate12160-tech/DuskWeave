# Start DuskWeave

## Setup scope

This guide navigates the engineering workflows.
PRD-000..002 have been accepted in a separate product decision.
No ADR, runtime, or foundation seal has been completed.
The canonical current state remains docs/ENGINEERING_STATE.md.
Read docs/BUILD_ORDER.md for sequencing.

## ChatGPT Project

1. Use a separate Project named DuskWeave.
2. Paste the complete contents of PROJECT_INSTRUCTIONS.md into Project Instructions.
3. Attach/connect current DuskWeave authority; avoid unrelated project sources.
4. Use duskweave-engineering for design and packet authoring; use
   duskweave-adversarial-review to challenge the completed artifact.
5. Start one conversation per bounded outcome.

Project instructions control guidance; they do not guarantee memory isolation.
Use the memory controls available in your account/workspace.
Treat any unrelated recalled project context as non-authoritative.
The setup file does not automatically modify the ChatGPT Project settings.

## IDE

Open D:/DuskWeave as its own workspace.
Read AGENTS.md and .agents/skills/build-duskweave/SKILL.md explicitly if your
IDE does not discover repository skills automatically.
For architecture use .agents/skills/duskweave-engineering/SKILL.md;
for a distinct review use .agents/skills/duskweave-adversarial-review/SKILL.md.
All three skill directories are self-contained. No other project's skill is needed.

## Source ownership

- Project instructions: scope and durable collaboration behavior.
- AGENTS.md: repository contributor protocol.
- PRDs/accepted ADRs/contracts: product and architecture authority.
- QUALITY_BAR.md: quality gates and budgets.
- Build order: dependency and stage navigation.
- Engineering state: actual acceptance/seal status.
- Skills: architecture, packet execution, and adversarial review workflows.
- Packet: exact bounded authorization for the current task.

Repository copies are canonical for repository procedure.
Installed skill copies enable use in Work but cannot override current repository
authority. If a material version difference appears, report it before execution.
Update the installed copy deliberately when repository workflow changes.

## Next permitted packet

PRD-000 Product Thesis, PRD-001 Campaign Lifecycle, and PRD-002 Cyber Terrain
are ACCEPTED. DW-DESIGN-001.md and its prior pinned execution base are historical.
The next permitted authoring packet is DW-DESIGN-002 (PRD-003..006).
Its combined prompt is docs/workflows/DW-DESIGN-002.md. Pin the invocation
against a newly verified clean HEAD when assigning it.
Do not start ADR or runtime work before the required PRD acceptance and foundation
dependencies are complete.

## Completion and handoff

The IDE reports artifacts, base/head, relevant document or runtime checks,
wiring evidence for runtime changes, findings, and open questions, then stops.
PRD-000..002 acceptance and registry/state updates were recorded after the
product owner's approval. DW-DESIGN-002 is the next permitted authoring packet,
not an automatic continuation.

Use a compact handoff: verified base/head, active packet, files changed,
checks actually run, acceptance status, blockers, and next permitted action.
Do not substitute conversation memory for repository state.

## Setup verification

Check skill frontmatter, resource links, exact file scope, size limits,
cross-document consistency, and preservation of INV-001..007.
No runtime tests apply to this documentation-only setup.
No remote CI, branch protection, PR, or merge is implied by local verification.
