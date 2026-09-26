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
runtime diff preferably < 400 LOC
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

ONE ADVERSARIAL REVIEW

↓

FIX VALID FINDINGS

↓

FINAL HEAD VALIDATION

↓

MERGE
```

---

# 29. Current next action

The next authoring sequence is exactly:

```text
1. AGENTS.md
2. SKILL.md
3. QUALITY_BAR.md
4. PRD-000 Product Thesis
5. PRD-001 Campaign Lifecycle
6. PRD-002 Cyber Terrain
7. PRD-003 Access & Footholds
8. PRD-004 Expansion Loop
9. PRD-005 Objective Loop
10. PRD-006 Adaptation
```

Only after these six PRDs are coherent:

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
