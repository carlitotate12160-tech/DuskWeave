# DW-IMPLEMENT-M1-INFERENCE-LIFECYCLE — R2

PROJECT: DuskWeave\
MODE: IMPLEMENT\
WORKSPACE: D:/DuskWeave-ci-complexity\
BRANCH: feat/m1-inference-lifecycle\
BASE: 79b0e3ee1bea44b5e5639c1142af45b6c44918b4\
TRACKING: UPDATE existing state/navigation/roadmap and this packet.\
Prepared: 2026-10-23. R1 replaced the earlier packet in full. R2 applies the
owner-issued resume instruction: qualification environment assignment and
retained-WIP/session preconditions only; R1 implementation decisions, exact
file map, T1–T9, quality floors and independent review remain binding.

Execute directly with the repository build-duskweave skill. Deliver one cohesive runtime PR. Do not return another architecture plan.

## 1. One observable outcome

Deliver a versioned successor of the existing CLI loop:

**Mission -> Decision 1 -> Broker-admitted synthetic DNS -> Terrain owner publication -> new feedback ContextPack -> Decision 2 -> durable history/inspect**, without intermediate operator input.

Exercise the production m1-run/SQL-owner path with two explicitly local wire protocols, Responses and Chat Completions. Keep DeepSeek as the first external adapter, qualified offline at its concrete transport boundary. A fixture proves orchestration and protocol interchangeability, not real cognition, provider availability or target facts.

This supplies reliable reasoning/feedback for objective-led authorized adversary emulation. APT41, Lazarus and Volt Typhoon remain behavioral lenses for hypotheses, active validation, evidence-led adaptation and restrained footprint, not playbooks or permissions. Preserve genuine retain/defer/stop/view/capability-gap choices; do not force every model proposal into DNS.

Authority: PRD-000 INV-001..007; applicable accepted ADR-002/004/005/006/011/015/016; M1 enabling architecture section 7 and ADR-016 section 4a's owner-ACCEPTED 2026-10-22 lifecycle/freshness revision; external-orientation contract; existing roadmap sections 1.2–1.4; AGENTS, QUALITY_BAR and build-order 00/07. Foundation/M0 are sealed; M1 remains unsealed.

**Authorization:** implementation and isolated local qualification only. No provider request/health check, target/resolver contact, inherited consent, real demo or automatic merge. Finish offline work without a provider key. A real episode follows qualified delivery under fresh disclosure/terms/window/key requirements.

## 2. Verified preflight and resources

At preparation master and the clean selected worktree equal BASE; the delivery branch does not exist. PR #115 is merged. Master Actions run 39895781323 passed all required jobs; rust was skipped for the documentation-only change. This is baseline evidence, not this candidate's runtime qualification. Merged #110/#112/#113 are retained; #111 closed without merge, so sha2 remains 0.11.0.

Recheck origin, HEAD, status, worktrees and relevant authority deltas. Continue
the existing named branch at retained BASE with its uncommitted WIP and evidence;
do not recreate the branch, reset or discard untracked files. Preserve unrelated
worktrees and user changes. A newer relevant baseline needs reconciliation,
not a reset.

- Windows x86_64 MSVC Rust 1.94.1. Ordinary target: D:/DuskWeave-build/rust-1.94.1/normal. Coverage target: D:/DuskWeave-build/rust-1.94.1/coverage/inference-lifecycle. Reports: D:/DuskWeave-build/rust-1.94.1/reports/inference-lifecycle. One owner holds each target for the verification sequence; process-scoped environment only, no concurrent builds/cleaning or routine cargo clean.
- Reuse owned dwpg-r3: inspected running, PostgreSQL 17.11, loopback 127.0.0.1:5432, image postgres@sha256:d74eeac9a635390a49bc21bd49fccd973de707e2a53a76ac49b552b8712ec46f. Requalify it; do not recreate it.
- Retain the valid bootstrap session for dw_native_lifecycle_20261023a. If it is
  unavailable, prove dw_native_lifecycle_20261023b absent before creating that
  fresh disposable fixture with the same authorized bootstrap; preserve a and
  its evidence and never recover its credentials. Use generated distinct
  restricted runtime logins; admin is provisioning/fault injection only.
  Serialize cluster-role fixtures. Missing authorized restricted DSNs blocks
  the affected gate, not permission to use admin at runtime.
- Keep DEEPSEEK_API_KEY stripped from offline test processes. Never read/copy/edit/stage .cargo/config.toml, inspect container environment, print .env/DSNs/keys, reset credentials, remove retained DBs or clear STOP markers.
- Record the Rust-linked libcurl/TLS/resolver backend. Reuse existing OpenSSL/Python/toolchain paths; no installation or host-policy changes.

Historical run 4a4ce477-5734-4b92-bb6a-dce74f430114 established the first decision, fixture DNS/Terrain and linked second input. Decision 2 ended exit124/may_send_uncertain. Its terminating guard remains unproved; preserve it as incomplete evidence. Do not replay it or repeat the completed PR #109 diagnostic exercise as a separate work item.

## 3. Exact editable map

Paths are relative to WORKSPACE; the map is an allowance, not a requirement to edit every file. Each change must serve this temporal contract or its real consumer. No wildcard edits.

| Boundary | Allowed paths |
| --- | --- |
| Packet/tracking/guide | docs/workflows/DW-IMPLEMENT-M1-INFERENCE-LIFECYCLE.md (new), docs/ENGINEERING_STATE.md, docs/BUILD_ORDER.md, docs/MVP_AND_DEFERRED_SCOPE.md, docs/RUN_M1_AUTONOMOUS_FIXTURE.md |
| Canonical profile and implementation reconciliation | docs/adr/ADR-016-execution-broker.md, docs/contracts/M1-enabling-architecture.md |
| Additive schema | migrations/0021_inference_lifecycle.sql (new) |
| Profile/permission/capacity | src/lib.rs, src/m1_inference_profile.rs (new), src/m1_inference_permission.rs, src/m1_inference_budget.rs |
| Composition, clocks and control | src/main.rs, src/database_wait.rs, src/database_config.rs, src/m1_run.rs, src/m1_run_cli.rs, src/m1_run_supervision.rs, src/m1_run_control.rs, src/m1_run_inference.rs, src/m1_run_effect.rs, src/m1_inference_control.rs, src/m1_reasoning_cli.rs, src/postgres_m1_run.rs |
| Pack and Terrain identity/use | src/m1_autonomous_context.rs, src/m1_terrain_view.rs, src/m1_terrain_issued.rs |
| Reasoning/protocol/publication | src/m1_reasoning/mod.rs, src/m1_reasoning/inference.rs, src/m1_reasoning/decision_attempt.rs, src/m1_reasoning/proposal.rs, src/m1_reasoning/postgres.rs, src/m1_reasoning/postgres_terrain.rs, src/m1_reasoning/request.rs, src/m1_reasoning/request_autonomous.rs, src/m1_reasoning/response.rs, src/m1_reasoning/transport.rs, src/m1_reasoning/transport_control.rs, src/m1_reasoning/fixture.rs, src/m1_reasoning/chat_completions.rs (new), src/postgres_reasoning_history.rs |
| Contract/scenario tests | tests/m1_inference_lifecycle.rs (new), tests/m1_autonomous_permission.rs, tests/m1_autonomous_wire.rs, tests/m1_run.rs, tests/m1_run_cli.rs, tests/m1_run_postgres.rs, tests/m1_run_failures.rs, tests/m1_run_diagnostics.rs, tests/m1_provider_fixture.rs |
| Private boundary fixtures/setup | tests/support/m1_run.rs, tests/support/m1_native_transport.rs, tests/support/m1_autonomous_transport.rs, tests/support/m1_inference_lifecycle.rs (new), tests/support/m1_native_bootstrap.py, tests/test_m1_native_helpers.py, tests/faults/m1_command_deadline.rs |
| Generated successor wire examples | docs/contracts/fixtures/m1-inference-lifecycle-wire-v2.json (new) |
| Owner-authorized advisory correction (2026-10-23) | Cargo.lock: ctutils 0.4.2 to 0.4.5 and its registry checksum only, for RUSTSEC-2026-0385 |

Read-only: other authority/skills, migrations 0001–0020, legacy wire examples, src/m1_context.rs, src/m1_context_capacity.rs, src/postgres_terrain_view.rs, src/postgres_mission_basis.rs, other existing native/upgrade/continuation fixtures, Cargo/dependency files except the explicit ctutils lockfile correction above, and CI/quality tooling. Inspect unexpected required paths and report the exact need before widening the map.

UPDATE tracking: reconcile #115 and dependency outcomes, name this candidate/next qualification honestly, retain the failed episode and seals. A candidate never records its own future merge or commit SHA. ADR-016 owns the new numerical table; enabling architecture links to it and reconciles implemented/pending statements. Preserve accepted history/anchors. No document split, duplicate roadmap, generic manager, SDK/service migration, dependency change beyond the owner-authorized ctutils correction, web UI or unrelated refactor.

## 4. Implement these decisions

### A. Explicit provider selection; prove two protocols through the CLI

Keep all legacy permission/run/pack semantics and golden bytes. Introduce:

- permission m1_inference_permission_v4;
- run request m1_autonomous_run_request_v2;
- pack m1_context_pack_autonomous_v2;
- one provider-independent run_profile autonomous-dns-fixture-v2 / controlled_fixture;
- inference_profile in the new run request and pack, exactly bound to registered permission.profile.

Install these three closed profile values. The two local profiles are explicit qualification doubles, not fallback destinations:

