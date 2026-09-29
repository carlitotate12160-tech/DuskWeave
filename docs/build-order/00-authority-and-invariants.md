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

## INV-005 — Sensitive data isolation and campaign-scoped secret custody

Raw client content dan operational authentication material tidak boleh mencapai five operational models, campaign-core PostgreSQL, event/retry payload, log, trace, crash diagnostic, evidence/proof, operator surface, atau reasoning/LLM context.

Client content untuk proof hanya boleh berada dalam isolated ephemeral proof boundary sampai opaque proof diturunkan. Operational secret yang diotorisasi terpisah boleh dipertahankan dan digunakan ulang hanya dalam isolated custody milik satu campaign, dengan scope, current authority, dan finite lifetime; core hanya menyimpan opaque reference serta metadata non-secret. Custody bersifat non-durable secara default dan boleh survive pause/restart hanya bila resumability diotorisasi secara eksplisit. Tidak ada reuse lintas campaign/retest. Expiry, revocation, invalidation, campaign termination, atau authorization withdrawal mengakhiri eligibility dan memicu disposal serta honest disposition. Mekanisme storage, cryptography, recovery, dan sanitization tetap deferred; custody bukan operational model keenam atau general credential vault.

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

## INV-007 — Defender Knowledge Boundary

Definisi otoritatif berada di [PRD-000 §6](../prd/PRD-000-product-thesis.md). Kampanye blind dapat beradaptasi pada efek keamanan yang benar-benar terlihat dari posisi kampanye saat ini; feed istimewa milik defender/Observer/Grader tidak boleh dipakai sebagai oracle kampanye blind. Telemetri defender yang diperoleh secara sah dari posisi kampanye yang diizinkan tunduk pada scope, provenance, rekonsiliasi, dan batas data sensitif. Exercise atau retest yang diberi bounded defender feedback memerlukan otorisasi terpisah, pelabelan, serta evaluasi yang terpisah dari hasil blind. Control-gap assessment tetap dimiliki Observer/Grader.

---
