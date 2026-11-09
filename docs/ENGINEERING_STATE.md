# DuskWeave Engineering State

## 1. Project Identity & Status

- **Project**: DuskWeave. Verify actual worktree, branch and HEAD before delivery.
- **Current Phase**: M1 is unsealed. Current work is the bounded Linux live-DNS
  SDK slice; PR state, committed head and checks come from GitHub.
- **Active seals**: `DW-FOUNDATION-001` and `DW-M0-001`; evidence below is unchanged.
- Scope/priorities belong to [the roadmap](MVP_AND_DEFERRED_SCOPE.md); dependency
  navigation belongs to [BUILD_ORDER](BUILD_ORDER.md). Neither duplicates PR state.

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
- **Coherence**: Historical F1–F7 reconciliation remains recorded. The accepted 2026-09-29 INV-005 amendment preserves five operational models and ordinary-surface zero exposure while allowing bounded campaign-scoped operational-secret custody. Refreshed cross-foundation terminology and status scans found no new contradiction.
- **Document verification**: The reseal check covered 41 tracked Markdown files and 87 relative links with zero broken links; all documents remain below 400 lines, current-status scans are coherent, and `git show --check` passes for the sealed baseline.
- **Review**: One refreshed adversarial review covered initial access without a foothold, authorized reuse, resumable restart, custody loss, cross-campaign isolation, PostgreSQL/evidence contamination, safety freeze, termination/withdrawal, in-flight uncertainty, and the distinction between disposing DuskWeave-held copies and revoking the client-side credential. No blocking contradiction remains.
- **Scope**: This seal accepts the foundation design at baseline `f93087b52c480822544bad0fb5d99d17eedf8ac0`. No runtime, tool integration, payload implementation, or CI result is claimed.
- **Product-owner action**: The product owner explicitly authorized the `DW-FOUNDATION-001` reseal on 2026-09-29 after accepting PRD-010.

---

## 5a. M0 Seal Evidence (`DW-M0-001`)

- **Scope**: accepts the bounded local M0 lane at verified commit `669f2f36bf2909f3c28f4018a6ae69e76364b286` — M0A registration/history, M0B planning/history, M0C withdrawal/refusal/recovery including C0 fresh-authority confirmation and the C1a/C1b corrections, plus the assurance tooling merged alongside (parser mutation qualification, dependency hardening).
- **Evidence**: owner-accepted `DW-DEMO-M0-TWO-ASSETS` — register → assess → withdraw → refuse → restart → inspect on two operator-reviewed real-asset scopes with live C0 exchanges; bounded Work REVIEW_PASS recorded through the PR #31 reconciliation; merged runtime deliveries #21–#30 each with exact-master CI; parser boundary qualification 33/17/0/16 (PR #32, run 37297574836); dependency gate with real negative controls (PR #33); exact-master run 37339827937 SUCCESS on the sealed commit.
- **Limits**: local loopback core only — no target observation, acquisition, execution, compromise, reachability or ownership proof is claimed; retained demo database `dw_m0_demo_two_assets_20261005` remains evidence, not authorization. The scoped historical RED-evidence gap remains UNVERIFIED-not-PASS; the DSN rotation/reference sync remains IDE attestation. The seal grants no `DW-DOMAIN-001`, no target runtime, no LLM integration and no five-model scaffolding. `DW-FOUNDATION-001` is unchanged.
- **Product-owner action**: the product owner explicitly accepted the Work-reviewed demo evidence and authorized the M0 seal on 2026-10-05.

---

## 6. Current slices and next action