| Profile | Recipient/protocol/model |
| --- | --- |
| deepseek-responses-reasoning-v1 | https://api.deepseek.com/responses; existing Responses codec; deepseek-flash, effort high, max_output_tokens 4096, stream false, no tools; retain temperature/top_p omissions |
| fixture-responses-v1 | urn:duskweave:fixture:responses:v1; bounded Responses envelope; synthetic protocol-fixture-v1 |
| fixture-chat-completions-v1 | urn:duskweave:fixture:chat-completions:v1; bounded Chat Completions envelope; synthetic protocol-fixture-v1 |

Keep connection/credential reference, protocol codec, model settings and routing policy distinct immutable values in src/m1_inference_profile.rs. It owns profile lookup and feasibility, not campaign state or orchestration. The selected profile remains fixed for both decisions and repair. An invalid explicit profile/settings/credential refuses before release; no ambient-key search, default provider substitution or silent field dropping.

DeepSeek's connection maps explicitly to DEEPSEEK_API_KEY, passed only to selected inference children. Fixture/acquisition/recovery children need no key. No secret is serialized into permission, pack, configuration, body, control or history.

Both local adapters must consume their real bound request bytes, derive the proposal from that supplied pack, produce their distinct bounded raw response envelopes and pass the normal codec/screening path. Reuse safe fixture proposal generation, not Accepted shortcuts or prewritten receipts. Responses and Chat Completions must share application/owner semantics while retaining their own wire contracts. For Chat Completions use messages, response_format with the same proposal schema/reference enums, max_tokens 4096, stream false and no tools; accept exactly one completed assistant text choice and reject refusal/tool calls/ambiguous/incomplete shapes. Missing usage remains unknown.

Record requested/reported model identity separately in bounded durable provenance. Missing reported identity is unknown, not invented confirmation. A fixture proves no real-vendor compatibility. OpenAI/Claude/Mistral/OpenRouter activation, role routing and automatic failover are outside this PR. Future GLM/Qwen/Kimi selections behind OpenRouter remain qualified model profiles, not an adapter per model.

### B. One immutable timing class; verify every consumer

Select reasoning-bounded-v1 as a finite **resource-policy ceiling, not calibrated provider latency**. Both local profiles and DeepSeek use it here. Add soft_completion_ms to v4's strict operation limits; old permissions cannot acquire successor values.

| Boundary | Selected ms / capacity |
| --- | --- |
| Connect, included in call | 5000 |
| Call soft completion / independent hard stop | 60000 / 62000 |
| Original shared generation + conditional repair clock | 124000 |
| Decision reservation | 129000 = 124000 + stop reserve 2000 + accounting reserve 3000 |
| Inference phase from launch | 134000 = preparation 5000 + decision 129000 |
| Outer inference-child command watchdog | 139000 |
| Acquisition phase and its legacy guard | Existing 60000, clamped to run |
| Run / outer parent command watchdog | 400000 / 410000 |
| Whole-run reservation | 4 calls; request 65536 B; response 262144 B; output capacity 16384 tokens; inference wall time 258000 ms |
| Revocation lease / renewal target / check bound | Preserve 2000 / 400 / 500 |

The allocations leave 72000 ms after two maximum inference phases plus one 60000-ms acquisition phase. This is feasibility arithmetic, not evidence of DB/scheduler/provider tail latency.

Clock origins are part of the contract:
- Keep the original shared deadline established before context/provider setup and reservation can consume that active work; construct it once from validated remaining authority. Readiness/repair/coordinator creation cannot restart it.
- Anchor each call's transport clock at its one-use handoff/guard arm, before marker checks, final release work or network I/O. It includes those pre-send costs and connection/exchange/assembly, not only server thinking time. Setup cannot leave an unwatched interval.
- At soft expiry, cancel unfinished I/O and prohibit new work. Independent hard termination remains available through 62000 ms, earlier if authority/permit/shared/phase/run requires it. A complete body obtained before soft expiry may finish screening/accounting only within the original active/decision bounds; partial output is never admitted.
- Phase/run/bootstrap guards retain their original origins and independent enforcement. Convert absolute remaining authority conservatively; never floor exhausted remainder to positive time.

Use a closed bootstrap hint: successor m1-run requires --input FILE --timing-profile reasoning-bounded-v1; fixed successor child invocations carry the same hint. Validate its syntax before blocking file/SQL work, then require equality with the admitted request/permission/profile before effects. Legacy commands retain original syntax/bounds. No numeric timeout flag, environment alias or unguarded fallback. The hint conveys no authority.

Before each release require enough remaining selected call budget and all shared/decision/phase/run/permission capacity. Reserve generation plus at most one conditional schema repair; repair is not guaranteed after a slow generation. Refuse/defer insufficient remainder visibly, preserve unused reservation accounting, and never reset an envelope.

Require one integration matrix that compares the same class through permission Rust validation, SQL matching/reservation/window, immutable configuration/request, transport soft/hard guard, coordinator, decision envelope, parent phase and outer command guard. Independent SQL enforcement remains mandatory. Reject unknown classes, nonfinite/out-of-range inputs and mismatches. One canonical ADR table plus cross-language assertions prevents drift; do not add more independent knobs.

Keep non-streaming selected. No SSE parser, byte-activity extension or claimed semantic progress. Keep-alive traffic cannot extend an envelope.

### C. Exactly bounded calls; no hidden retry

The only possible second provider call is the existing **conditional schema repair** under its separate identity, current release check and remaining budget. Bind it to the same selected profile and original pack/shared deadline. Preserve existing repairability rules and admitted repair fields/codes.

No transport/SDK reconnect-and-resubmit, fallback, summary, settlement or “recover by calling again”. Exceptions after may-send, response admission, usage accounting, decision publication, Trajectory acceptance or diagnostics cannot invoke Provider again. No output received is not proof of no send/no processing/no charge. Keep may_send_uncertain charged with unknown usage unless actual evidence establishes otherwise.

Add physical adapter-entry/request counters to relevant tests: inject each downstream failure and assert one generation send at most, zero undisclosed replacements, no forced repair and zero provider/DNS activity during repeated inspect/recover. A summary or error wrapper must never catch a downstream error and call its handler again. Do not implement a summarization subsystem merely to test this prohibition.

### D. Record expired reasoning without authorizing stale effects

Freshness and authority are separate checks:
1. Context acquisition and each actual generation/repair release require current Mission/run/window and fresh eligible premises.
2. After admitted release, TTL age alone may cease to clamp waiting. Continue checking owner identity/revision, publication/material/evaluation IDs, source/mode, original validity and temporal frontier, plus conflict/repudiation/integrity hold and Mission/STOP/control lease.
3. Screen a completed safe result, preserving its actual proposal/intent and original times.
4. Revalidate current owner eligibility before consequential use. Historical publication never grants dispatch.

Add a separately named integrity comparison for the successor; do not weaken Terrain's existing fresh comparison or infer “TTL only” from routing_usable=false. Missing/unclassifiable owner state cancels. Preserve expiry = perceived_at + min(lowest applicable TTL, 60 seconds); no timestamp extension, automatic DNS refresh or replay.

Use a typed successor admission result retaining the safe Proposal under recorded_stale_premises when the retained dependencies expired by TTL alone. Publish successor Decision version 2; preserve legacy version 1 and all six proposal intent categories. Never rewrite observe_more into a model-chosen defer. Completed response consumption stays charged/completed; stale action eligibility is separate.

Carry that result through Cognition/outbox -> Trajectory source validation/history -> inspect/recover -> supervisor consumer. A fully recorded stale result is historical completion, not run_inference_pending. Decision 1 with this disposition terminates/defer-disposes visibly with zero DNS; Decision 2 may finish history with current use blocked.

Close both bypass paths: postgres_m1_run::first_proposal/supervisor must read disposition before next_step, and restricted SQL reserve_fixture_dns must independently refuse stale/nonvalidated proposals. Previously validated proposals still need current owner checks. Receipt/history distinguish actual model intent from blocked eligibility; dispatch_granted stays false.

### E. Attribution records observation, not a guessed killer

Use the existing child stdout/readiness channel and parent-owned durable diagnostic path. Add a closed guard frame and successor diagnostic v2 with an optional reported_guard field; retain observed exit/parent action/last marker separately. Allowed guard reports: transport_hard_deadline, decision_envelope, permit_expiry, command_watchdog. A received report means that guard reported its condition, not proof that it won a concurrent exit race.

Keep dependency direction explicit: transport_control owns typed guard values and receives an optional bounded SyncSender from binary composition. Library code cannot import the CLI. The fixed inference child owns one bounded reporter queue/thread; guards only try_send. CLI-local guards may use a nonblocking lookup of the installed sink. Before reporter installation or on a full/disconnected queue, omit the signal honestly. Parent commands without a child channel retain their existing pending/unknown limits.

Guard paths never wait for pipe/stdout/SQL, locks, reporter joins or flush completion. Reporter failure cannot postpone stopping. Extend the bounded frame parser, diagnostic Rust/SQL validators and recovery together. No raw errors/provider text/keys/reasoning in frames. Lost/malformed signals and exit124 remain unknown attribution. A provider TimeoutError/category alone cannot identify a guard. Database-only accounting finishes inside original bounds; no extra LLM settlement turn or cancellation-detached context.

### F. Forward migration and real producer/consumer binding

Add 0021 after existing migrations; do not edit old SQL. Extend latest definitions for strict v4 shapes/profile matching, budget decoding/reservation/window, run start/current/claim/summary, autonomous pack/config/body binding and may-send, issued DNS source/profile lookup and reservation, version-2 Cognition publication/Trajectory validation/history, and diagnostic v2. Make additive compatible table/check changes, not only function changes.

Preserve advisory campaign fences/session epoch ordering, post-lock current reads, exact principal/writer binding and commit/ACK semantics. New decision shapes require current v4-bound original identities; old permissions cannot manufacture stale-record status. No widened grants or admin runtime shortcuts.

