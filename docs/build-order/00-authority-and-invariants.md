# 0. Authority hierarchy

Urutan authority:

```text
1. PRODUCT THESIS / PRD
2. ACCEPTED ADR
3. DOMAIN CONTRACT
4. QUALITY BAR
5. AGENTS.md
6. Relevant DuskWeave SKILL.md under .agents/skills/
7. IMPLEMENTATION
```

Aturan:

- `PRD` menentukan **WHAT / WHY**.
- `ADR` menentukan **architectural HOW**.
- `Domain contract` menentukan interface yang boleh diimplementasikan.
- `AGENTS.md` menentukan cara contributor/model bekerja.
- `.agents/skills/build-duskweave/SKILL.md` menentukan workflow reasoning/implementation.
- Code tidak boleh menciptakan architecture baru tanpa authority di atasnya.

`AGENTS.md` dan `.agents/skills/build-duskweave/SKILL.md` tidak boleh mengubah keputusan PRD/ADR.

---

# 1. Hard invariants

Semua fase tunduk pada invariant berikut.

## INV-001 — No God Object

Tidak boleh ada object/service seperti:

```text
CampaignManager
SystemManager
AgentManager
GlobalContext
WorldManager
ToolManager
```

yang mengetahui sebagian besar sistem.

---

## INV-002 — Separate operational models

Lima model harus tetap terpisah:

```text
CyberTerrain
FootholdGraph
AttackPathView
ObjectiveState
CampaignTrajectory
```

Tidak boleh digabung menjadi satu `CampaignState` raksasa.

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

LLM tidak memperoleh arbitrary execution authority.

---

## INV-004 — Observation != fact

Observation harus melalui:

```text
Observation
→ validation/reconciliation
→ state delta
```

Inference tidak boleh diam-diam berubah menjadi fact.

---

## INV-005 — Sensitive data zero-retention

Raw:

```text
credential material
customer records
financial records
authentication stores
```

tidak boleh mencapai persistence layer.

Proof harus diturunkan sebelum persistence.

---

## INV-006 — Audit integrity

Campaign capability tidak boleh:

```text
delete authoritative evidence
alter audit evidence
disable authoritative audit
falsify audit records
```

---

## INV-007 — Defender isolation

Campaign reasoning tidak memperoleh real-time:

```text
EDR verdict
AV verdict
SIEM alert
SOC response
```

untuk mengoptimalkan adaptive evasion.

Control-gap assessment dilakukan oleh Observer/Grader.

---