**Next product action:** continue the interrupted r5 final release qualification
on the separated build storage delivered 2026-10-31 — bind/verify the final
release identity, remaining failure/recovery coverage, committed-head
CI/coverage and draft-PR review. Keep the original runtime deadlines and
preserve uncertain attempts, the two historical FIX batches and r4 evidence. The
[operator guide](RUN_M1_AUTONOMOUS_FIXTURE.md#sdk-loopback-qualification-and-fresh-consent)
owns supported procedures.

**Delivered SDK runtime:** the accepted
[ADR-016 section 4b](adr/ADR-016-execution-broker.md#4b-candidate-rust-genai-sdk-transport-amendment-2026-10-28)
and [enabling contract section 8](contracts/M1-enabling-architecture.md#8-candidate-rust-genai-sdk-adapter-amendment-2026-10-28)
were delivered by [#132](https://github.com/carlitotate12160-tech/DuskWeave/pull/132)
(merge `a85afcb`): pinned SDK and bounded per-call evidence patch,
shared semantic preflight, post-reservation `dw-inference-intent-v1` binding,
restricted schema 0022 admission/source recovery, and explicit v5/v3 bindings.
Local Windows fixture tests exercise D1, one DNS fixture acquisition, qualified
D2 feedback and key-free recovery, plus failure/STOP/lease/parent-loss paths.
GitHub owns SDK candidate/PR/check status. These observations are not a seal, paid-provider
qualification, Linux acquisition qualification or global SDK error-safety proof.
Legacy libcurl remains only for its explicitly retained old profiles and M0
consumers; no SDK profile may select it or construct its legacy vendor binding.
`DW-FIX-M1-DNS-SELECTOR-CONTRACT` merged as `e11f62f`
([#120](https://github.com/carlitotate12160-tech/DuskWeave/pull/120)): an
admitted `observe_more` + `dns_udp` proposal carrying contextual references
alongside a single canonical `asset_N` seed now selects and reserves that seed
identically in Rust and restricted SQL, and pack-local `asset_N` ordinals
resolve through entity identity to the correct `seed_names` entry even when
ContextPack omission shifts them. Fresh disclosure, current terms/window,
credential and actual platform qualification are required before provider
contact. Historical consent is unusable.

Relevant merged implementation/evidence pointers, not a rolling merge log:

| Outcome | Durable reference |
| --- | --- |
| Controlled skeleton | [#74](https://github.com/carlitotate12160-tech/DuskWeave/pull/74), [record](workflows/DW-IMPLEMENT-M1-AUTONOMOUS-SKELETON.md) |
| Real-provider wiring and diagnostics | [#107](https://github.com/carlitotate12160-tech/DuskWeave/pull/107), [#109](https://github.com/carlitotate12160-tech/DuskWeave/pull/109), [diagnostics evidence](workflows/DW-FIX-M1-RUN-DIAGNOSTICS.md#12-merge-reconciliation-2026-10-22) |
| Successor lifecycle / Rust fixture supervision | [#116](https://github.com/carlitotate12160-tech/DuskWeave/pull/116), merged `618c885`; [final Windows qualification](workflows/DW-FIX-M1-FIXTURE-SUPERVISOR-RUST.md#final-windows-qualification-2026-10-25) |
| Nonblocking release and transport error precedence | [#117](https://github.com/carlitotate12160-tech/DuskWeave/pull/117), merged `7d7e696`; [record](workflows/DW-FIX-M1-INFERENCE-RELEASE-INTEGRITY.md) |
| Conditional maintenance and qualified skill-doc CI routing | [#118](https://github.com/carlitotate12160-tech/DuskWeave/pull/118), merged `d29a1fb` |
| DNS selector contextual references and entity-based ordinals | [#120](https://github.com/carlitotate12160-tech/DuskWeave/pull/120), merged `e11f62f` |
| Rejected-provider diagnostic metadata with uncertainty-only accounting | [#122](https://github.com/carlitotate12160-tech/DuskWeave/pull/122) |
| Live DNS LAB lane + build-storage guard hardening (draft, candidate `517e92c`) | [#139](https://github.com/carlitotate12160-tech/DuskWeave/pull/139) |

**Unproved:** the 2026-10-22 provider run `4a4ce477-5734-4b92-bb6a-dce74f430114`
committed Decision 1 and fixture Terrain; Decision 2 ended exit 124 with
`may_send_uncertain`. Exact terminating guard and complete cognitive feedback
remain unproved. [Retained evidence](workflows/DW-DESIGN-M1-PROVIDER-NEUTRAL-LIFECYCLE.md)
does not authorize replay or establish a successful successor episode.

**Live acquisition remains unqualified:** selected uncommitted Oracle ARM64 LAB
witnesses demonstrate normal CLI D1, one real isolated UDP exchange, durable safe
result, Terrain/history, D2 and fresh key-free recovery. Separate physical cases
cover limited/negative results, TTL-only stale D2 history, in-flight withdrawal,
delayed authority, private egress probes and result-COMMIT/ACK-loss recovery.
The corrected Broker terminal-handling window also has successful physical
unanswered-query timeout witnesses. These use the deterministic HTTP fixture,
not a local model or a real DeepSeek invocation.
The cumulative McCabe gate passes with 10 retained debts unchanged.
Cold preparation was sensitive to shared storage: build artifacts, Docker
and containerd used the same backing filesystem. Historical pending attempts
localize delay to init-snapshot preparation. Controlled dirty-file experiments
held temporary-layer unmount for 886 ms at 128 MiB and 14212 ms at 768 MiB;
the latter create took 14317 ms, exceeding the unchanged 5000 ms preparation
policy. This reproduced the writeback-coupling mechanism; exact dirty-buffer
counts for the historical failures were not recorded. Pending creates remain
UNKNOWN with no replay/refund or inferred disposal. On 2026-10-31 an
operator-provisioned 50 GB ext4 block volume (iSCSI, `_netdev` fstab) was
mounted at `/srv/duskweave-build` on a device disjoint from `/`,
`/var/lib/docker` and `/var/lib/containerd`. The qualification script now
fails closed on absent/unmounted/coupled/wrong-filesystem/symlink/foreign
content before any compiler or Docker write (all seven refusal cases
exercised; read-only `build-storage` subcommand added) and scopes
`CARGO_TARGET_DIR`, `GOCACHE`, `GOTMPDIR`, `TMPDIR`, Go release and Docker
build context to the new filesystem. One real build installed binaries whose
digests match the new-filesystem artifacts; the old `/srv/duskweave/build`
tree is retained untouched and no longer consumed. Earlier controlled
measurements (167-190 ms clean, 174-187 ms at 128 MiB dirty, 161-179 ms at
768 MiB dirty, 209-350 ms under concurrent write load) covered only Docker
create→start→PID→rm — the OS-level worker lifecycle, not the complete
application READY handshake; full `Pair::prepare` readiness now measures
899-1298 ms on the separated filesystem (3 samples each at the clean
baseline and 128/768 MiB dirty writeback), plus 1047-1157 ms across
3 samples with a verified concurrent `rustc -O` compilation running
(rc=0, ~27 s per 4 MB-binary iteration, alive throughout every
measurement window), still below the unchanged 5000 ms deadline. The storage guard now
also refuses dot-prefixed foreign names and non-conforming fixed
components (symlink, foreign mount, wrong owner/mode/device) before any
build write; 23/23 negative and positive cases refused correctly through
both the `build-storage` and `release` entrypoints. One normal LAB run
completed D1, the isolated DNS exchange, durable result, Terrain/history,
D2 and two fresh key-free recoveries; an immediately preceding attempt
recorded a transient decision-two authority refusal as UNKNOWN with no
replay and passed on a single rerun. A controlled demonstration reproduced
that exact signature — `failed_never_released`, `current_authority_unavailable`,
honest UNKNOWN, committed effect and confirmed disposal — by holding the
campaign advisory fence across the post-`may_send` release window, so the
refusal is fail-closed behavior under a competing owner, not a proved
implementation defect; the historical lock holder remains unidentified and
both attempts' identities, charged state and receipts stay preserved.
Storage separation removes the demonstrated coupling but is
not a universal readiness guarantee. Committed-head CI on the draft PR
(#139, candidate `517e92c`) passes every functional gate — builds, both test
shards, mutation, structure, metrics, CodeQL, geiger, links, dependencies,
Go boundary and the guarded qualification lane — on run `41370937004` /
`41370936961` (mutation). The production coverage floor is FAIL/BLOCKED
(measured coverage below the floor is FAIL/BLOCKED, not merely UNVERIFIED):
run `41377109367` reached total 84.99% (<90% floor) with 21 host-bound files
under the 80% per-file floor. Option A (B2 completion) expands CI-runnable
owner unit tests across launcher admission, attempt journal, control/exchange,
watchdog, docker, network namespace disposal, qualification, relay, effect
session/socket, context live binding, run effect and live withdrawal CLI.
External qualification evidence retains source/version identities and metadata-only traces.
`DW-FIX-PR139-QUALIFIED-LAB` (r2.1/r2.2) physical Oracle ARM64 LAB
qualification (`qualification_ref: 93318766-3139-423d-962b-c91bc588db04`,
`purpose: LAB`, DB `dw_native_pr139_r2_20261101a`, cloned at `4c59ef2`):
4/4 LAB scenarios PASS — normal D1/DNS-exchange/result/history/D2/recovery,
limited/negative, in-flight withdrawal, and result-COMMIT/ACK-loss recovery.
`DW-FIX-PR139-QUALIFIED-LAB` CodeRabbit review: 15 VALID resolved
(13 in U1/U2, 2 in `517e92c`; docker-inspect timeout + broad cleanup handler),
7 FALSE POSITIVE / deferred (CARGO_HOME intentional disk, ACK-wait fail-fast,
launcher-child reap deadlock risk, UUID slot M1-sequential, exporter-child
wait-bounded, deployment.next invariant, root-cause masking).
Distinct review found run-STOP serialization and ordinary launcher-child reaping
defects; focused regressions pass after correction. Remaining loss cases,
compatibility/boundary evidence, final release rebinding and an M1/full-domain
seal remain incomplete. These selected witnesses do not qualify a final
release, M1 seal, provider or public-target run.

**Debt remains open:** use [roadmap sections 1.3-1.5](MVP_AND_DEFERRED_SCOPE.md#13-audit-disposition-attached-to-these-deliveries)
and current candidate metrics. Legacy CLI/M0 behavior remains supported until
tested replacements and compatibility disposition exist. No M1/full-domain seal,
new provider support or blanket Windows full-suite PASS is implied.