Apply 0021 last in bootstrap initial/refresh and run-support refresh; handle additive DDL repeatably in this explicit fixture procedure. Older test targets can reinstall earlier functions: successor tests must reapply the final ordered definitions. Test both fresh installation and upgrade from populated 0020 with old records readable.

The real path is:
registration -> admitted run/profile -> owner-produced immutable pack -> actual configuration/body digest and Broker marker -> selected codec/Provider -> screened decision/outbox -> Trajectory -> supervisor/Broker effect consumer.

Keep Terrain's fixture source owner-derived from the exact stored run tuple; no caller-selected source override or cross-profile issued reference.

## 5. Wire fit and evidence

Keep existing limits: P <=8192 B, configuration/request <=16384 B each, response <=65536 B, proposal <=8192 B, repair material <=2048 B, eight repair codes and existing field/depth/collection bounds.

Preparation measured stored baseline UTF-8 producer artifacts:

| Existing case | P | Configuration | Generation body | Repair body |
| --- | ---: | ---: | ---: | ---: |
| Decision 1 minimum | 3300 | 5503 | 8078 | 8298 |
| Decision 2 minimum | 5131 | 5759 | 10329 | 10549 |
| Provider quote-escape boundary | 7900 | 6350 | 14278 | 14498 |

An earlier offline successor rehearsal reached P 7945 and repair body 14547, leaving 247/1837 bytes respectively. That rehearsal was not new Rust output and did not qualify this R1 configuration. Do not publish it as a production golden.

Generate the successor examples from the **real ContextPack/request producers** for both local protocols, then feed their exact P/C/B through real binding verification and restricted may-send. Measure complete configuration, escaping/UTF-8, schema and repair, not prompt-only bytes. Cover minimum first/second decisions, eight seeds, quote/backslash and multibyte cases, exact limit and over-limit refusal. Required Mission/seed/current Terrain/predecessor linkage wins over optional hypothesis summary; retain safe omission reasons. Eight candidate slots do not promise all maximum-width fields simultaneously fit. No silent truncation or global cap increase.

Relevant local reference evidence: ARTEX f3e3f54 and Decepticon 7c9ba715 inspected 2026-10-23. Adopt explicit config, budget-vs-cancel separation and named causes; reject all-filtered pool fallback, fail-open spend admission and broad-exception retry. AST probes observed nonfinite timeout acceptance, timeout-cause ambiguity and two downstream handler calls in Decepticon's summary wrapper. These justify regressions, not new framework dependencies. Do not copy foreign code or infer ARTEX's installed SDK behavior from the separate unqualified Norma checkout.

## 6. Acceptance tests: close these assertions

Start with honest RED assertions on BASE for successor admission, TTL-only result retention, outer-clock selection and disposition-sensitive consumption. Existing correct behavior uses passing characterization; the ambiguous historical failure is not a deterministic test oracle.

| ID | Required observation |
| --- | --- |
| T1 Two real CLI paths | Run actual m1-run separately with fixture-responses-v1 and fixture-chat-completions-v1. Both traverse restricted owners, one synthetic DNS, Terrain, newly produced feedback pack, second decision and history/inspect. Assert each codec consumed its actual wire; no Accepted shortcuts or copied identities/receipts. |
| T2 DeepSeek boundary and timing | Use the existing private concrete loopback transport fixture with successor bound bytes, screening and authority control. A real-duration response beyond old 20s but before 60s completes. A deliberately blocked/nonreturning exchange crosses soft 60s and is independently terminated by hard 62s under actual renewals; outer 50/60s guards must not kill it first. No admitted late network result. Report actual child stop time; test-custodian cleanup is not product termination evidence. |
| T3 Freshness/use split | Fresh release followed by short-TTL expiry produces charged, durably readable recorded_stale_premises with original times and intent. Normal CLI consumption and direct restricted SQL both release zero stale-dependent DNS. Pre-release expiry, including before repair, sends nothing. Include fresh success and changed-identity/conflict/hold controls. |
| T4 Revocation and stalled control | Long calls stop on STOP, withdrawal/window, parent loss and control lease loss. >500-ms checks/scheduling gaps cannot renew late. Blocked callback/SQL/reporter cannot prevent independent termination; already committed Terrain/history survives. Reuse unchanged negative-control machinery. |
| T5 Profile/budget coherence | Compare the full cross-layer timing/profile matrix. Reject wrong/missing bootstrap hint, illegal/old tuple, recipient/settings/profile rebinding, impossible allocations and insufficient remainder. Preserve old values/bytes. SQL independently refuses forged packets/reservations. |
| T6 No hidden send | Inject provider failure, admission failure, accounting failure and publication/history/diagnostic failure after release. Physical counters and durable call identities show no implicit retry/fallback. Only a confirmed repairable response may use the one distinct repair identity under fresh admission and original shared deadline. |
| T7 Honest attribution | Delivered guard frame is recorded as a report; parent-owned kill is parent-attributed. Dropped/full/disconnected/malformed frames, competing reports and provider-timeout-shaped errors cannot manufacture an exact cause. Recording failure never delays termination. |
| T8 Durability/compatibility | Fresh and populated-0020 migration/refresh, restricted-principal rejection, duplicate/conflicting starts and commit/ACK recovery retain original records/charges. Repeated key-free inspect/recover produces zero provider/DNS replay/refund. Required source/profile bindings and legacy rows remain valid. |
| T9 Assembly/capacity | Real producer -> codec/verifier -> SQL tests cover generation/repair fit, over-limit refusal, split UTF-8/body assembly, unsafe/key-containing decoded output, incomplete/ambiguous response and unknown usage. Streaming-event/delta tests are N/A: no streaming is added. |

Test doubles may be selected only by explicit fixture profiles or existing private test construction. No public endpoint/CA override, sleep/fault flag or unrestricted provider command may be added for testing. Local protocol CLI tests establish application wiring; loopback tests establish concrete transport boundaries. Neither proves a real provider's latency distribution.

## 7. Commands, review and closure

Use process-scoped CARGO_TARGET_DIR for the named ordinary target, then one existing bootstrap session:

```text
python tests/support/m1_native_bootstrap.py --container dwpg-r3 --database fixture --database-name dw_native_lifecycle_20261023a --report D:/DuskWeave-build/rust-1.94.1/reports/inference-lifecycle --session
```

Through its action protocol run the new target and affected native-compatible
Windows wire/permission/run/provider/diagnostic scenarios. Keep DSNs inside
bootstrap/children and serialize the shared fixture. Do not rerun the known
incompatible local legacy role-setup gate.

**R2 qualification assignment (owner-issued resume).** Required PostgreSQL
full-suite execution and production coverage run in exact-candidate Linux CI,
using the existing isolated PostgreSQL 17 fixture and bootstrap `--ci` procedure.
The observed local PostgreSQL 42501 is an environment/setup mismatch before
scenario assertions, not product RED, PASS or a waived gate. Provisioning
administration stays in the owned ephemeral CI fixture. Runtime owners remain
distinct and restricted; do not grant CREATEROLE, ADMIN OPTION, superuser or
BYPASSRLS to local/runtime principals.

Windows qualification remains mandatory for affected native-compatible scenarios,
real CLI behavior, linked libcurl/TLS/resolver boundary, control/termination,
permissions and applicable ordinary/instrumented timing samples, mapped to
T1–T9. Linux success cannot qualify Windows behavior. A required Windows case
that still needs incompatible provisioning is a specific blocker. CI executes
the full scenario suite and unchanged 90% total / 80% per-production-file floors,
without exclusions, ignores, reduced thresholds or shortened product deadlines.
No separate higher-authority local gate is waived by this assignment.

```text
cargo +1.94.1 test --locked --test m1_inference_lifecycle -- --test-threads=1
cargo +1.94.1 fmt --all -- --check
cargo +1.94.1 clippy --locked --all-targets -- -D warnings
python -m unittest discover -s tests -p test_m1_native_helpers.py
python scripts/check_links.py
python scripts/check_structure.py
git diff --check 79b0e3ee1bea44b5e5639c1142af45b6c44918b4
```

Use the repository's existing Linux CI all-targets `cargo llvm-cov` procedure
and `tests/check_coverage_gate.py` for the full suite and production coverage.
Use the separate local instrumented target for required Windows timing/native
qualification; record its selected scenarios without presenting them as full
production coverage. No local duplicate of unchanged PR #109 qualification is
required beyond relevant regressions and the required candidate suite.

After new sources are tracked, measure cumulative complexity; run the diff report against committed HEAD:

```text
python scripts/check_complexity.py --analyzer D:/DuskWeave-build/rust-1.94.1/tools/rca-0.0.25-pr22/bin/target/release/rust-code-analysis-cli.exe --report-dir D:/DuskWeave-build/rust-1.94.1/reports/inference-lifecycle/metrics --base 79b0e3ee1bea44b5e5639c1142af45b6c44918b4
python scripts/check_runtime_budget.py --base 79b0e3ee1bea44b5e5639c1142af45b6c44918b4 --head HEAD
```

McCabe ratchet remains enforced; no exception is selected. LOC/diff/function-span crossings are advisory and require a cohesion decision, not helper proliferation or removal of failure behavior. Estimated breadth is several dozen paths because one serialized/temporal contract spans producers, independent SQL validators and real consumers; do not use that estimate to authorize adjacent cleanup.

Perform QUALITY_BAR section 7 self-review, then a **different reviewer instance** for adversarial review. Challenge clock origins and handoffs, direct SQL bypasses, hidden sends, stale-use consumers, lost guard signals, source identity and both CLI protocol paths. Self-review is not the independent review. Keep routine corrections in this PR; at most two substantive FIX cycles before architecture root-cause review of unresolved blockers.

