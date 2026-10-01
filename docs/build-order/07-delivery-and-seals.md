# 25. Slice rule

Every implementation packet should fit:

```text
one architectural concern
+
one meaningful vertical behavior
+
tests
```

Target:

```text
runtime diff preferably < 300 LOC
> 400 LOC: explicit cohesion/ownership review
> 600 LOC: SPLIT_REQUIRED
```

Do not intentionally grow a slice merely to hit a minimum.

---

# 26. File rule

Preferred:

```text
< 300 LOC/module
```

Hard review threshold:

```text
400 LOC/module
```

When approaching the limit:

```text
split by responsibility
```

not:

```text
move functions randomly to helpers.py
```

The 400 file cap applies to production/tooling; test/benchmark files have 500,
Markdown has 600 unless a narrower document-specific rule applies. Physical
comments/blanks count. File size is independent of the cumulative runtime diff.

Before issuing a packet, measure the formatted cumulative diff against its exact
base, declare a ceiling no higher than 600, and normally retain 15-20% planning
room for corrections. Crossing 400 requires an explicit disposition in the
existing distinct review; CI green alone does not resolve that disposition.
Do not add a low-value precursor or remove failure behavior merely to fit 400.
Split for responsibility, material scope drift or a hard/packet ceiling.

---

# 26.1 Wiring gate for runtime slices

Each runtime slice must connect its changed production behavior to a real
entrypoint and consumer, with an observable output and a relevant test through
that path. Test-only references or unused exports are insufficient. Inspect
changed modules, event flow, configuration, migrations, adapters, and error
paths for dead code and isolated islands. If wiring requires out-of-scope
consumer/test files, return SPLIT_REQUIRED with exact missing paths.
DESIGN-only packets verify authority references and document links; runtime
wiring is N/A.

---

# 27. STOP conditions

The model MUST STOP a slice if:

```text
PRD dependency not accepted
ADR dependency missing
requested change violates invariant
new domain boundary is required
scope expands materially
module would become God Object
runtime diff becomes mini-project
tests reveal architectural mismatch
```

Return:

```text
SPLIT_REQUIRED
```

with reason.

Do not silently expand scope.

---

# 28. Standard implementation loop

Every code slice:

```text
DESIGN AUTHORITY CHECK

↓

TDD

↓

IMPLEMENT

↓

LOCAL CHECKS

↓

SELF-REVIEW DIFF

↓

ARCHITECTURE INVARIANT CHECK

↓

PR

↓

ONE DISTINCT ADVERSARIAL REVIEW PASS

↓

FIX VALID FINDINGS

↓

FINAL HEAD VALIDATION

↓

MERGE
```

---

# 29. Current next action

Read current seal/acceptance status in `docs/ENGINEERING_STATE.md`.
Read the currently permitted packet in `docs/BUILD_ORDER.md`.
Do not restart completed bootstrap work or infer permission to author every PRD.

The product authoring sequence is:
1. DW-DESIGN-001: PRD-000, PRD-001, PRD-002, as dependency-ordered drafts.
2. Acceptance and required registry/state updates through an authorized step.
3. DW-DESIGN-002: PRD-003, PRD-004, PRD-005, PRD-006.
4. Acceptance of all seven PRDs, PRD-000 through PRD-006.

Only after all seven PRDs are accepted:

```text
11. ADR-001 Modular Monolith
12. ADR-002 Domain Boundaries
13. ADR-003 Domain Events
14. ADR-004 Rust Core
15. ADR-005 PostgreSQL SoR
16. ADR-006 Terrain Storage
17. ADR-007 Foothold/Path Separation
```

Do not reverse this order.

---

# 30. First design seal

The first meaningful seal is:

```text
DW-FOUNDATION-001
```

It consists of:

```text
AGENTS.md
SKILL.md
QUALITY_BAR.md

PRD-000
PRD-001
PRD-002
PRD-003
PRD-004
PRD-005
PRD-006

ADR-001
ADR-002
ADR-003
ADR-004
ADR-005
ADR-006
ADR-007
```

No runtime implementation should begin before this seal is coherent.
