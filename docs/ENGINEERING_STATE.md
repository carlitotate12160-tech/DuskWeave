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

**Next product action:** distinct delivery review of the locally verified
DW-AUD-007 catalog-resolution correction on `fix/aud007-catalog-resolution-r1`.
The held profile packet's dependency disposition stays with the audit
workstream; this record neither merges the candidate nor releases that hold.
Acquisition, Terrain and D2 remain unproved. Preserve historical evidence, the
frozen deployment and finite binding; do not restart image restoration, prior
qualification/coverage, or paid/target runs. The
[operator guide](RUN_M1_AUTONOMOUS_FIXTURE.md#linux-lab-build-storage-owned-oracle-arm64)
owns deployment procedures. Real-provider/live-target consent remains separate.

**DW-AUD-007 catalog resolution (DW-AUD007-CATALOG-RESOLUTION-R1) — locally
reproduced and corrected, awaiting committed-head CI and distinct review:**
the audit's catalog-shadow hypothesis was verified dynamically, not assumed.
Twenty-four SECURITY DEFINER functions plus the invoker
`terrain.admit_dns_request` resolved unqualified catalog relations through
`pg_temp` first under `search_path=pg_catalog`; a TEMP-eligible restricted
caller could shadow `pg_roles` and cross the original-writer and
role-qualification checks. On a fresh schema-0031 fixture the three new
worker cases reproduced the defect (foreign run-current/record read, foreign
inference settlement and live stop, denied-principal qualification) while
every control and warmed session refused. Migration `0032_catalog_resolution`
sets `search_path = pg_catalog, pg_temp` on exactly those 25 functions in one
transaction; adapter-local qualification SQL gained `pg_catalog.` on catalog
relations only. On a fresh schema-0032 fixture the full 14-case group passed,
and a populated schema-0031 fixture refreshed through the maintained
`migrate()` path to 0032 with unchanged function OIDs, bodies, ACLs and
evidence digests; that refresh replay for migration 0024's dynamic trigger is
now idempotent. Driver, verdicts and attempt archives live at
`D:/DuskWeave-build/rust-1.94.1/reports/aud007-catalog-resolution-r1`.
Limitations: existing deployed databases stay unverified until they receive
0032; the correction is not merged, deployed or sealed; slot 0032 consumed by
this fix moves the planned profile migration to 0033.

**Proposal-admission diagnostics — fixture correction measured, awaiting CI and distinct review:**
structural failures retain only closed reason/kind/known-field metadata; arbitrary
member names, values, response bodies and private reasoning remain excluded.
Generation and repair keep the original repair material and one-repair bound.
Optional `proposal_diagnostic` metadata is settled on each original call and flows
through existing provenance/receipt/inspect/recover. Migration 0031 supplies the
validated diagnostic-completion entrypoint; old rows are not amended. Native setup
restores that migration after its 0012/0013 replay. Diagnostic completion accepts
requested model `deepseek-flash` or `protocol-fixture-v1`; JSON null, non-strings,
unknown strings and `probe-fixture` still refuse before a terminal rewrite.
`probe-fixture` remains reported-model metadata. The provider-response branch is
unchanged. Historical Oracle evidence stays at
`/srv/duskweave/reports/proposal-diagnostics-r1/`. On fresh base-0021 fixture
`dw_native_pr147fix2_20261111r1`, lifecycle T6, Terrain repair and SDK
repair/exhaust each ran once and passed. T6's generation call completed with
`protocol-fixture-v1` and closed `proposal_admission_diagnostic_v1` metadata, then
opened its one eligible repair. The schema-0031 SQL group passed, including the
new requested-model refusals. Python helper compatibility, formatting, locked
all-target Clippy, links, diff hygiene, LOC and McCabe passed against base
`6066a771`. That head's Rust shards passed. Its coverage gate failed only on the
declarations-only file named below.

**Coverage classifier — public named fields measured locally, awaiting new-head CI:**
run 43005834065 reported 95.59% (17221/18016) and failed because
`src/m1_reasoning/inference/provider_evidence.rs` has no llvm-cov record. That
source is data declarations, attributes and public named fields. The gate already
exempts declarations-only files; its private field/variant recognizer missed
visibility-prefixed fields. A separate recognizer accepts `pub`, `pub(crate)`,
`pub(self)` and `pub(super)` named fields. Executable and unknown constructs,
including `pub(in ...)`, stay fail-closed. Floors stay 90% total and 80% per
file. The new positive case failed on the previous recognizer (22/23); the
selftest then passed 23/23. The unchanged source classifies as declarations-only.
The unmodified JSON from that run, read locally after an in-memory Linux
workspace-prefix adjustment required on Windows, reports the file as not
applicable and the same total with no gate problems. That reading is old-head
provenance. New-head CI, including the coverage gate and ci-ok, and distinct
review remain open. The change is not deployed, accepted or sealed.

**Separate model spike (not release acceptance):** retained local report
`D:/DuskWeave-build/spike-v4pro-report-complete/` records two completed
`deepseek-v4-pro` calls, validated D1/D2, committed fixture acquisition/Terrain
and exit 0. The unsuffixed report now records a different incomplete Flash
episode; the spike worktree has since changed. These artifacts support a Pro
profile candidate, not a controlled host-matched causal conclusion or changed
production pin. A separate contract/profile amendment and qualification remain
necessary; no provider or real-target execution is added by this diagnostic fix.

**Schema-0030 successor pack compatibility (DW-FIX-SCHEMA0030-SUCCESSOR-PACK-R1)
— merged in [PR #146](https://github.com/carlitotate12160-tech/DuskWeave/pull/146):** the installed
`execution.assert_successor_pack` unconditionally required
`live_binding`/`qualification_purpose` while rejecting both on non-v4 requests,
so no retained v2/v3 pack could pass the restricted marker consumer. The
correction admits the two live-only keys only for
`m1_autonomous_run_request_v4` before strict key validation; all identity,
scope, owner, premise, freshness and authority checks are unchanged, as is the
real Rust producer. Two new dedicated-worker cases prove the retained/v2/v3/v4
matrix with per-field refusal controls and a real-CLI SDK v3 loopback run
(controlled feedback, Terrain/history, key-free inspect/recover with
before/after immutable-identity comparison and explicit D2
predecessor/Terrain linkage) on the retained 0030 function; shared lifecycle
support lives in the target-private `run_support` module consumed by the
receipt, pack and episode cases. Oracle ARM64 offline evidence on the
consolidated harness executable SHA-256
`37f7fc9cd0b19aa1601b9c4309bb19331bcf22617df36f30b64e7085fadc97b9`: RED on the
byte-verified unpatched source (`dw_native_fix0030_red2_20261110r1`, 8 pass/2
fail — exactly the two new cases, zero provider sends) and GREEN on the
corrected candidate (`dw_native_fix0030_green3_20261110r1`, 10/10 pass);
earlier pre-consolidation runs (`dw_native_fix0030_red_20261110r1`,
`dw_native_fix0030_green_20261110r1`/`dw_native_fix0030_green2_20261110r1`)
showed the same signature and remain preserved; reports at
`/srv/duskweave/reports/fix-schema0030-successor-20261110r1`. Draft PR
[#146](https://github.com/carlitotate12160-tech/DuskWeave/pull/146) head
`9bf1f70` passed all committed-head required checks (one stuck
`structure (ubuntu-latest)` posting was recovered by rerun; the job itself
had succeeded). The single authorized bounded DeepSeek episode then ran
once on the frozen release CLI (`e4aad5ca…`, image-verified) against fresh
provider fixture `dw_native_fix0030_provider_20261110r1` with all-new
identities and one 300-second window: `register` accepted
(history=completed); `m1-run` started, claimed decision_one, reserved and
bound the generation call, and the v3 successor pack was admitted past
`inference_may_send` on the corrected schema — the surface this fix owns.
The send was then refused after the may_send marker and before the
provider call with the mapped `current_authority_unavailable`; which of
the fence/current-authority/basis branches fired is not identified from
retained evidence (postgres error text is not captured on that host). The
call is durable as `failed_never_released`, the decision recorded
disposition `current_authority_unavailable`, and the run closed `unknown`
with a committed diagnostic. Zero provider sends occurred (the ≤4-call
bound was never consumed), zero effects were recorded, and key-free
`m1-run-inspect`/`m1-run-recover` afterwards left decision/call/run/
effect/terrain identities byte-identical. That observability gap is
reported as provider feedback, not attributed to this fix. **LIVE
readiness: not verified or delivered by this packet** —
`boundary/qualify/qualify.sh lab-bind` is purpose-locked to
`LAB`/`autonomous-dns-genai-lab-v1`; whether a supported procedure can
mint the `purpose=LIVE`/`autonomous-dns-genai-live-v1` binding a v6 live
run requires was not verified, and no such procedure or binding is
delivered here; no row was fabricated. Distinct review of the fix, the
provider outcome and the LIVE-readiness question is pending; niagamas.com
discovery remains an unproven prerequisite, not delivered by this fix.

**Final-release LAB qualification (R1) — FAIL / NOT READY:** owner-authorized
backup, fresh schema-0030 database/config provisioning, frozen-bundle installation
and finite LAB binding completed on 2026-11-10. Release source remains
`8c6f752`; driver `d78d204` has the same Git tree as the integrated driver
([#144](https://github.com/carlitotate12160-tech/DuskWeave/pull/144)). No runtime,
SQL, bundle or image input was rebuilt or changed. Driver run
[42751002099](https://github.com/carlitotate12160-tech/DuskWeave/actions/runs/42751002099)
passed required checks and the 90/80 coverage gate (95.56%, 17039/17830).

All 17 physical cases executed serially on the installed frozen release:
16 PASS, 1 FAIL, no skipped/zero-test successes. The mid-D1 case failed at
`tests/m1_live_dns/withdrawal/pre_acquisition.rs:196`: it expects top-level
`operation`, while the existing durable producer owns
`contract.operation_id`. Independent read-only review classified this as a
VALID test defect, not a demonstrated runtime defect. Earlier held-D1/parent
STOP assertions executed; subsequent scope/receipt-STOP/zero-result assertions
did not. Supplemental database/resolver witnesses show durable withdrawal and
zero DNS/results but do not turn the failed case into PASS.

Evidence is retained at `/srv/duskweave/reports/final-release-qual-20261110`
(`qualification-summary.json`, per-case logs/results and verified rollback).
Fresh database `dw_native_final_release_qual_20261110` (OID 39077), restricted
roles, protected config and LAB row `af0a0d66-9f11-4713-857d-4dc8e3d2c863`
remain preserved. Resource/schema checks passed after every case, with the
normal case's supplemental metadata-checker correction retained. After all
owned resources were reconciled, the failure rollback restored exact prior
binary/manifest bytes and ownership/modes. That prior qualification is expired:
the host is **UNQUALIFIED**. The frozen bundle/image and historical resources
remain intact. No real-provider/public-target qualification or M1 seal is claimed.

A later deployment installed the same unchanged frozen release against the
fresh b fixture `dw_native_final_release_qual_20261110b` (OID 40150, role
prefix `dw_frq10b`), protected config
`/etc/duskweave/m1/lab-final-release-20261110b`, report root
`/srv/duskweave/reports/final-release-qual-20261110b` and LAB row
`4629211c-4fdc-48bc-b8fe-c4c1124a2a9d` (manifest SHA-256
`80667eac1ba0b63f40d0e33e78d361cd93891aea7c3285a81648a2debc8c8835`) under the
original finite interval ending 2026-11-11 13:01:10.035 WIB — never extended.
Installed hashes equal the frozen bundle; the b database held zero
autonomous runs until the Phase B rerun, which added exactly one for the
corrected case, while the original 17-case matrix ran only on the first
fixture. This binding is administrative deployment evidence, not an accepted
qualification.

**Mid-D1 correction (DW-FINAL-RELEASE-QUAL-R1-FIX2) — corrected case passed on
b, awaiting external review:** the case consumes the existing durable contract
(`contract.operation_id`) and persists the parsed withdrawal receipt as
`mid-d1-withdrawal.json` in its case report directory immediately after
parsing. Every other scenario assertion, deadline, witness and counter is
unchanged. On 2026-11-10 the single corrected case executed once on the
verified b binding via harness executable SHA-256
`c4742cf3f85b71560a9f1d2e7675f44e9a735e2dc15f4b955f61aaf8d87b8174` (built from
the byte-verified `757b0c36` export): exit 0, exactly one test passed in 1.03s,
no retry. The durable receipt carries `contract.operation_id`,
`mission_authority_withdrawn`, `stop_latched` plus `disposal_confirmed`, scope
limited to the run's engagement/campaign, and zero DNS counters while the
fixture reply stayed held; `execution.autonomous_runs`, `mission.withdrawals`
and `trajectory.withdrawal_history` each gained exactly one row and the
live/fixture DNS tables stayed empty. Post-run reconciliation left the
manifest, installed binaries, schema, functions, grants and qualification row
unchanged, with no owned namespace/route/rule/listener residue; evidence is
retained at `/srv/duskweave/reports/final-release-qual-20261110b/fix2-phase-b`
and `cases-mid-d1-fix2`. Sixteen retained first-fixture PASS plus this
corrected PASS on b form the corpus for external delta review — not a fresh
17/17 run on b and not an acceptance.

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
