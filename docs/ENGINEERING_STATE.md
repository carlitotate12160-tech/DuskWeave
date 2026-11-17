# DuskWeave Engineering State

Current-state index: active work, next action, milestone/seal status and the
evidence pointers needed to interpret authority. PR state, candidate SHA and CI
results belong to GitHub; scope and priorities belong to
[the roadmap](MVP_AND_DEFERRED_SCOPE.md); dependency navigation belongs to
[BUILD_ORDER](BUILD_ORDER.md). This file is not a merge log or episode
chronicle. The retired episode chronology remains readable at
[the immutable pre-compaction record](https://github.com/carlitotate12160-tech/DuskWeave/blob/9d6cd89d8349c57dfca67e0a246daf1e5125f339/docs/ENGINEERING_STATE.md);
entries below keep only outcome, tested-source identity, limits and owners.

## 1. Project Identity & Status

- **Project**: DuskWeave. Verify actual worktree, branch and HEAD before delivery.
- **Current phase**: M1 is unsealed. The active assigned outcome is the Linux
  platform-direction and state-index documentation delivery; its PR and check
  status lives on GitHub.
- **Active seals**: `DW-FOUNDATION-001` and `DW-M0-001`; evidence in section 5
  is unchanged.
- **Platform direction**: Linux is the MVP core/Broker/acquisition runtime under
  accepted [ADR-015](adr/ADR-015-execution-boundary.md). Windows remains an
  operator/developer workstation with WSL as the preferred development
  direction. Terms and retained compatibility live in
  [the roadmap](MVP_AND_DEFERRED_SCOPE.md#16-mvp-platform-direction-owner-direction-2026-11-17).

## 2. Stage 0 Artifact Checklist & Verification

Bootstrap verification is historical; the full checklist remains in
[the pre-retirement record](https://github.com/carlitotate12160-tech/DuskWeave/blob/d29a1fbb0cb6bdab18e705a0e085e774c0d7a325/docs/ENGINEERING_STATE.md).
Current contributor protocol and quality rules live in [AGENTS](../AGENTS.md)
and [QUALITY_BAR](../QUALITY_BAR.md).

## 3. Exit Criteria Evaluation for `DW-BOOTSTRAP-001`

Completed bootstrap evidence remains in the historical record above; it is not
a new acceptance or a claim that every current artifact was checked then.

## 4. Active Invariants Enforced

Use [PRD-000](prd/PRD-000-product-thesis.md) and
[the invariant index](build-order/00-authority-and-invariants.md).

## 5. Foundation Seal Evidence

- **Authority completeness**: PRD-000..006 and ADR-001..007 are `ACCEPTED`.
- **Coherence**: Historical F1–F7 reconciliation remains recorded. The accepted
  2026-09-29 INV-005 amendment preserves five operational models and
  ordinary-surface zero exposure while allowing bounded campaign-scoped
  operational-secret custody. Refreshed cross-foundation terminology and status
  scans found no new contradiction.
- **Document verification**: The reseal check covered 41 tracked Markdown files
  and 87 relative links with zero broken links; all documents remained below
  400 lines and `git show --check` passes for the sealed baseline.
- **Review**: One refreshed adversarial review covered initial access without a
  foothold, authorized reuse, resumable restart, custody loss, cross-campaign
  isolation, PostgreSQL/evidence contamination, safety freeze,
  termination/withdrawal, in-flight uncertainty, and the distinction between
  disposing DuskWeave-held copies and revoking the client-side credential. No
  blocking contradiction remains.
- **Scope**: This seal accepts the foundation design at baseline
  `f93087b52c480822544bad0fb5d99d17eedf8ac0`. No runtime, tool integration,
  payload implementation, or CI result is claimed.
- **Product-owner action**: The product owner explicitly authorized the
  `DW-FOUNDATION-001` reseal on 2026-09-29 after accepting PRD-010.

Accepted amendment exact-text records stay in their owning documents: the
[roadmap §1.2 entry](MVP_AND_DEFERRED_SCOPE.md#12-autonomous-campaign-delivery-order-owner-direction-2026-10-19)
records the 2026-10-19 autonomous-run amendment acceptance at exact text
`0615fb72c946fd6abf0823692be621096091377c`; the INV-005 foundation amendment is
recorded above; scoped R1/R2/R3 acceptances are recorded in the ADR-015/016 and
M1-contract revision rows. An amendment status not recorded in an owning
document or owner direction is proposed, not accepted.

## 5a. M0 Seal Evidence (`DW-M0-001`)

- **Scope**: accepts the bounded local M0 lane at verified commit
  `669f2f36bf2909f3c28f4018a6ae69e76364b286` — M0A registration/history, M0B
  planning/history, M0C withdrawal/refusal/recovery including C0
  fresh-authority confirmation and the C1a/C1b corrections, plus the assurance
  tooling merged alongside.
- **Evidence**: owner-accepted `DW-DEMO-M0-TWO-ASSETS` — register → assess →
  withdraw → refuse → restart → inspect on two operator-reviewed real-asset
  scopes with live C0 exchanges; bounded Work REVIEW_PASS recorded through the
  PR #31 reconciliation; merged runtime deliveries #21–#30 each with
  exact-master CI; parser boundary qualification 33/17/0/16 (PR #32, run
  37297574836); dependency gate with real negative controls (PR #33);
  exact-master run 37339827937 SUCCESS on the sealed commit.
- **Limits**: local loopback core only — no target observation, acquisition,
  execution, compromise, reachability or ownership proof is claimed; retained
  demo database `dw_m0_demo_two_assets_20261005` remains evidence, not
  authorization. The scoped historical RED-evidence gap remains
  UNVERIFIED-not-PASS; the DSN rotation/reference sync remains IDE attestation.
  The seal grants no `DW-DOMAIN-001`, no target runtime, no LLM integration and
  no five-model scaffolding. `DW-FOUNDATION-001` is unchanged.
- **Product-owner action**: the product owner explicitly accepted the
  Work-reviewed demo evidence and authorized the M0 seal on 2026-10-05.

## 6. Current slices and next action

**Active work (unmerged):** `DW-MVP-LINUX-STATE-INDEX-R1` — records the
owner-requested Linux MVP platform direction and restores this file as a
concise current-state index. This is a support-direction decision: no seal,
deployment qualification, Windows runtime retirement or new execution
permission follows from it.

**Latest completed product evidence (bounded historical observation):**
On 2026-11-16 the explicitly authorized Oracle ARM64 episode
`live-episode-niagamas-20261116r1` on tested source `7ea3b90` (delivered through
PR #155) completed D1 → real DNS → Terrain → D2 in 73.558 seconds, exit `0`:
two provider calls, two decisions/history entries, one effect and one Terrain
history. Resolver `1.1.1.1` returned `206.189.93.100` for `niagamas.com`
(rcode `0`, TTL `1799`); D2 retained the hypothesis on `terrain_0` with
predecessor feedback. This is a routing observation — not HTTPS proof, an M1
seal, permanent qualification or reusable provider/target authorization; its
binding windows were finite. Deployment detail, custody/inspect evidence and
the report root are in
[the immutable record](https://github.com/carlitotate12160-tech/DuskWeave/blob/9d6cd89d8349c57dfca67e0a246daf1e5125f339/docs/ENGINEERING_STATE.md).

**Also delivered:** the bounded Rust-genai SDK transport lane (accepted
[ADR-016 §4b](adr/ADR-016-execution-broker.md#4b-candidate-rust-genai-sdk-transport-amendment-2026-10-28)
and enabling contract §8, merge `a85afcb` via #132) and the DNS selector
contextual-reference/entity-ordinal fix (#120, `e11f62f`), qualified on Windows
fixtures. These are not a seal, paid-provider qualification or live
qualification; legacy libcurl remains only for its explicitly retained old
profiles and M0 consumers.

**Next product action:** bounded HTTPS-validation DESIGN through the actual
reasoning → acquisition → Terrain → adaptation loop in the existing roadmap
order, with cost bounded before code. Fresh disclosure/authorization, accepted
profiles and qualified live boundaries remain prerequisites; fixture or expired
consent cannot satisfy them.

## 7. Unresolved constraints

Current open items with their owning references. Historical entries marked
FAIL/NOT READY/UNQUALIFIED describe those episodes, not present verdicts.

| Item | Status | Owning reference |
| --- | --- | --- |
| Native Windows M1 control-root/ACL qualification | OPEN — deferred for MVP; retained Windows code, tests and dev tooling unchanged, not relabeled PASS | [operator guide](RUN_M1_AUTONOMOUS_FIXTURE.md); retained evidence `D:/DuskWeave-build/rust-1.94.1/reports/deepseek-pro-profile-r4` |
| TTL-bounded premise and repair under a stale premise | OPEN | [enabling contract](contracts/M1-enabling-architecture.md) premise/freshness rules; [historical qualification ledger](M1_TERRAIN_INFORMED_DNS_QUALIFICATION.md) |
| HTTPS admission prerequisites | OPEN — fresh authorization, accepted profile and qualified live boundary required before execution | [roadmap §1.2](MVP_AND_DEFERRED_SCOPE.md#12-autonomous-campaign-delivery-order-owner-direction-2026-10-19) |
| Original R2 live-DNS LAB harness | NOT READY — r2.1/r2.2 LAB witnesses passed 4/4 scenarios, but the coverage floor was FAIL/BLOCKED (84.99%) and PR #139 closed unmerged | PR #139 delivery notes |
| Parked Rust operator release-descriptor port | PARKED — intentionally outside delivered scope | PR #155 delivery notes; branch `implement/operator-release-cli-r1` |
| GitGuardian dashboard disposition, incidents 40144272/40144273 | UNVERIFIED — repository literals verified as synthetic test sentinels; dashboard state not checked | PR #155 delivery notes |
| Audit debt: named McCabe debts, host-bound coverage, CI performance | OPEN | [roadmap §1.3–1.5](MVP_AND_DEFERRED_SCOPE.md#13-audit-disposition-attached-to-these-deliveries) |

## 8. Historical episode index

Outcome, tested-source identity and carried limits only; full detail lives in
the owning PRs and
[the immutable record](https://github.com/carlitotate12160-tech/DuskWeave/blob/9d6cd89d8349c57dfca67e0a246daf1e5125f339/docs/ENGINEERING_STATE.md).

| Episode | Outcome / status | Limits carried forward |
| --- | --- | --- |
| Run lease coordination fix (PR #151) | QUALIFIED (Linux): heartbeat-fence mechanism confirmed, then resolved by migration `0034`; 21/21 SQL cases on fresh schema `0034` and on `0033→0034`/`0032→0034` upgrades | Offline/Linux scope; no Windows ACL change; no deployment qualification |
| AUD-007 catalog resolution (PR #150) | Merged: 24 SECURITY DEFINER functions plus `terrain.admit_dns_request` resolved catalog relations through `pg_temp` first; migration `0032` pins `search_path`; fresh-0032 group passed 14/14 and populated-0031 refresh preserved bodies/ACLs/digests | Deployed databases stay unverified until they receive `0032`; no seal |
| Proposal-admission diagnostics + coverage classifier (PR #147) | Merged: closed diagnostic metadata settles on the original call; declarations-only recognizer accepts visibility-prefixed fields; new-head CI coverage gate passed (95.59%) | No deployment qualification; floors stay 90%/80% |
| Schema-0030 successor-pack compatibility (PR #146) | Merged: v2/v3/v4 admission matrix corrected (RED 8/10 → GREEN 10/10, harness `37f7fc9c…`); the bounded provider episode passed admission then refused `current_authority_unavailable` (unattributed) | LIVE readiness not verified by that packet; provider refusal unattributed |
| Final-release LAB qualification R1 + FIX2 (PRs #144/#145) | R1: 16/17 PASS, 1 FAIL classified as a VALID test defect; FIX2's corrected case passed once on fixture b — a 16+1 corpus for delta review, not a fresh 17/17 | Host UNQUALIFIED after finite window expiry; no M1 seal |
| Closed Pro candidate R4-FIX1 + D2 release diagnosis | Linux fresh-0033 group passed 17/17; populated-0032 attempt passed 13/14 with a second silent `current_authority_unavailable`; the heartbeat-fence mechanism was later confirmed and fixed (PR #151) | The diagnosis made no production change; Linux results do not qualify Windows |
| Live-DNS LAB lane r2.1/r2.2 (PR #139, closed) | 4/4 LAB scenarios PASS on Oracle ARM64 at `4c59ef2`, qualification `93318766-3139-423d-962b-c91bc588db04` | Witnesses do not qualify a release, M1 seal, provider or public-target run |
| Controlled skeleton → provider wiring → live-DNS direction | Delivered via #74, #107/#109, #116, #117, #118, #120, #122 and #132; merge IDs and per-episode detail are in the immutable record | Fixture/loopback scope only; no live or seal claim |

**Debt remains open:** use
[roadmap sections 1.3-1.5](MVP_AND_DEFERRED_SCOPE.md#13-audit-disposition-attached-to-these-deliveries)
and current candidate metrics. Legacy CLI/M0 behavior remains supported until
tested replacements and compatibility disposition exist. No M1/full-domain
seal, new provider support or blanket Windows full-suite PASS is implied.
