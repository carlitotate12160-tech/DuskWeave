# 2. Stage 0 — Repository authority bootstrap

Selesaikan sebelum PRD implementation detail.

Files:

```text
AGENTS.md
QUALITY_BAR.md
.agents/skills/build-duskweave/SKILL.md
docs/BUILD_ORDER.md
docs/ENGINEERING_STATE.md
docs/prd/README.md
docs/adr/README.md
```

`SKILL.md` at repository root is a compatibility pointer only; the canonical skill content lives at `.agents/skills/build-duskweave/SKILL.md`.

Tidak ada runtime code pada stage ini.

Seal:

```text
DW-BOOTSTRAP-001
```

Exit criteria:

```text
authority hierarchy documented
build order documented
God-object rules documented
module-size rules documented
ADR/PRD conventions documented
```

---

# 3. Stage 1 — Product thesis

Author:

```text
PRD-000-product-thesis.md
```

Harus menjawab hanya:

```text
What is DuskWeave?
Who is it for?
What problem does it solve?
What is explicitly outside scope?
What makes campaign emulation different from vulnerability scanning?
```

Jangan masukkan:

```text
database schema
Rust structs
tool CLI
network protocols
```

Target:

```text
4–6 pages maximum
```

Seal:

```text
DW-PRD-000
```

---

# 4. Stage 2 — Campaign semantics

Author secara berurutan:

```text
PRD-001-campaign-lifecycle.md
PRD-002-cyber-terrain.md
PRD-003-access-and-footholds.md
PRD-004-expansion-loop.md
PRD-005-objective-loop.md
PRD-006-adaptation.md
```

Dependency:

```text
PRD-001
   ↓
PRD-002
   ↓
PRD-003
   ↓
PRD-004
   ↓
PRD-005
   ↓
PRD-006
```

Jangan mengerjakan `PRD-006` sebelum semantics previous state jelas.

---

# 5. Stage 3 — Foundation ADRs

Baru setelah PRD-000..006 accepted.

Author:

```text
ADR-001-modular-monolith.md
ADR-002-domain-boundaries.md
ADR-003-domain-events.md
ADR-004-rust-core-language.md
ADR-005-postgres-system-of-record.md
ADR-006-cyber-terrain-storage-model.md
ADR-007-foothold-and-path-separation.md
```

Dependencies:

```text
PRD-001..006
       ↓
ADR-001
       ↓
ADR-002
       ↓
ADR-003
```

Language decision:

```text
ADR-004
```

Storage decisions:

```text
ADR-005
ADR-006
```

Graph separation:

```text
ADR-007
```

Tidak ada tool integration pada stage ini.

---
