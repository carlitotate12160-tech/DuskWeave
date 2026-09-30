# 0. Authority hierarchy

Authority order:

```text
1. PRODUCT THESIS / PRD
2. ACCEPTED ADR
3. DOMAIN CONTRACT
4. QUALITY BAR
5. AGENTS.md
6. Relevant DuskWeave SKILL.md under .agents/skills/
7. IMPLEMENTATION
```

Rules:

- `PRD` defines **WHAT / WHY**.
- `ADR` defines **architectural HOW**.
- `Domain contract` defines the interfaces that may be implemented.
- `AGENTS.md` defines how contributors/models work.
- `.agents/skills/build-duskweave/SKILL.md` defines the reasoning/implementation workflow.
- Code may not create new architecture without authority above it.

`AGENTS.md` and `.agents/skills/build-duskweave/SKILL.md` may not change PRD/ADR decisions.

---

# 1. Hard invariants

All phases are subject to the following invariants.

## INV-001 — No God Object

No object/service such as:

```text
CampaignManager
SystemManager
AgentManager
GlobalContext
WorldManager
ToolManager
```

may know most of the system.

---

## INV-002 — Separate operational models

The five models must remain separate:

```text
CyberTerrain
FootholdGraph
AttackPathView
ObjectiveState
CampaignTrajectory
```

They may not be merged into one giant `CampaignState`.

---

## INV-003 — Reasoning != execution

```text
Reasoning Worker
     ↓
Proposal
     ↓
Deterministic Validation
     ↓
Capability Gateway
     ↓
Executor
```

The LLM does not obtain arbitrary execution authority.

---

## INV-004 — Observation != fact

Observation must pass through:

```text
Observation
→ validation/reconciliation
→ state delta
```

Inference may not silently become fact.

---

## INV-005 — Sensitive data isolation and campaign-scoped secret custody

Raw client content and operational authentication material must not reach the five operational models, campaign-core PostgreSQL, event/retry payloads, logs, traces, crash diagnostics, evidence/proof, operator surfaces, or reasoning/LLM context.

Client content for proof may exist only inside an isolated ephemeral proof boundary until opaque proof is derived. A separately authorized operational secret may be retained and reused only inside isolated custody belonging to a single campaign, with scope, current authority, and finite lifetime; the core stores only an opaque reference and non-secret metadata. Custody is non-durable by default and may survive pause/restart only when resumability is explicitly authorized. No cross-campaign/retest reuse. Expiry, revocation, invalidation, campaign termination, or authorization withdrawal ends eligibility and triggers disposal plus honest disposition. Storage, cryptography, recovery, and sanitization mechanisms remain deferred; custody is not a sixth operational model or a general credential vault.

---

## INV-006 — Audit integrity

A campaign capability may not:

```text
delete authoritative evidence
alter audit evidence
disable authoritative audit
falsify audit records
```

---

## INV-007 — Defender Knowledge Boundary

The authoritative definition lives in [PRD-000 §6](../prd/PRD-000-product-thesis.md). A blind campaign may adapt to security effects genuinely visible from the current campaign position; privileged defender/Observer/Grader feeds must not be used as an oracle for a blind campaign. Defender telemetry legitimately obtained from an authorized campaign position is subject to scope, provenance, reconciliation, and sensitive-data limits. Exercises or retests given bounded defender feedback require separate authorization, labeling, and evaluation apart from blind results. Control-gap assessment remains owned by Observer/Grader.

---