After completed implementation and self-review, commit only intended allowed
files and open/update a DRAFT PR on this branch for candidate CI. Keep
DRAFT/NOT READY while qualification is pending. Completion requires a committed
candidate/PR, exact-candidate required CI, distinct-review disposition, T1–T9
assertion-to-environment evidence and updated tracking/guide. Distinguish the PR
head from any tested merge SHA. Record exact source/run evidence in PR or
verification output, and actual UTF-8 margins, ordinary/instrumented termination
samples and limitations here. Never embed the candidate's own SHA in committed
tracking files or infer calibrated percentiles from a few samples.

**Finish at READY FOR REVIEW.** No merge, provider activation, real demo, live acquisition, M1 seal, web UI, document decomposition, automatic failover or LLM compaction. Fresh real-provider fixture authorization is the next action after runtime qualification.

STOP for a material base/authority conflict, missing required sealed dependency, invariant violation, unexpected necessary cross-domain/file expansion, unsafe unrelated changes, or unavailable mandatory qualification resources. Report the exact blocker. Do not stop for advisory size, routine compiler/test defects or an absent real provider key. Correct routine failures in this PR; never silently alter the selected profile to hide a failed qualification.

## 8. Execution record: blocked uncommitted work

**Current R2 disposition:** the owner-issued resume instruction resolves the
legacy Windows full-suite/coverage environment assignment in section 7.
The initial runtime candidate is [DRAFT PR #116](https://github.com/carlitotate12160-tech/DuskWeave/pull/116).
Independent review found two valid defects; FIX cycle 1 and final qualification
remain in progress, so the disposition is NOT READY. Retained bootstrap session
a ended without credential recovery. The original R1 incident and partial test
evidence below remain historical, not current acceptance evidence.

The selected worktree was clean at the issued BASE before creating
`feat/m1-inference-lifecycle`. PR #115 is verified merged at BASE. The dependency
outcomes were also read from GitHub: #110/#112/#113 merged; #111 closed without
merge. Cargo/dependency files are unchanged and sha2 remains 0.11.0.

Windows x86_64 MSVC Rust 1.94.1 and Python 3.12.10 were verified. Owned `dwpg-r3`
was running PostgreSQL 17.11 with the issued image digest and loopback binding.
The database-name collision check returned zero. The prescribed bootstrap then
created `dw_native_lifecycle_20261023a`, generated distinct restricted logins,
applied migrations through 0020 and reported ready. Credentials stayed inside
the bootstrap and children; the provider key was removed from the offline
process environment. Only the ordinary target was used. The linked libcurl
backend, instrumented target and termination samples remain unqualified.

**Mandatory local qualification blocker.** The existing full-suite fixture is
incompatible with the prescribed local bootstrap's administrative principal:

```text
cargo +1.94.1 test --locked --test registration_postgres registration_lifecycle_idempotency_and_conflicts -- --exact --test-threads=1
exit 101; 0 passed, 1 failed
tests/support/registration_db.rs:132
PostgreSQL 42501: permission denied to alter role
Required: CREATEROLE and ADMIN OPTION on the generated runtime role.
```

This occurs in setup before the scenario assertions; it is neither product RED
nor a full-suite PASS. `tests/support/m1_native_bootstrap.py` creates the local
admin with `--no-createrole`, while the unchanged legacy fixture issues
`ALTER ROLE ... LOGIN PASSWORD ...`. No password or DSN is reproduced here.
The role/credential change and a replacement of the explicitly required local
full-suite/coverage gate were not authorized. Implementation stopped at the
packet's unavailable-mandatory-resource condition. An owner disposition is
pending on using exact-head Linux CI for full-suite/coverage while retaining
the applicable Windows qualification. Until disposition, the R1 gates remain
unchanged. The bootstrap refresh completed and restored final ordered 0020
definitions after the legacy fixture's setup.

**Partial source, not a runtime delivery.** Uncommitted changes introduce the
closed profile/timing values, strict v4 permission parsing, explicit v2 run and
pack fields, early closed CLI timing-hint parsing, successor request encoding
and binding verification, and an initial bounded Chat Completions decoder.
The full child/transport timing path, protocol adapters, migration 0021,
TTL-only reasoning retention, disposition-sensitive SQL/effect consumption and
guard reporting are not implemented or qualified. The decoder and profile
selection are not yet a complete production path. No runtime completion or
dead-code/reachability closure is claimed.

Observed behavioral REDs covered successor admission, v2 run admission,
bootstrap-hint parsing and refusal of recomputed-hash profile rebinding.
The last focused run after formatting passed **11 tests** across:

```text
cargo +1.94.1 test --locked --test m1_inference_lifecycle --test m1_autonomous_permission --test m1_autonomous_wire --test m1_run_cli -- --test-threads=1
```

This includes four initial successor tests, two legacy permission tests, two
legacy wire tests and three CLI tests. The legacy wire cases preserve their
existing golden bytes. These are working-source checks only. Successor wire
examples from real ContextPack producers, physical send counters, T1–T9
acceptance, full suite, coverage, metrics, author self-review and distinct
adversarial review remain incomplete. There is no committed candidate, PR or
candidate CI, and the outcome is **NOT READY**. Preserve this WIP and the owned
fixture/reports; do not recover credentials if the bootstrap session ends.
No provider/target request, merge or seal was performed.

Sanitized action logs remain under the issued report directory:
`red-successor-admission.log`, `red-run-and-bootstrap-hint.log`,
`red-bootstrap-hint.log`, `red-successor-wire-binding.log`,
`final-wip-focused.log` and `preflight-legacy-db-gate.log`.

WIP hygiene checks passed: pinned `cargo fmt --all -- --check`, document links
(after correcting this execution-record anchor), structure (advisory size
warnings only), and `git diff --check` against BASE. These do not substitute
for the missing runtime acceptance, coverage, complexity or review gates.

### R2 continuation observations (uncommitted, NOT READY)

The continued worktree remains on the issued branch and BASE. Owned `dwpg-r3`
was requalified as PostgreSQL 17.11, the issued image digest and loopback port.
After proving `dw_native_lifecycle_20261023b` absent, the authorized bootstrap
created that fresh fixture with distinct restricted owners and migrations
through 0021. Fixture a, its roles and reports remain retained. No credentials
were recovered, printed or persisted; offline children remain provider-key free.

Working-source observations, not committed-candidate qualification:

- `r2b-lifecycle-clock-compile`: six tests passed, including both actual CLI
  protocols, two decisions/history and key-free inspect/recover with one DNS.
- `r2b-red-guard-diagnostic-assertion` demonstrated a real rejection of the
  successor diagnostic's closed report; `r2b-guard-green` passed after the
  Rust extension. The earlier compiler failure was not product RED evidence.
- `r2b-lifecycle-and-diag-green`: seven tests passed, including both CLI paths
  after the SQL wire reconstruction and diagnostic-v2 changes.
- `r2b-transport-real-duration`: ordinary concrete loopback transport completed
  a response after 22,005 ms with 55 actual 400-ms permit renewals. A blocked
  callback crossed soft completion and was product-terminated at 61,714 ms
  with a full reporter queue. Each mode had one physical request. These are
  individual ordinary samples, not calibrated percentiles or a live-provider
  result; instrumented qualification remains required.
- `r2b-ttl-issued-premises`: the actual first decision/issued DNS/Terrain
  sequence supplied the retained premise. Its unchanged 60-second TTL was
  allowed to age to four seconds remaining before release. Post-release
  expiry retained charged/completed historical reasoning; pre-release expiry
  prevented the second call. Identity, conflict and hold controls refused
  admission; repeated recovery added no DNS. Six modes passed in 148.95 s.
  This does not yet close the separate first-stale-result consumer proof.
- Initial cumulative McCabe inspection reported 17 regressions. Owner-local
  refactoring reduced these to zero in `r2b-complexity-profile-complete`;
  ten unchanged baseline debt items remain. `r2b-clippy-refactor` passed
  all-target compilation with warnings denied. Later source/test additions
  require affected rechecks; neither result qualifies the final candidate.
- `r2b-wire-production-examples`: 24 producer-derived first/feedback examples
  passed across both local protocols, including eight seeds, quote/backslash,
  multibyte, optional-summary omission and exact 8,192-byte ContextPacks.
  Exact-limit repair bodies were 14,596 / 14,621 bytes, margins 1,788 / 1,763.
  Full configuration/schema/body bytes crossed Rust verification and restricted
  SQL generation/repair admission; recomputed configuration forgeries were
  refused. The generated artifact is in the exact allowed fixture path.
- `r2b-new-consumer-failures`: eleven selected tests passed. The first-stale
  consumer test injects the typed owner classification at the private port;
  it then exercises actual publication/history, the same store consumer used
  by the CLI, restricted DNS refusal and key-free recovery with zero DNS.
  Actual TTL classification remains the separate issued-Terrain scenario.
  Provider/unsafe-output/publication/history faults made one adapter entry;
  only a genuinely repairable codec response made the second distinct call.
  Repeated recovery added no provider/DNS activity. Closed diagnostic-v2
  reports survived restricted publication without changing the parent cause.
- `r2b-red-shared-sql-origin` exposed a renewed SQL duration on repeated reads.
  The successor now retains a writer-bound original decision window, clamps
  it to the original phase/run authority and refuses insufficient call
  remainder; legacy clock selection is unchanged. The monotonic CLI origin
  is sampled before its window query. `r2b-green-shared-sql-origin` passed
  all three profile matrices and raw missing-field/timer controls.

Full T1–T9 closure, final local checks, instrumented samples, self-review,
committed candidate/DRAFT PR, exact-candidate Linux full-suite coverage and the
distinct reviewer instance remain pending. No provider/target/resolver contact,
merge, real episode or seal is authorized or claimed.

### R2 completed runtime and qualification ledger

This supersedes the incomplete-implementation descriptions above, preserving
their incident history. All observations below initially apply to working
source; the delivered Git commit and its CI/review are identified in the PR.
No working-source PASS automatically qualifies a later committed candidate.

The real CLI selects the immutable profile, builds a successor pack, reserves
through the restricted Broker and anchors the original SQL/monotonic window.
The coordinator supplies the atomic revocation permit to either controlled
local protocol or concrete DeepSeek transport. Raw protocol output crosses the
normal codec and proposal admission, Cognition publication, Trajectory and
the supervisor consumer. The fixture factory uses the same guarded control
boundary: there is no unguarded local-profile branch in production. The
second pack consumes the actual first decision and owner-published Terrain.
Migration 0021 independently reconstructs wire configuration and validates the
source pack, original window and current effect eligibility. Inspect/recover
exposes actual intent and disposition without granting dispatch or replaying.

| Assertion | Windows evidence and limits | Remaining candidate gate |
| --- | --- | --- |
| T1 | `r2b-lifecycle-complete-native`: both real CLI profiles traverse two decisions/history and one issued DNS/Terrain; exact bound calls and separate protocol envelopes checked. | Same tests in full Linux CI. |
| T2 | `r2b-transport-real-duration`: response 22,005 ms, 55 renewals; blocked callback product exit at 61,714 ms, full reporter queue, one physical request each. Repeat ordinary sample 22,012 / 61,750 ms. Windows instrumented sample pending. | Instrumented Windows sample and full Linux CI. |
| T3 | Actual issued Terrain retained its original 60s TTL; fresh-release expiry, pre-release expiry, hold/conflict/identity controls passed. First-stale consumer uses a private typed owner-result injection, then real publication/history, the CLI's `first_step` consumer and direct restricted SQL with zero DNS. This is separate from the actual TTL-classification proof. | Same assertions in full Linux CI. |
| T4 | Native provider/control tests cover STOP, withdrawal, parent/control loss, original envelopes and blocked SQL; controlled local protocols terminate blocked screening at 1,988 / 1,975 ms with a full reporter queue. Binary unit controls 56/56 passed; the subsequently added production-factory stopped-permit negative passed separately. | Instrumented selected controls and full Linux CI. |
| T5 | Full three-profile Rust/SQL matrix passed, including immutable remaining SQL duration and forged source/configuration/timers. Key-free DeepSeek CLI retains its profile, publishes `live_key_missing`, has zero calls/DNS and never falls back during repeated recovery. Legacy bytes and permission targets passed. | Same assertions in full Linux CI. |
| T6 | Physical adapter counters cover provider, unsafe-output, publication and history failures; only a repairable codec result invokes one distinct repair call. Legacy admission's 10 cases preserve uncertain accounting/marker-ACK refusal. Inspect/recover stays key free with zero new calls/effects. | Full Linux suite includes accounting/diagnostic failure machinery. |
| T7 | Strict optional diagnostic-v2 reports cross restricted publication; null/unknown/version-conflicting fields refuse. Closed frame/parser, competing/lost/full/disconnected reporter controls retain parent cause and never infer an exact cause from exit/time. | Instrumented controls and full Linux CI. |
| T8 | Populated legacy owner records survive applying 0021 twice unchanged; current-source bootstrap refresh passed under the existing restricted fixture admin. Native duplicate/conflict/commit-ACK/recovery targets passed. The shared catalog already contains new DDL during the populated-owner-record rehearsal; fresh bootstrap separately traverses 0001-0021. | Full legacy provisioning/upgrade suite in Linux CI. |
| T9 | 24 producer-derived examples exercise both protocols, first/feedback packs, eight seeds, escaped and multibyte text, exact/over limits and SQL generation/repair admission. Existing concrete assembly/unsafe-output tests remain selected. No streaming: event/delta cases are N/A. | Instrumented selected decoder/transport tests and full Linux CI. |

`r2b-lifecycle-complete-native` passed 15/15 in 349.62 s; the later
DeepSeek-no-key case passed separately in `r2b-no-key-receipt-parser`. The
remaining native compatibility run passed provider 17, run 4, CLI 3,
diagnostics 8, failures 14 and PostgreSQL 2 tests. The 36-pass library run had
one TLS fixture prerequisite failure (OpenSSL absent from PATH), so it is not
a whole-run PASS. The same TLS case then passed using the existing absolute
OpenSSL/Python paths; no installation or host change occurred.

Concrete linked backend: vendored libcurl 8.22.0-DEV / curl-sys
0.4.91+curl-8.22.0, Schannel, asynchronous threaded resolver (`c_ares=None`,
`USE_RESOLV_THREADED=1`). Samples are individual loopback observations, not
provider latency calibration or scheduler percentiles. The only traffic is
owned local PostgreSQL and explicit loopback test endpoints.

The generated wire artifact has 24 complete producer examples. At exact
P=8,192 bytes, Responses / Chat Completions configurations are 6,429 / 6,443;
generation bodies 14,289 / 14,342; maximum eight-code repair bodies 14,596 /
14,621, leaving 1,788 / 1,763 bytes. Eight-seed feedback preserves required
seeds/Terrain/predecessor; optional summary omission is explicit `byte_bound`.
No cap or required-input truncation was introduced.

Author self-review under QUALITY_BAR section 7 (static inspection plus the
above working-source assertions; final candidate CI remains pending):

| Question | Disposition and evidence |
| --- | --- |
| Q1 | PASS: immutable profile values, protocol codecs, owner-qualified Terrain comparison, CLI composition and SQL trust boundaries retain separate owners. No new mutable cross-domain aggregate. |
| Q2 | PASS: library owns typed guard reports and optional bounded sender; CLI injects its reporter/coordinator. Domain code imports no CLI, database or transport implementation. |
| Q3 | PASS: one guarded provider factory, one shared `CallClock`, one diagnostic version selector and private fixture setup serve actual production boundaries. No generic manager, pooling or optional consolidation. |
| Q4 | PASS: ADR timing table, closed Rust lookup and independent SQL assertions agree in the three-profile matrix. SQL retains the original window rather than issuing renewed durations. Earlier window, pack-identity and local-control findings were VALID and fixed with behavioral RED/GREEN evidence. |
| Q5 | PASS for implemented scope; candidate qualification UNVERIFIED: exact allowed map, unchanged legacy/dependency/CI files and T1-T9 ledger. No provider activation or seal. |
| Q6 | PASS: `main -> m1_run_cli -> m1_run_inference -> owner preparation/reservation -> selected guarded adapter -> codec/admission -> Cognition/Trajectory -> first_step/inspect`. Both actual CLI protocols exercise the consumer; changed modules/migrations/configuration have real callers. No orphan retained. |
| Q7 | PASS for local affected assertions; full-suite gate UNVERIFIED: physical send counters, one conditional repair, uncertain markers, original clock origins, repeatable migration and no-replay recovery. |
| Q8 | PASS for examined local boundaries; independent review pending: original source identity/freshness and current authority are separate; stale proposals preserve intent but cannot release DNS, including direct SQL; credentials stay outside ordinary surfaces. |

Advisory size disposition: retain migration 0021 as one ordered additive
trust-boundary revision, including the complete static proposal schema used
for independent wire reconstruction. Retain the lifecycle target/private
fixture as one T1-T9 owner/consumer scenario matrix; splitting would obscure
shared setup and assertion flow. Existing large transport/CLI modules keep
their established responsibility and private tests. Long declarative schema
and test matrices are not dispatch exceptions. McCabe remains enforced:
`r2b-source-freeze-complexity` reported ten retained baseline debt items and
zero regressions. No threshold, exception or exclusion changed.

Changed production function-span triggers were reviewed: profile `lookup`
retains one closed declarative selection table; autonomous `template` retains
the complete schema; `infer` keeps one ordered decision/publication receipt;
`decide` and `successor_decision` retain explicit composition/injection and
watchdog handoff. Their measured own complexities are 6, 1, 4, 7 and 5.
None claims the pure-dispatch allowance; the sole retained allowance is
`main::run` at cap 10. Other reported long functions are unchanged owners or
cohesive private test matrices, not new production debt.

Final source hygiene observed: pinned fmt, all-target Clippy with warnings
denied, Python helper 29/29, links, structure and whitespace checks passed;
structure has advisory size warnings only. Windows instrumentation is running
in the isolated coverage path. Commit, DRAFT PR, exact-candidate Linux
full-suite/90%-80% coverage and the different reviewer instance remain required
before READY FOR REVIEW.

### PR review and FIX cycle 1

Initial [CI run 39939070051](https://github.com/carlitotate12160-tech/DuskWeave/actions/runs/39939070051)
passed Rust all-targets and the unchanged coverage gate: 96.73%
(12,139/12,549 production lines), every production file at least 80%.
Its log binds the PR head and tested merge checkout; exact identities stay in
the PR. Metrics/structure/links/CodeQL/Geiger passed. Dependencies and `ci-ok`
failed on newly published `RUSTSEC-2026-0385`, affecting existing ctutils 0.4.2.
The owner explicitly authorized committing/pushing their ctutils 0.4.5
lockfile update on 2026-10-23. Only its version/checksum changed; sha2 remains
0.11.0. A separate commit in this PR contains that correction. Local
`cargo audit --json --no-fetch` then passed: 103 dependencies, zero
vulnerabilities, no ignored advisories or warnings. Candidate CI must still
verify the dependency gate against its own advisory snapshot.

Initial Windows instrumentation passed 41 library, 57 binary and 16 lifecycle
tests; response/blocked-transfer samples were 22,008 / 61,675 ms, one request
each. These are pre-correction results, not final qualification.

The distinct reviewer instance confirmed CodeRabbit's permit-watchdog lifetime
finding and Codex's causal inspection ordering finding. It found no additional
independent blocker. The watchdog remedy retains an original-deadline permit
guard through screening and subsequent owner recording/accounting/publication,
overlapping the call-clock handoff and keeping both controlled protocols
equivalent. New network and local post-call regressions demonstrated actual
custodian-required termination failures before the fix. Qualified ordinary
GREEN passed four cases: network screening/recording product exits at
1,959/1,965 ms, local post-call exits at 1,990/1,976 ms, one network request.
The reviewer accepted that working-source delta; candidate binding and final
CI remain required. CodeRabbit's 80% docstring warning is advisory, with no
corresponding DuskWeave acceptance threshold.

The committed deterministic reverse-UUID owner/CLI inspection regression
failed behaviorally in [CI run 39970114164](https://github.com/carlitotate12160-tech/DuskWeave/actions/runs/39970114164):
16 lifecycle cases passed, while the ordering assertion observed the actual
second UUID before the first. Migration 0021 now orders recorded decisions
by their bound run phase rather than random UUID value. GREEN and final
candidate coverage remain required. Bootstrap session b ended during the
inactive interval; its credentials were not recovered. Databases a/b and
all evidence remain retained. New fixture c requires a bounded owner
disposition, requested explicitly; it has not been provisioned.

Post-correction ordinary library requalification failed before transport at
the unchanged 1,500-ms Windows ACL probe (`run_control_unqualified`); a later
attempt also exited 124. These are retained failed runs, not PASS. Five
bounded read-only ACL probes completed at 1,368/1,151/1,347/1,223/959 ms, but
do not establish reliable platform qualification. Product deadlines, ACLs,
host settings and runtime privileges are unchanged. Focused instrumentation
is separate from the unresolved affected Windows gate. Final disposition
remains DRAFT / NOT READY, with both review threads and the dependency gate
tracked in PR #116. No merge, provider activation or seal is authorized.

Focused post-correction Windows instrumentation passed both network cases
(1,960 / 1,967 ms; one physical request each) and both local-protocol cases
(post-call 2,003 / 1,995 ms; screening 1,989 / 1,982 ms). The subsequent
real-duration T2 case refused before its physical request; the diagnostic
rerun records child exit 101 / `run_control_unqualified` during startup
(1.61 s), so neither attempt is timing qualification. The complete ordinary/instrumented affected gate
remains unresolved; passing focused controls do not waive it.

After extracting the completed-envelope admission boundary into `admit_raw`,
the ordinary post-call local-protocol test passed (2,000 / 1,993 ms), but
the selected screening case exceeded its unchanged 2,300-ms assertion.
That failed run does not qualify the refactor or the new dependency graph;
affected rechecks, the McCabe ratchet and final candidate CI remain required.

The subsequent unchanged-bound ordinary recheck passed all four focused cases:
local post-call 1,974/1,976 ms, local screening 1,985/1,977 ms, network
screening/recording 1,951/1,951 ms with one physical request each. The
cohesive-source McCabe remeasurement passed with ten retained debt items and
zero regressions. These working-source results do not replace final-candidate
Windows qualification or Linux SQL/coverage evidence.

The distinct reviewer accepted the completed-envelope extraction and SQL
ordering delta without another valid finding. `admit_raw` measures 23 lines /
McCabe 6; `ProtocolFixture::call` measures 16 / 7. Retain the private test
matrices and this contiguous packet/evidence record despite advisory size
crossings: both preserve one lifecycle contract and its incident chronology.
Post-correction fmt, all-target Clippy, native-helper 29/29, links, structure
and whitespace checks passed. Final committed-candidate checks remain pending.

Candidate Windows ordinary and instrumented library runs then passed 44/44
each (196.95 / 200.27 s). T2 response/blocked samples were 22,006/61,772 ms
ordinary and 22,006/61,767 ms instrumented, one physical request each and
55 renewals for the responses. Selected instrumented supervisor/factory tests
passed 13/13; ordinary wire/CLI parsing passed 2/2 + 3/3. The following
ordinary binary run had 50 passes and seven setup failures: absent restricted
fixture configuration poisoned initialization. It is not a whole-run PASS.

[CI run 39973040964](https://github.com/carlitotate12160-tech/DuskWeave/actions/runs/39973040964)
passed dependencies, metrics, structure, links, CodeQL and Geiger; mutation
run 39973041189 also passed. SQL causal ordering assertions passed for inspect
and repeated recovery. Its new test then failed at custodian shutdown:
completing Decision 2 revokes the lease, so continued renewal correctly
returned `run_stopped`. The test now joins its active-work custodian before
terminal phase completion, preserving all order/history/effect assertions.
No production timing, renewal policy or SQL behavior changes in this cleanup.
Final CI/coverage and native fixture qualification remain required.

The CI project audit has 103 dependencies, zero vulnerabilities and zero
ignores. Its fresher advisory snapshot reports informational `unsound`
RUSTSEC-2026-0386 for transitive Tokio 1.53.1. Upstream Tokio PR 8587 confirms
the AsyncFd constructor issue; no direct AsyncFd/AsRawFd use exists in
repository production/tests, while transitive reachability remains UNVERIFIED.
This non-gating follow-up does not widen the ctutils-only lockfile allowance.

### Qualification FIX cycle 2 (2026-10-24)

The owner-issued `DW-FIX-PR116-QUALIFICATION` closes two evidence gaps in this
same outcome: the affected Windows native/CLI execution lacked a valid
process-scoped fixture, and the original T8 started after 0021 DDL existed.
These are qualification findings, not newly demonstrated production defects.
Starting-candidate [CI run 39976038653](https://github.com/carlitotate12160-tech/DuskWeave/actions/runs/39976038653)
passed full-suite PostgreSQL execution and 96.75% production coverage
(12,198/12,608), every file at least 80%. Its tested merge tree matches the
reviewed source; that result does not qualify this correction automatically.

Bootstrap now selects only schema revision `0020` or `0021` (default `0021`).
The predecessor selection is fixture-only and applies to both initial
migrations and refresh; invalid/live combinations refuse before provisioning.
One-shot report collisions also refuse before provisioning. Provisioning,
restricted logins, migration ordering and CI administrative separation retain
their existing owners. The helper suite passes 35/35 and unchanged progress
controls 3/3. The initial helper RED observed 19 migrations instead of 18;
the missing constructor argument was a setup error, not behavioral RED.

T8 selects its private child branch before `native::db` or any refresh. Its
outer branch launches the same already-built executable, with the exact T8
filter, under a fresh `dw_native_lifecycle_upgrade_<unique>` 0020 database and
distinct restricted logins. Windows uses owned `dwpg-r3`; Linux uses the
existing `dwpg --ci` path automatically in the all-targets suite. Each nested
bootstrap has its own retained temporary report directory. No Cargo runs
inside the test, LLVM profile/executable identity is inherited, malformed
child markers fail, and both worker completion and exactly one passing test
are required. The 240-second test custodian fails on timeout; cleanup is not
product termination evidence.

The catalog RED on a fresh 0021 arrangement failed at `successor table already
exists`. Staging then checks the absent successor table/functions, existing
0020 diagnostics and version-1 history constraint before setup and again
after actual restricted registration/inference/history population. The final
ordinary focused T8 passed 1/1 in 42.62 s using the existing legacy CLI phase
consumer. It ends active-work lease renewal before terminal disposition,
snapshots actual run/call/binding/reservation/closure/outbox/history rows, then
applies the unchanged 0021 migration twice as the isolated migration owner.
Fresh restricted connections and fresh inspect/recover processes preserve
the complete snapshot, payloads, original identities, charges and counts;
each upgrade has two recoveries with zero additional calls/DNS/refunds.
Earlier staging-development failures and all generated fixtures remain
retained; they are not PASS. No production or migration source changed.

The Windows current-schema fixture `dw_native_lifecycle_20261024a` was
provisioned, but the tool closed stdin and the bootstrap session ended before
an action ran. Its credentials were not recovered; the fixture is retained.
A fresh replacement fixture requires a bounded owner disposition. Until
that disposition and the four required ordinary/instrumented binary/lifecycle
actions execute successfully, the affected Windows gate is BLOCKED, not PASS.
Ordinary artifacts remain at `D:/DuskWeave-build/rust-1.94.1/normal`;
instrumented artifacts remain separately owned at
`D:/DuskWeave-build/rust-1.94.1/coverage/inference-lifecycle`.

Source-bound reuse: the earlier 44/44 ordinary and instrumented library runs,
real-duration T2 observations and selected 13/13 supervisor/factory controls
remain evidence for their unchanged production/test consumers and baseline
environment. They do not substitute for the assigned Windows binary and
lifecycle runs or this correction's exact-candidate Linux CI.

Working-source fmt, all-target Clippy, links, structure and whitespace checks
passed. Author Q1-Q4/Q6: PASS for the examined delta (one staged-fixture
owner, closed schema selection, existing native consumer paths and no new
production component). Q5/Q7/Q8: PASS for executed T8 assertions and
synthetic refusal/redaction controls; final platform/CI qualification remains
UNVERIFIED. Retain the cohesive lifecycle scenario/helper files and incident
chronology despite advisory size crossings; no count-driven extraction or
threshold change is justified. The distinct delta reviewer independently ran
the 35 helper tests, inspected retained catalog RED/T8 GREEN logs and the
seven-file delta, and found no new VALID defect. Its bounded source review
passes; final committed-candidate binding and Linux CI remain required.
Existing runtime-budget reporting measures zero production changed lines in
this correction and 3,483 cumulatively against the original PR base; the
original cohesive-runtime advisory disposition remains applicable. Delivery
stays DRAFT / NOT READY; no
provider activation, target contact, merge or seal is claimed.

### Qualification STOP and root-cause disposition (2026-10-24)

The owner authorized replacement fixture `dw_native_lifecycle_20261024b`
after the previous session's stdin EOF. A TTY-backed bootstrap captured the
ordinary target and absolute existing Python path before provisioning; keys
were absent. All four assigned actions ran serially on Windows x86_64 MSVC,
Rust 1.94.1, cargo-llvm-cov 0.9.1 and the retained owned PostgreSQL 17.11
`dwpg-r3`. Each action's exit value and retained log were checked separately.

| Action log | Executed Cargo command | Result / test duration |
| --- | --- | --- |
| `fix2b-native-bin.log` | `cargo +1.94.1 test --locked --bin duskweave -- --test-threads=1` | Exit 0; 57 PASS, zero failed/ignored; 40.66 s |
| `fix2b-native-lifecycle.log` | `cargo +1.94.1 test --locked --test m1_inference_lifecycle -- --test-threads=1` | Exit 0; 17 PASS, zero failed/ignored; 380.56 s |
| `fix2b-cov-bin.log` | `cargo +1.94.1 llvm-cov --locked --bin duskweave --no-report -- --test-threads=1` | Exit 0; 57 PASS, zero failed/ignored; 44.00 s |
| `fix2b-cov-lifecycle.log` | `cargo +1.94.1 llvm-cov --locked --test m1_inference_lifecycle --no-report -- --test-threads=1` | Exit 101; 15 PASS / 2 FAIL, zero ignored; 469.95 s |

Logs remain under
`D:/DuskWeave-build/rust-1.94.1/reports/inference-lifecycle/fix-qualification`.
The two instrumented actions used a per-action Python subprocess that set
both `CARGO_TARGET_DIR` and `CARGO_LLVM_COV_TARGET_DIR` to the isolated
coverage path. Ordinary executables remained in the normal path. No global
environment, local Cargo configuration, privilege or deadline changed.
The session exited normally after the failed action; shutdown exit 0 is not
a qualification PASS. Both current-schema fixtures and all nested upgrade
fixtures/reports remain retained; ended-session credentials were not recovered.

The mandatory instrumented lifecycle gate exposed two distinct blockers:

1. **Platform qualification UNVERIFIED / blocking.** The real Responses CLI
   stopped at Decision 1 with durable parent-observed `readiness_deadline`,
   elapsed 5,068 ms, `readiness_observed=false`, `child_error=null` and
   `run_child_terminal_unknown`. It recorded zero decisions, history or effects.
   A read-only query confirmed that diagnostic. This identifies the parent's
   observation, not the underlying child guard or an ACL/SQL/scheduler cause.
   The exact internal cause remains UNVERIFIED; no envelope was enlarged and
   no retry was performed.
2. **Fixture repeatability defect VALID / blocking.** The reverse-UUID causal
   inspection test assigns fixed `MAX - 0xe620` and `0xe620` decision IDs.
   Its ordinary invocation retained those IDs in this same fixture. The
   required instrumented invocation therefore failed with SQLSTATE 23505 on
   `autonomous_runs_decision_one_key` before the ordering assertion. A
   read-only catalog query confirmed the retained original pair. This is a
   test identity collision, not a demonstrated production ordering failure.
   The smallest future remedy is unique per-run identities that preserve the
   adversarial first-greater-than-second ordering, inside the existing test
   file. It was not silently patched or rerun past this cycle's STOP boundary.

The qualification-source [Linux CI run 40056489936](https://github.com/carlitotate12160-tech/DuskWeave/actions/runs/40056489936)
passed all nine required jobs, including full-suite real PostgreSQL execution
and 96.75% production coverage (12,198/12,608), every file at least 80%.
Its tested merge checkout has the same tree as the qualification head;
exact identities remain in verification output. The downloaded sanitized
artifact confirms automatic staged T8 and lifecycle 17/17 (221.47 s).
Parser mutation run 40056489947 also passed. Linux success does not waive
the failed mandatory Windows gate. Current-head CI is read from the PR;
later tracking-only commits do not automatically inherit a prior run's PASS.

Current T1-T9 disposition supersedes the initial qualification table above:

| Assertion | Current executed evidence / limit |
| --- | --- |
| T1 | Ordinary two-protocol CLI and causal inspection PASS; instrumented real CLI readiness and repeated-fixture UUID collision FAIL. Blocking. |
| T2 | Unchanged ordinary/instrumented 44/44 library timing evidence remains source/consumer/environment-bound; Linux full-suite PASS. No new timing claim. |
| T3 | Ordinary and instrumented stale-intent, actual TTL/integrity and independent zero-DNS SQL cases PASS; Linux PASS. |
| T4 | Ordinary/instrumented binary 57/57 PASS, with unchanged source-bound library/selected-control evidence retained; Linux PASS. |
| T5 | Ordinary/instrumented profile, admission/window and no-key/no-fallback cases PASS; Linux PASS. |
| T6 | Ordinary/instrumented failure, accounting and bounded repair cases PASS; Linux PASS. |
| T7 | Ordinary/instrumented diagnostic boundary/publication cases PASS; Linux PASS. |
| T8 | Genuine staged populated-0020 upgrade PASS ordinarily, instrumented and automatically in Linux CI; original payloads, bindings, charges and history retained after repeated migration/recovery. |
| T9 | Ordinary/instrumented producer/wire examples and restricted SQL controls PASS; Linux PASS. |

The distinct reviewer independently read all four action logs and confirmed
both blockers, preserving the earlier source review and the bounded reuse of
unchanged library/T2/control evidence. Q1-Q4/Q6 remain PASS for the examined
correction; Q5/Q7 are FINDING for qualification/fixture repeatability and Q8
remains UNVERIFIED for the unexplained readiness failure. No production or
migration source changed. FIX cycle 2 has reached its explicit STOP: delivery
is **DRAFT / NOT READY**, requiring a bounded owner root-cause disposition.
There is no new cycle, automatic replay, provider/target contact, merge or seal.

### Bounded readiness disposition (2026-10-24)

The owner-issued `PR116-READINESS-DISPOSITION` re-scopes the existing STOP:
correct the established test identity collision and collect bounded startup
evidence. It does not reset the two substantive FIX cycles or authorize a
speculative runtime correction. Preflight verified the clean selected worktree,
branch `feat/m1-inference-lifecycle`, origin and starting commit
`f46c1d2aecd83e755095a64276771335c763e51d`; the PR base remains
`79b0e3ee1bea44b5e5639c1142af45b6c44918b4`. Exact-start-head
[CI 40059942732](https://github.com/carlitotate12160-tech/DuskWeave/actions/runs/40059942732)
was completed SUCCESS, including Rust and all nine required jobs; parser
mutation and the external checks were also SUCCESS. This qualifies that
starting candidate, not the later test changes or a new coverage percentage.

Only `tests/m1_inference_lifecycle.rs`, its existing support file, this ledger,
`docs/ENGINEERING_STATE.md` and `docs/BUILD_ORDER.md` change. Tracking disposition
is UPDATE: name the correction, observation limits and next owner action.
Production, migrations, permissions, protocol allowlists and deadlines do not
change. The existing same-PR workflow remains DRAFT / NOT READY.

**Correction A / RED provenance.** The retained
`fix-qualification/fix2b-cov-lifecycle.log` records genuine SQLSTATE 23505 on
`autonomous_runs_decision_one_key`, before any history assertion. The corrected
test takes the two distinct decision UUIDs already allocated by the fresh run,
assigns their maximum to `decision_one` and minimum to `decision_two`, and
asserts strict descending order. The private update, causal inspect/history,
one-effect and twice-repeated recovery assertions remain. No retained record
is removed and no uniqueness constraint changes. The two protocol tests now
have independently selectable names and one private common assertion helper;
neither failure can suppress the other test.

Owned `dwpg-r3` was requalified running on loopback 127.0.0.1:5432,
PostgreSQL 17.11, with the issued digest
`postgres@sha256:d74eeac9a635390a49bc21bd49fccd973de707e2a53a76ac49b552b8712ec46f`.
Immediately before creation the named database was absent; the unchanged
TTY-backed native bootstrap created `dw_native_lifecycle_readiness_20261024a`
with fresh restricted owners. The same database served both UUID regressions
and all three diagnostic runs. Windows x86_64 MSVC Rust 1.94.1,
cargo-llvm-cov 0.9.1 and the existing Python 3.12 installation were used.
One owner serialized targets and fixtures. No provider key was needed.
The bootstrap session ended normally after its action exits were inspected;
the database and reports remain retained, and its credentials are not recovered.

New action logs are under
`D:/DuskWeave-build/rust-1.94.1/reports/inference-lifecycle/readiness-root-cause`.
Ordinary actions used `D:/DuskWeave-build/rust-1.94.1/normal`. Each instrumented
action's Python subprocess set both `CARGO_TARGET_DIR` and
`CARGO_LLVM_COV_TARGET_DIR` to
`D:/DuskWeave-build/rust-1.94.1/coverage/inference-lifecycle` before Cargo.
The isolated instrumented workspace profiles were cleaned once with
`cargo +1.94.1 llvm-cov clean --workspace`; no `cargo clean` ran.

| Action log | Exit / executed result | Test duration |
| --- | --- | --- |
| `causal-ordinary.log` | 0; 1 PASS, 17 filtered, zero failed/ignored | 5.38 s |
| `causal-instrumented-exact.log` | 0; 1 PASS, 17 filtered, zero failed/ignored | 5.51 s |
| `observed-ordinary-responses.log` | 0; 1 PASS, 17 filtered, zero failed/ignored | 9.87 s |
| `observed-instrumented-responses.log` | 0; 1 PASS, 17 filtered, zero failed/ignored | 10.16 s |
| `observed-instrumented-chat.log` | 0; 1 PASS, 17 filtered, zero failed/ignored | 9.95 s |

An earlier `causal-instrumented.log` records exit 1: the initial llvm-cov
invocation put the filter before `--` and was rejected as an unrecognized
subcommand. No test ran; this is neither product RED nor a failed scenario.
The corrected invocation above is the single executed instrumented regression.
All three subsequent diagnostic cases executed once, with new run identities;
there was no readiness retry or complete Windows-suite rerun.

The exact Cargo action commands, run through the existing bootstrap session,
were:

```text
cargo +1.94.1 test --locked --test m1_inference_lifecycle t1_inspection_orders_decisions_causally_when_the_first_uuid_sorts_last -- --exact --test-threads=1 --nocapture
cargo +1.94.1 llvm-cov --locked --test m1_inference_lifecycle --no-report -- t1_inspection_orders_decisions_causally_when_the_first_uuid_sorts_last --exact --test-threads=1 --nocapture
cargo +1.94.1 test --locked --test m1_inference_lifecycle t1_responses_real_cli_completes_feedback_and_key_free_recovery -- --exact --test-threads=1 --nocapture
cargo +1.94.1 llvm-cov --locked --test m1_inference_lifecycle --no-report -- t1_responses_real_cli_completes_feedback_and_key_free_recovery --exact --test-threads=1 --nocapture
cargo +1.94.1 llvm-cov --locked --test m1_inference_lifecycle --no-report -- t1_chat_completions_real_cli_completes_feedback_and_key_free_recovery --exact --test-threads=1 --nocapture
```

For commands 3–5 only, their action subprocess set
`DW_TEST_READINESS_OBSERVE=1`; `run_support::command` still clears the child
environment and does not forward that test variable. The corrected descending
pairs and distinct run identities were:

| Mode / run | Decision one | Decision two |
| --- | --- | --- |
| Ordinary `2d4bfbb7-824c-433c-8043-4797584cb251` | `660a6539-3dc3-4475-8386-5e1bfd90bac4` | `427d551b-18bd-4662-99a2-81eb00839988` |
| Instrumented `d9bec730-6ca3-4bb8-8f12-3b767cf8a6c3` | `a259143c-5362-4de0-a206-3a118065e49f` | `4fa655a7-966f-4d0c-a995-f8361c6e64df` |

**Diagnostic B / observed versus inferred.** The opt-in support observer
preconnects one existing fixture-admin client, checks the selected database,
and samples only that database while excluding its own backend. Every query
uses a fresh read-only transaction with 250 ms statement, 50 ms lock and
1,000 ms idle-in-transaction timeouts. Sampling targets 100 ms, ends on CLI
exit or the 410 s fixture episode envelope, and is stopped/joined by RAII
before any CLI assertion can fail. Records contain only monotonic time, PID,
closed owner alias, state/wait labels and server-side startup-SQL classification.
The classification follows `autonomous_owners`, `owners`, `prepared_basis`,
`PgContextReader` and `PgRunStore`; SQL/binds/content never leave the server.
Before/after snapshots contain only run-scoped counts and existing closed
readiness diagnostics. No grant, server logging or profiler was added.

The first ordinary control exposed an observer defect: filtering on
`backend_type='client backend'` excluded other owners' masked statistics rows.
Its empty activity samples and `status=observed` are **INVALIDATED**, not
evidence of inactivity. The observer was corrected before either instrumented
case: retain masked rows and explicitly check existing stats visibility.
No ordinary run was repeated. Its CLI assertions and durable snapshots remain
valid control evidence, but it did not execute the final observer source.
The two instrumented cases executed the final support source; unchanged
ordinary causal assertions used the observer-disabled path.

| Diagnostic / run | CLI launch–exit from observer origin | Decision 1 / 2 readiness diagnostics | Observation availability |
| --- | --- | --- | --- |
| Ordinary Responses `d7a1230f-2c8a-4821-9a2a-56743a1d3b72` | 56–5,630 ms | READY received; child exit 0 at 1,499 / 1,756 ms | Activity samples invalidated as above |
| Instrumented Responses `01a1f847-c5cf-4e8c-b7bb-ffce746243dc` | 63–6,062 ms | READY received; child exit 0 at 1,585 / 1,942 ms | UNAVAILABLE: 55 of 56 samples contained masked backend fields |
| Instrumented Chat `e24a9939-84c1-4537-ada8-343594ad9191` | 57–5,954 ms | READY received; child exit 0 at 1,636 / 1,794 ms | UNAVAILABLE: 54 of 55 samples contained masked backend fields |

All before snapshots have no run and zero bindings/windows/reservations/calls/
decisions/history/effects. All after snapshots have two bindings, windows,
decision-specific reservations, calls, decisions and history rows, with one
synthetic DNS result. Common assertions additionally qualify bound protocol
bytes, feedback consumption, owner-qualified Terrain, real inspect and two
key-free recoveries preserving counts. Acquisition requires no READY frame;
its recorded `readiness_observed=false` is not an inference-startup failure.

Observer query wall totals were 388,880 / 471,769 / 465,426 microseconds for
52 / 56 / 55 queries; maxima were 16,154 / 20,836 / 13,125 microseconds.
Maximum sample gaps were 112 / 111 / 117 ms. Stop/join/report intervals were
13 / 21 / 15 ms; no query failed or observer envelope expired. These are
observer work measurements, not a measured slowdown relative to an unobserved
run. Instrumented samples expose PID/owner presence but mask SQL, state and
waits; `class=other` for a masked row identifies no SQL component. Parent and
child role sharing and intervals between samples remain unattributed.
Observed runs are diagnostics, not clean Windows qualification.

**Reconciled disposition (architecture review, 2026-10-24): historical cause
UNVERIFIED.** The original 5,068 ms incident is not explained by the selected
passing diagnostics. A subsequent bounded architecture probe on source
07348549179614055ec23ec088484346251421ea observed other restricted sessions
through existing provisioning access, without a new grant. Ordinary and
instrumented Responses passed once; decision-child lifetimes were 1.56-1.86 s,
not READY timestamps or a latency distribution. Deliberately pausing the owned
instrumented child after one binding/window produced parent-observed
readiness_deadline at 5,011 ms with zero decision reservation/call/decision/
history/effect. Thus the footprint alone cannot attribute a database stall.
The injected control proves its bounded stop, not the old physical cause.

Sanitized probe evidence is retained at
`D:/DuskWeave-build/rust-1.94.1/reports/inference-lifecycle/architect-startup-probe-20261024a/DW-PR116-WINDOWS-STARTUP-PROBE.md`.
The source worktree remained unchanged throughout those experiments.

The selected next scope is test/tooling closure: remove the Rust SQL sampler,
separate existing contract/upgrade/wire responsibilities, and reuse the native
Fixture in one fail-fast four-action Windows qualification process. Session
custody alone does not aggregate action failures; the qualifier must do so.
No startup checkpoint protocol, diagnostic-v3/schema, timer, privilege, ACL,
warm-up or Windows-runner change is justified by this evidence. The 5 s
preparation and 1.5 s ACL policies remain unchanged. This is a bounded re-scope
after the preserved cycle-2 STOP, not a counter reset or acceptance. Clean
Windows gates, candidate Linux CI and final review remain required; the owner
must not be told that all future Windows tail events are eliminated.

**Retained gates and delta review.** The earlier ordinary binary 57/57,
ordinary lifecycle 17/17, instrumented binary 57/57 and instrumented lifecycle
15 PASS / 2 FAIL remain source/environment-bound historical results; selected
passes above do not convert the last full gate to PASS. T8 ordinary/instrumented/
Linux upgrade evidence, 44/44 library, real-duration T2 and unchanged controls
remain retained. Linux 96.75% belongs to its recorded qualification source;
no new production-coverage percentage is claimed. Final committed-head Linux
full-suite/coverage and required checks must be read from the PR before any
later readiness claim. Local checks are working-source evidence.

Final source fmt, all-target Clippy with warnings denied, dependency direction,
links, structure and whitespace checks pass. Pinned rust-code-analysis-cli
0.0.25 against the original PR base reports zero regressions and ten retained
debt items (`readiness-root-cause/metrics`). This delta has zero production
changed lines. The earlier author review retained the observer in the existing test
support file. Architecture reconciliation rejects that cohesion disposition:
observation, migration custody and inference/wire qualification have distinct
consumers and reasons to change. The assigned closure will remove the sampler and
moves existing responsibilities without deleting assertions or count-driven
forwarding helpers. No dispatch exception is used.

Earlier author QUALITY_BAR section 7: Q1–Q4 PASS for test-only ownership, dependency
direction, necessary private observation and unchanged product truth/timers.
Q5 PASS for the bounded correction/matrix, while the original lifecycle
qualification remains blocked. Q6 PASS: split tests -> existing real `m1-run`
producer -> supervisor/inference/SQL owners -> both decisions/Terrain/history/
inspect; the optional observer is called only by that harness, with no new
production component or island. Q7 PASS for distinct retained per-run IDs,
unchanged recovery/effect assertions and observer shutdown on CLI failure.
Q8 UNVERIFIED for historical startup causation; the invalid ordinary activity
claim is explicitly withdrawn and masked instrumented fields are unavailable.
The existing distinct review and unchanged T8/control evidence remain retained;
no additional reviewer agent or independent-review claim is introduced here.

Delivery remains **DRAFT / NOT READY**. The known test correction is qualified
in its selected scope; original lifecycle delivery remains undelivered pending
causal disposition and clean mandatory gates. No new FIX cycle, automatic
retry, real provider/target request, merge, acceptance or seal is claimed.
