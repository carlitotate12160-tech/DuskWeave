# 16. Stage 14 — Expansion engine

PRD:

```text
PRD-014-expansion-planning.md
```

Implement:

```text
Observe
↓
ExpandAccess proposal
↓
InternalPath
↓
Authorized transition
↓
New Foothold
↓
Observe
```

No hard-coded playbook.

---

# 17. Stage 15 — Chain Composer

PRD:

```text
PRD-015-chain-composition.md
```

ADR:

```text
ADR-020-chain-validation.md
```

Architecture:

```text
Reasoning Worker
      ↓
ChainCandidate
      ↓
Rust Chain Validator
      ↓
ValidatedChain
```

Composer cannot execute.

---

# 18. Stage 16 — Adaptation engine

Input:

```text
terrain delta
foothold changes
objective changes
evidence freshness
capability availability
```

Output:

```text
ReplanProposal
```

No execution authority.

---
