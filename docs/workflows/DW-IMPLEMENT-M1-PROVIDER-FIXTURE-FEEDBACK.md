# DW-IMPLEMENT-M1-PROVIDER-FIXTURE-FEEDBACK

PROJECT: DuskWeave
MODE: IMPLEMENT
WORKSPACE: `D:/DuskWeave-ci-complexity`
BRANCH: `feat/m1-provider-fixture-feedback`
BASE: `2b807d6961a6fd4cb0feafb690d178213e41fb2e`
Prepared: 2026-10-20, after verified merges #74 and #106.

Execute this packet directly. Do not return another implementation plan or a replacement architecture. Deliver one cohesive PR. Routine corrections stay in that PR.

## Outcome and authority

Extend the merged Walking Skeleton through **real LLM Decision 1 → Broker-admitted synthetic DNS → Terrain owner publication → fresh ContextPack → real LLM Decision 2 → durable history/inspect**, using the production `m1-run` supervisor and restricted PostgreSQL owners. Initial Mission, admitted seeds, scope, disclosure, window and finite budgets are supplied before start; there is no operator input, JSON editing, copied identity, `approve` or `continue` between phases.

This supplies the cognitive-feedback evidence needed for objective-led authorized adversary emulation and evidence-led adaptation. APT41, Lazarus and Volt Typhoon remain behavioral reference lenses, not playbooks, equivalence claims or execution permission. DNS is a bounded hypothesis-testing action; the controller must preserve the LLM's genuine retain/defer/stop/view-request/capability-gap choices. A synthetic result establishes no live target fact. Two decisions do not complete or seal M1.

Authority is the accepted text of **M1-enabling-architecture §7 and ADR-016 §4a at `0615fb72c946fd6abf0823692be621096091377c`**, existing §§1–6, PRD-000 invariants, applicable ADR-004/005/011/015/016, roadmap §§1.2–1.4, AGENTS, QUALITY_BAR and build-order packets 00/07. Read in repository authority order. Do not modify accepted authority or seals.

**Provider contact is conditional:** complete the offline inference-boundary qualification below on the actual deployment first, then require fresh, explicit, scoped v3 provider disclosure. This packet authorizes implementation and isolated tests; it is not itself disclosure consent, acceptance of provider terms, a target authorization or evidence that runtime qualification already passed. If fresh consent/key is absent, finish the offline implementation and deliver the PR with the real episode explicitly NOT EXECUTED. Do not fabricate consent or substitute an old episode.

## Verified baseline and preflight

- #74 merged at `94c240be0f5b4e6d71d1f19b52b1d03da17a93d7`; #106 reconciled tracking at BASE. #74 final reviewed head was `059a34b33ea2d9bb05a46ef3f32541e79738a90e`. Only `docs/ENGINEERING_STATE.md` differs between that head and BASE.
- At preparation, the selected Windows worktree is clean on `feat/m1-autonomous-skeleton` at `059a34b`; `origin/master` is BASE. Preserve this branch and all unrelated worktrees. Recheck identity, HEAD, status and worktrees, then create the named new branch from BASE in the clean selected worktree. If the new branch already exists, inspect its ancestry/work before reuse; do not reset it. A newer master requires a scoped base/delta check, not silent substitution of BASE.
- #74 final-head run `39400100099` passed its required checks, including coverage (96.93% total, minimum file 85.38%). BASE's `doc-check` run `39407695637` is successful; this is not a fresh Rust run. New runtime changes require their own exact-head CI.
- The existing normal target is `D:/DuskWeave-build/rust-1.94.1/normal`; reserve `D:/DuskWeave-build/rust-1.94.1/coverage/provider-fixture-feedback` for instrumentation and `D:/DuskWeave-build/rust-1.94.1/reports/provider-fixture-feedback` for sanitized evidence. One verification owner holds each target through builds, tests and CLI subprocess use. No concurrent build/clean from another worktree, no routine `cargo clean`, no global environment changes.
- Reinspect and reuse owned `dwpg-r3`, PostgreSQL 17, image `postgres@sha256:d74eeac9a635390a49bc21bd49fccd973de707e2a53a76ac49b552b8712ec46f`, local loopback port 5432. It was running at preparation. Do not recreate it because of its name, inspect container environment, or delete databases/roles/volumes.
- Use the existing bootstrap to provision fresh disposable `dw_native_provider_fixture_20261020a` and unique generated restricted logins. A collision refuses; do not reuse that database as fresh evidence. A later genuinely necessary rerun may use the helper's generated unique `dw_native_` name and record it. Admin is provisioning/fault injection only; runtime remains separated core/Broker/Cognition/Terrain/Trajectory principals, without SUPERUSER/CREATEROLE/BYPASSRLS. Serialize cluster-role fixtures.
- Qualified baseline toolchain is Windows x86_64 MSVC Rust 1.94.1. Record the actual linked libcurl/TLS/resolver backend in the boundary tests; a system `curl --version` is not evidence about the Rust-linked library. Use `C:/Program Files/Git/usr/bin/openssl.exe` for existing local TLS tests and the resolved existing Python executable; record availability without installing or changing the host.
- Never access, copy, stage, modify or delete `.cargo/config.toml`. Do not print `.env`, DSNs, authorization headers or API keys. Existing authorized secret configuration may be used only inside its process/credential boundary.

## Exact editable map

Each listed existing file is editable only for this outcome or its directly touched debt. New paths are deliberately specified. Do not widen this list by glob.

| Responsibility | Editable files |
| --- | --- |
| Packet, tracking and operator instructions | `docs/workflows/DW-IMPLEMENT-M1-PROVIDER-FIXTURE-FEEDBACK.md` (new; retain this packet and bounded execution evidence), `docs/ENGINEERING_STATE.md`, `docs/BUILD_ORDER.md`, `docs/MVP_AND_DEFERRED_SCOPE.md`, `docs/RUN_M1_AUTONOMOUS_FIXTURE.md` |
| Forward schema change | `migrations/0019_provider_fixture_feedback.sql` (new) |
| Run/profile composition and stopping | `src/main.rs`, `src/m1_run.rs`, `src/m1_run_cli.rs`, `src/m1_run_inference.rs`, `src/m1_run_supervision.rs`, `src/m1_run_effect.rs`, `src/m1_run_control.rs` (new), `src/m1_inference_control.rs` (new), `src/m1_reasoning_cli.rs` |
| Inference transport | `src/m1_reasoning/mod.rs`, `src/m1_reasoning/transport.rs`, `src/m1_reasoning/transport_control.rs` (new), `src/m1_reasoning/postgres.rs` |
| Issued Terrain binding and premise-change signal | `src/m1_terrain_issued.rs`, `src/m1_terrain_cli.rs` |
| Profile, wiring and failure tests | `tests/m1_run.rs`, `tests/m1_run_cli.rs`, `tests/m1_run_postgres.rs`, `tests/m1_run_failures.rs`, `tests/m1_autonomous_wire.rs`, `tests/m1_provider_fixture.rs` (new) |
| Concrete transport/fixture support | `tests/support/m1_native_transport.rs`, `tests/support/m1_autonomous_transport.rs` (new), `tests/support/m1_run.rs`, `tests/support/m1_native_bootstrap.py`, `tests/test_m1_native_helpers.py` |

Read-only references include AGENTS, QUALITY_BAR, PRDs/ADRs/contracts and the six-case wire fixture, migrations 0001–0018, `src/m1_context.rs`, `src/m1_autonomous_context.rs`, `src/m1_context_capacity.rs`, `src/m1_reasoning/{inference,decision_attempt,proposal,request,request_autonomous,response,postgres_terrain}.rs`, `src/database_{config,wait}.rs`, `src/postgres_m1_run.rs`, `src/postgres_m1_terrain.rs`, `src/postgres_terrain_view.rs`, the existing native/permission/continuation tests, historical qualification documents/helpers, dependency and complexity gates, and CI workflows. These paths already contain reusable behavior; an unchanged caller does not need an edit.

No dependency, Cargo, CI, accepted-contract or Linux host changes are planned. No Go adapter, capture/exporter, real DNS socket, host firewall, generic scheduler, qualification registry or new approval tier. If an unexpected required consumer is genuinely missing from this map, report its exact path and reason before expanding scope.

Tracking disposition: **UPDATE** ENGINEERING_STATE current phase/active packet/next action; reconcile actual #74/#106 merges without embedding this candidate's own SHA or claiming its future merge. BUILD_ORDER changes only navigation to this active packet. Update the existing roadmap's slice/debt disposition without creating another roadmap. Preserve all historical failures, Foundation/M0 seals and the owner acceptance record. Use the existing runtime guide for the provider-fixture instructions; do not create another one-use demo runner.

## Decisions to implement

### 1. Select one closed provider-fixture lane

Keep the deterministic fixture lane intact. Add runtime support for exactly this already accepted tuple:

| Binding | Value |
| --- | --- |
| Run profile / execution mode | `autonomous-dns-provider-fixture-v1` / `controlled_fixture` |
| Inference recipient / profile | `https://api.deepseek.com/responses` / `deepseek-responses-autonomous-v1` |
| Request settings | Existing `request_autonomous.rs`: `deepseek-flash`, reasoning effort `high`, text-only, non-streaming, no tools, max output tokens 4096; preserve its omissions and schema |
| Effect / transport | `dns-routing-fixture-v1` / synthetic UDP-labelled result, with **no target DNS socket** |
| Terrain source | `fixture_dns_acquisition`; never `issued_dns_acquisition` or `operator_reviewed_dns` |
| Permission | Fresh `m1_inference_permission_v3`, exact run/profile/mode/recipient, explicit terms approval and non-nil current terms reference |

Do not enable `autonomous-dns-v1` runtime, switch an existing campaign/profile, upgrade v1/v2 consent, fall back to a different endpoint/model/API, or silently select `DeterministicProvider` if real inference fails. A provider refusal, unavailable model, incompatible envelope or expired permission is a bounded failed/deferred/UNKNOWN outcome, not permission to change the contract.

Dispatch the provider in the existing autonomous composition using admitted run/profile data. Reuse the real ContextPack, reservation, proposal admission, decision store and history use case. Keep the deterministic provider isolated to its own tuple. Rename the run-scoped basis coherently if its old `FixtureBasis` name becomes misleading. Do not duplicate the inference engine.

Pass `DEEPSEEK_API_KEY` explicitly to **inference children only**, after validating the admitted provider-fixture tuple. Acquisition children receive no provider key; deterministic children, inspect and recovery require none. Preserve `env_clear`, fixed executable/commands and the infrastructure allowlist. Do not forward PATH, proxy variables, arbitrary endpoint flags or the whole parent environment. The key is never a Mission field, ContextPack field, request argument, control message, durable record or report value.

### 2. Forward-only SQL and exact issued-source identity

Add migration 0019. Do not edit merged migrations 0016–0018. Carry forward their current fencing fixes rather than copying the pre-fix skeleton.

Extend these existing functions in the new migration where required: `execution.run_current`, `execution.run_start`, `execution.assert_autonomous_pack`, `execution.run_summary`, `terrain.admit_dns_request`, `terrain.issued_dns_request`. Existing `inference_profile_matches`, reservation/window/may-send functions already admit v3 semantics and should be reused unless an actual failing assertion establishes a further necessary correction inside this new migration.

- Bind request, current registered permission, immutable pack and configuration to the **same** tuple. Both controlled-fixture profiles are allowed, each with its own exact inference recipient/provider. Live remains refused by run admission. Preserve writer identity, registration-history qualification, nine-class disclosure, time and budget checks.
- Keep the campaign advisory fence and `session_fences.epoch` ordering before mutable admission/control writes. Do not regress the fixed READ COMMITTED/SERIALIZABLE release/commit/stop races from #74. Every updated writer must reevaluate current state after its fence, with no stale pre-lock authority snapshot.
- Derive run-summary profile/mode from its immutable admitted run; remove the existing hard-coded deterministic profile there. Report provider/inference and acquisition/Terrain outcomes separately using existing durable records. A provider failure after a committed fixture result cannot relabel that result as acquisition failure.
- Change the typed `IssuedDnsReference` construction/validation and `m1_run_effect` caller together: carry the actual admitted controlled-fixture run profile. Retain the existing deterministic constructor/behavior where needed by consumers.
- Terrain independently verifies that the reference tuple exactly equals the stored run tuple, campaign/result, attempt, session generation, release, fixture profile/mode and publication obligation. Do not merely add another accepted string. Strengthen the issued-result lookup's request-profile equality while extending it. Cross-profile references to otherwise valid records must reject.
- Preserve original result perception time, TTL and source through owner publication, dependency checks, fresh pack, reasoning provenance and inspection. Existing mode-aware Terrain view behavior already supports controlled fixtures; test that consumer rather than rewriting it gratuitously.
- Apply 0019 after 0018 in both native bootstrap initial/refresh lists and `tests/support/m1_run.rs::refresh`. Update helper assertions for **17 initial / 8 refresh** migrations and their order. A refresh must not leave older function definitions installed by another test target. Exercise fresh installation and the actual fixture refresh sequence; repeatable setup is not permission to mutate a retained client database.

### 3. Qualify cancellation before using the real provider

The existing native transport calls synchronous `basis.current_authority_and_premises()` from curl's progress callback. It also receives the whole shared deadline as its transfer budget. Neither qualifies the accepted autonomous boundary. Implement this lane's control separately while sharing reusable HTTP/TLS/response mechanics with legacy transport.

Use three narrow pieces, not a campaign manager:

1. `m1_inference_control.rs`: a run/decision-scoped authority coordinator with its **own restricted Broker connections/basis**. At most one current Mission/run/premise check is outstanding. Start renewals at most 500 ms apart (use the existing 400 ms cadence). A successful result may publish only a permit ending no later than **check-start + 2000 ms**, further clamped by all original deadlines/authority/premise validity. Late, failed or expired checks cannot renew. Check retained Terrain dependencies as well as Mission and run status, including original fixture expiry. Preopen required connections within preparation; do not reconnect indefinitely during a transfer.
2. `transport_control.rs`: a small transport-side latch/monotonic permit view. Curl's autonomous progress callback reads **only** the latch and monotonic expiry. It performs no SQL, mutex wait, filesystem, history, pipe read or owner callback. Response admission still performs the existing fresh authority/reference checks outside curl callbacks; a permit never makes a reference eligible.
3. Existing phase supervision plus a database-independent transport/permit deadline guard: cooperative abort first, independent hard stop when it stalls, child reaped by its parent when present. Retain the legacy watchdog behavior for legacy commands; do not delete `process::exit(124)` protection without a tested replacement. A parent blocked in SQL or killed must not leave autonomous network I/O waiting until the 50-second phase ceiling after its permit expires. The independent guard must not wait for a coordinator join, PostgreSQL response, stdout lock or callback. A hard exit leaves charged uncertainty and possibly pending publication.

The fresh release fence remains mandatory **for generation and repair**. Keep one prepared, one-use worker handoff ordered under the campaign fence, followed promptly by lock release. No transaction/session lock spans a network wait. The worker may be prepared/parked before this handoff but may not contact DNS/TLS/HTTP before fresh admission. Do not replace the final check with a cached permit or treat unlock acknowledgement as proof that a send did or did not occur. Preserve the existing actual-send/unlock-failure negative control.

### 4. Use a real local stop channel, including before slow SQL

Implement a deliberately small same-host cancellation channel in `m1_run_control.rs`, not a message bus. Select **fixed, bounded parent-to-child stdin control/EOF plus write-once local stop markers** for external commands. This uses the standard library and avoids a new dependency. These signals may only revoke local work; they never grant execution, establish a committed withdrawal, or replace the durable SQL fence.

- Provision one private, operator-owned control root, `D:/DuskWeave-build/rust-1.94.1/provider-fixture-control`, before any provider run. `DW_M1_RUN_CONTROL_ROOT` is trusted process configuration, forwarded explicitly to the fixed children; it is not request JSON or an arbitrary input-selected path. Offline tests use their own private temporary root. Qualify root ownership/access, path safety and bounded I/O on the actual host; absent/unqualified control refuses the provider lane. Do not change machine-wide policy or ACLs outside the owned root.
- Scope fixed marker locations by validated engagement/campaign UUIDs and run UUID. Markers are monotonic STOP intents, bounded in size, contain no secret/payload and cannot be cleared to resume that run. A run marker can be addressed by `m1-run-stop --run` without a preceding database lookup; campaign markers let withdrawal/premise writers cancel before database waits. Unknown/malformed bindings never enable work. No directory traversal, arbitrary filename, symlink/reparse redirection or positive permit is accepted from this channel.
- Parent owns the fixed inference child's stdin pipe. A dedicated child reader handles STOP or EOF without SQL. Unexpected EOF/parent loss latches stop. The supervisor's independent monitor observes scoped markers even while its normal orchestration thread is blocked in SQL, stops the child and reaps it. The child's permit watchdog remains the fallback if any control worker or file I/O stalls; no slow control operation may renew a permit.
- `m1-run-stop` signals the run before opening/waiting on its SQL path. The production `withdraw` command signals the parsed campaign before opening/waiting on its withdrawal SQL path. A manual Terrain record that can replace an in-use premise signals that campaign before its mutation/lock waits. The normal run's own fixture Terrain intake occurs only after inference is closed and must not accidentally stop the next phase. Inspection/history-only recovery does not emit a new stop intent.
- Preserve authority semantics: a local cancellation intent is not a durable SQL withdrawal or authenticated owner mutation. A malformed/refused writer may conservatively cancel local activity but cannot commit another principal's authority state. Report control failure honestly and still attempt the authoritative withdrawal/stop under existing bounded waits; do not lose withdrawal merely because signaling failed. Never continue effects while this disposition is unresolved.
- Restricted direct SQL writers retain their mandatory common fence. Tests of those writers must also prove permit expiry/authority loss stops owned I/O without assuming that an unrelated process can share an AtomicBool. Only production entrypoints in this single-host lane claim the pre-SQL local signal path; no distributed-writer or target-host STOP qualification is claimed.

Keep this channel scoped to active provider-fixture work. Absence of an active control binding must not make legacy M0/v1/v2/deterministic commands require a new control configuration. A new provider run cannot start without its qualified binding. Preserve terminal markers and never treat a stale marker's removal as restart authorization; inspect/recovery remains database-only.

### 5. Apply the accepted clocks, not a global timeout increase

| Clock/capacity | Required behavior |
| --- | --- |
| Preparation / supervised inference phase | At most 5000 ms preparation; phase at most 50000 ms from launch, clamped to original run remainder |
| Each provider call | At most 20000 ms including connection establishment, itself at most 5000 ms; clamp again to shared/authority/permit/phase/run remainder |
| Generation plus conditional repair | One original 40000 ms active clock including local admission and validation; no reset for repair |
| Decision reservation | 45000 ms = 40000 active + up to 2000 cancellation + 3000 terminal accounting; no effects during cancellation/accounting |
| Run / acquisition | At most 150000 ms total; fixture acquisition/owner phase retains the 60000 ms ceiling, always clamped to actual remainder |
| Run capacity | Reserve capacity for four call slots, 65536 request bytes, 262144 response bytes, 16384 output-token capacity and 90000 inference wall-time ms |

Deadline/grace never extends permission to send. Escalate by the earliest applicable authority/permit/call/shared/decision/phase/run bound, allowing less than the nominal grace when necessary. An exhausted accounting reserve means pending/UNKNOWN, not guaranteed successful cleanup. Parent monitoring and coordinator checks use bounded waits; no growing thread-per-renewal pattern or unbounded blocking join.

Keep the existing 20/30-second legacy behavior. Do not infer runtime clocks solely from descriptive configuration JSON. Prove the actual autonomous libcurl transfer timeout is <=20 seconds even though its decision has 40 seconds of active budget. The accepted endpoint's provider-name DNS resolution is infrastructure traffic; target names from seeds/LLM prose must never reach a resolver/socket in this fixture lane. Qualify the linked resolver/backend and a stalled-resolution/connection case, or its independently enforced process-stop fallback, on Windows. Linux CI evidence alone is insufficient for a Windows timing claim.

### 6. Preserve wire fit, causality and honest cognitive outcomes

Reuse the existing Rust `m1_context::prepare_autonomous` → `request::bind` → `verify_binding` → SQL `assert_autonomous_pack`/`inference_may_send` path. It already checks full generation and worst repair fit during pack construction; do not build a second serializer, widen limits or change the accepted prompt just to force DNS.

The pack is <=8192 UTF-8 bytes, configuration and complete escaped request each <=16384; response aggregate <=65536, proposal <=8192, each text field <=1024, repair material <=2048, with existing depth/collection and enum limits. Preserve `dw-inference-binding-v1` length-prefixed P/C/B SHA-256 and exact input/pack identity for generation and repair. No raw response, private reasoning, provider error body or arbitrary campaign history may enter ordinary storage, control pipes or reports.

For Decision 2, get a new owner-qualified Terrain view and the bounded committed predecessor, retaining selected seed, actual first decision/event, attempt/result/Terrain-publication identities and source/mode/time limits. The optional hypothesis summary remains <=128 UTF-8 bytes and may be omitted; the current eligible Terrain premise cannot be displaced. Expiry stays `min(perceived_at + TTL, perceived_at + 60s)`; do not refresh timestamps, replay DNS, or prompt an operator to repair the chain.

Success of the **real feedback demonstration** requires an admitted first real proposal choosing the qualified fixture DNS action, its committed owner outcome/history, and a second actual provider-generated admitted proposal whose references resolve to the first decision and the selected owner-qualified fixture Terrain evidence, with a safe explanation of retention/revision and fixture uncertainty. Show those references and bounded explanation for review; two HTTP responses or two decision rows alone do not prove this.

Do not rewrite a valid retain/defer/stop/view request into DNS, script the second answer, or reject a legally different strategy solely to manufacture demo success. Such a run remains an honestly reported terminal outcome; feedback demonstration is NOT DEMONSTRATED if the required evidence was not consumed. A second proposed effect is recorded but not executed in this two-decision/one-attempt profile. No automatic paid restart, reseeding, repair outside the original allowance or deadline extension.

The existing `completed_charged_unused_repair_reserved` consumption string is coarse and can also appear after repair. Do not interpret its name as proof that repair was unused: inspect the durably bound generation/repair call identities and states when attributing calls, usage and failure phases. Preserve compatibility unless a necessary, tested new-lane disposition is introduced in the forward migration and decoder together.

## Required qualification and acceptance evidence

Use TDD for new behavior and genuine regression assertions for defects. Reuse passing baseline characterization for unchanged behavior. Test fixtures are isolated and deterministic; synchronize races through barriers/observed milestones, not arbitrary sleeps deciding the winner. Bounded elapsed-time assertions test timing mechanisms and report actual observed margins.

| Boundary | Assertions before provider contact |
| --- | --- |
| Closed profile and disclosure | Through real registration/parse/SQL/run admission: valid provider-fixture tuple succeeds; v1/v2, deterministic consent, missing feedback/repair class, missing terms approval/ref, wrong recipient/profile/mode, expired/revoked permission and insufficient cumulative capacity refuse before provider preparation/release. Exercise restricted raw SQL as well as Rust. No real key or provider network is used. |
| Native entrypoint wiring | Existing real `m1-run` deterministic two-decision CLI regression still passes. A provider-fixture CLI start reaches the real selected provider boundary and refuses safely when the key/control qualification is missing, without fixture fallback. Inject provider factories only at private test composition points; no production endpoint override. Real-provider success must later use the normal compiled binary. |
| Actual local transport | Extend private adapter tests using the real autonomous configuration/body and loopback HTTP/TLS endpoints, synthetic token only. Verify exact received request bytes/binding; valid TLS vs untrusted CA/wrong hostname; no redirect/proxy/retry/compression; aggregate header+body overflow; malformed/refusal/incomplete responses; key reflection; and no late unsafe publication. Test-only endpoint/CA construction stays private and unavailable in the production factory. |
| Release/withdrawal races | Stop/withdrawal before release gives zero accepted endpoint requests. Release ordered first may have sent and remains charged/uncertain on cancellation/unlock failure. Cover generation and conditional repair, with a positive control proving the unrejected request would reach the endpoint. Preserve #74's fenced reserve/commit/terminal race regressions under READ COMMITTED and SERIALIZABLE. |
| Nonblocking cancellation | An authority/premise SQL check that blocks cannot block the progress callback or renew a stale permit. Check-start anchoring, <=500 ms renewal starts, <=2000 ms lease, one outstanding check, stale/late success, blocked local control I/O and no cached-permit release all have negative controls. Stop from a separate process must affect the actual owned transport. |
| Stop, parent loss and deadlines | Block withdrawal/stop SQL deliberately and observe the local signal first. Test idle HTTP, stalled TLS/connect/resolver or enforced resolver fallback, stalled cooperative abort, parent hard loss and coordinator loss. Assert socket closure/process exit/reaping within original bounds and no late request/publication/effect, rather than merely asserting a flag was set. The independent stop must work while normal parent SQL is blocked. Record actual linked backend and Windows observations; don't relabel generic timeout 124 as a known killer without evidence. |
| Real producer → consumer | Real Rust autonomous pack and issued-result producers flow through bound request verification/SQL and Terrain consumer, for one/eight seeds, quote escaping, 128-byte UTF-8 summary, long legal identifiers, original TTL and smaller legal consent limits. Cover exact-limit/+1 at affected byte boundaries, fit omission, minimum-fit failure before reservation/send, and stale/conflicted dependency refusal. Static independent snapshots alone do not satisfy causal linkage. |
| Identity/source integrity | Cross-campaign/result/run-profile/mode/release/attempt substitutions, caller-made issued JSON/source labels, fixture→live promotion and reused first pack reject. Exact stored run tuple must match the issued Terrain reference, not just belong to an allowed set. |
| Recovery and accounting | Commit-before-ACK loss, history failure, accounting failure, child hard exit, repeated start/inspect/recover and restart preserve original identities and charged unknown capacity. Recovery has at most three DB-only delivery tries, constructs no provider, sends no unused repair and releases no new DNS result. Required unresolved obligations prevent Decision 2. |
| LLM strategy freedom | Retain/defer/stop/view request/capability gap remain visible, no forced effect. Causal demo assertions distinguish real linked fixture feedback from legal terminal alternatives and scripted offline fixtures. Further proposed effects are not executed. |

The local TLS tests, restricted-owner composition tests and actual CLI/supervisor tests are complementary evidence. Label their injection boundaries. Do not call a test-only factory a complete real-provider CLI run or represent a deterministic endpoint response as real cognition. Before contact, record the selected candidate's offline boundary evidence in this packet/PR and provide the fully bound initial disclosure configuration for the operator to review. No additional governance artifact or blanket approval ritual is needed.

### Existing measurements reused at preparation

Read-only Windows probe on `059a34b` (runtime source identical to BASE): `cargo test --locked --test m1_autonomous_wire -- --test-threads=1` passed **2/2**, test execution **0.06 s**, cached build **0.68 s**. Independent recomputation from BASE verified all six fixtures' generation/repair bindings (12 digests) and stored byte lengths:

| Existing case | P | C | Generation B | Repair B |
| --- | ---: | ---: | ---: | ---: |
| Decision 1 minimum | 3300 | 5503 | 8078 | 8298 |
| Decision 2 minimum | 5131 | 5759 | 10329 | 10549 |
| Decision 2 eight candidates | 7581 | 6319 | 13703 | 13923 |
| Decision 2 quote/width | 7831 | 6319 | 14209 | 14429 |
| Decision 2 UTF-8 width | 7703 | 6319 | 13825 | 14045 |
| Provider-fixture quote/width | 7900 | 6350 | 14278 | 14498 |

Largest tested pack margin is **292 bytes**; largest complete repair-request margin is **1886 bytes**. These are existing synthetic serialization measurements and consumer checks, not runtime provider qualification, linked real-cognition evidence or latency percentiles. The implementation must measure its real producer output and new boundary; no provider or target was contacted while preparing this packet.

## Local verification, CI and review

Set `CARGO_TARGET_DIR` in the owning PowerShell process only. Run focused local checks in this order: formatting and Clippy; non-database profile/wire/transport/control tests; then the focused native bootstrap session; finally document/dependency/structure/diff checks and the current McCabe ratchet. Name test binaries explicitly; do not launch a blanket local full suite or parallel database role fixtures.

Suggested focused native session, using the existing bootstrap's JSON stdin action protocol so all checks use the same freshly provisioned principals:

```powershell
$env:CARGO_TARGET_DIR = 'D:/DuskWeave-build/rust-1.94.1/normal'
python tests/support/m1_native_bootstrap.py --container dwpg-r3 --database fixture --database-name dw_native_provider_fixture_20261020a --report D:/DuskWeave-build/rust-1.94.1/reports/provider-fixture-feedback --session
```

Send sequential actions through that process's stdin; do not print or extract its credentials:

```json
{"label":"provider-fixture-db","args":["cargo","test","--locked","--test","m1_provider_fixture","--","--test-threads=1"]}
{"label":"run-regressions","args":["cargo","test","--locked","--test","m1_run_postgres","--test","m1_run_failures","--","--test-threads=1"]}
{"label":"native-release-regressions","args":["cargo","test","--locked","--test","m1_native_release","--test","m1_native_continuation","--test","m1_native_admission","--","--test-threads=1"]}
{"exit":true}
```

Tests that requalify older SQL definitions must restore this candidate's forward migration before the next provider-fixture assertion. Use the existing fixture approach; do not add another fixture manager. Supply synthetic keys only inside private test construction and exclude real provider credentials from every automated check. Concrete TLS/control tests needing an environment prerequisite must fail with the prerequisite named rather than silently return/skip while being claimed as PASS.

Run helper unit tests (`test_m1_native_helpers.py`) and existing script checks for links, structure, dependency direction, conflict markers and `git diff --check`. Use existing test filters for transport/supervision rather than rerunning unrelated suites. Required final-head GitHub checks, including the **full suite once under coverage**, remain mandatory: 90% total / 80% per-production-file floors and current McCabe ratchet. Do not change workflows, parallelism, coverage floors or native-local privileges to make this packet green.

Local Windows full legacy-suite execution is **not** a delivery gate: QUALITY_BAR §3 and RUN_M1_NATIVE_INFERENCE explicitly assign the full suite/coverage to isolated CI and focused native tests locally. Report platform coverage limits honestly without turning legacy NOCREATEROLE fixture incompatibility into a new blocker or granting CREATEROLE to runtime/native-local principals.

Perform one adversarial review under QUALITY_BAR §7 after the complete candidate, with evidence for touched boundaries and explicit N/A limits. Correct valid in-scope findings and check the affected final delta; reuse unchanged baseline evidence. At most two FIX cycles for this unchanged outcome before architecture root-cause review. Don't repeatedly request owner acceptance of unchanged already accepted §7/§4a text. Fresh provider disclosure is a distinct operational permission, not acceptance of new architecture.

## Real episode, only after offline qualification and fresh disclosure

Keep provider execution opt-in and outside automated CI. First finish build/qualification and review the initial input, provider terms reference/approval, endpoint/profile, safe classes, budgets and time window. An existing API key or accepted architecture is insufficient. Never send before those prerequisites pass on the actual candidate/platform.

Use non-sensitive synthetic Mission/seed material and a new campaign with the exact provider-fixture v3 permission. Reuse the existing supported registration entrypoint and restricted owners; setup completes before short-lived result freshness begins. Use the normal production binary and **one `m1-run --input` start**. The application performs both real inference decisions and the fixture effect/owner intake; no historical Python helper stitches phases together. No automatic paid rerun if the LLM chooses a different valid intent or the provider times out.

Record only safe scoped identities, actual call states/requested-vs-reported settings, existing P/C/B binding sizes, first safe proposal, fixture outcome/source/original perception/expiry, second safe proposal/references, durable history and terminal/UNKNOWN disposition. Show that the second pack differs and resolves to the actual first decision and issued fixture outcome. Inspect/recover afterward without the key and prove zero additional provider/effect requests. Do not store raw response/private reasoning or imply provider-side cancellation/data recall.

If consent or qualification is unavailable, report **IMPLEMENTED/OFFLINE QUALIFICATION status as measured; REAL PROVIDER EPISODE NOT EXECUTED**, naming the missing prerequisite. If an episode starts and fails, retain its historical identities/charges and honest failure, not a fresh successful-looking replacement. A successful real-provider fixture demo remains neither live DNS nor M1 seal readiness.

## Debt, delivery and STOP conditions

Repay debt only in the touched responsibility. Deduplicate the common native/autonomous inference composition and shared transport mechanics; remove obsolete fixture-only assumptions in the changed run/Terrain reference path. If `m1_terrain_cli::command` or its ambiguous closure changes for the pre-mutation signal, characterize existing behavior and factor the mixed parsing/control/wiring responsibility so changed functions meet the current McCabe rule. Leave `connect_qualified`, `run_record` and the other untouched roadmap hotspots explicitly OPEN. No blanket deletion of legacy/M0/manual commands or old migrations; their production consumers remain supported.

Estimate, not a cap: this is a boundary + vertical integration delivery, likely roughly **1,500–3,000 changed runtime/SQL lines**, plus tests/docs, depending on reuse. Existing provider HTTP parsing, wire serialization, owners, synthetic DNS and Terrain reconciliation are reused. Measure actual diff, LOC/function spans and complexity. >400/600 numerical review references are advisory; review cohesion or refactor within this PR, do not weaken behavior/formatting to lower a count. New/changed functions satisfy the enforced McCabe ratchet (normally <=7; only repository-qualified dispatch allowances apply).

Deliver the PR with exact base/head, affected tests/platforms and boundary evidence, current CI, remaining debt, and the real-episode disposition. Give the owner a short plain-language explanation: what Decision 1 chose, what the synthetic observation means, how Decision 2 used it, what stopping/recovery proved, and what remains unproved. Counts/green badges alone are insufficient.

STOP for a genuine invariant violation, missing accepted dependency, unresolved authority, unsafe/unqualified boundary before external contact, material ownership/file-map expansion, or unrelated work that cannot be preserved. Missing fresh disclosure blocks external execution only; continue authorized implementation/offline qualification. No STOP for advisory LOC/diff numbers, a routine failing test, or an unrelated local legacy full-suite issue. Do not broaden scope into live target acquisition or continue into the next delivery slice.

## Measured offline qualification and initial disclosure review record (2026-10-20)

### 1. Candidate and platform baseline

- **Workspace / branch**: `D:/DuskWeave-ci-complexity`, branch `feat/m1-provider-fixture-feedback`, based on `2b807d6961a6fd4cb0feafb690d178213e41fb2e`.
- **Toolchain**: Windows `x86_64-pc-windows-msvc`, `rustc 1.94.1 (e408947bf 2026-03-25)`, `cargo 1.94.1`, `rust-code-analysis-cli 0.0.25`.
- **Linked transport backend (measured inside `m1_reasoning::transport::autonomous_tests`)**: vendored `libcurl 8.22.0-DEV` (`curl 0.4.50` / `curl-sys 0.4.91+curl-8.22.0`), TLS `Schannel`, `c-ares=None`, `feature_async_dns=true`, `feature_ssl=true`, `feature_http2=false`.
- **Local service & disposable databases**: reused running `dwpg-r3` (`postgres@sha256:d74eeac9a635390a49bc21bd49fccd973de707e2a53a76ac49b552b8712ec46f`, PostgreSQL 17.11 on `127.0.0.1:5432`). Initial non-interactive session provisioned `dw_native_provider_fixture_20261020a` before stdin EOF; preserved without deletion or reuse. Subsequent piped verification runs used fresh generated `dw_native_fixture_*` databases (`04cabbb06bcae2b1`, `8bc6165b3d804ed1`, `ece226da44a8bd51`, `7d0afc7c87f535b1`, `d03fb07d5e9a0726`).

### 2. Real producer wire measurements (`m1_provider_fixture`)

Measured from `m1_context::prepare_autonomous` → `request::bind` → `request::verify_binding` → `execution.inference_may_send`:

| Producer case | Decision | Pack `P` (<=8192) | Config `C` (<=16384) | Generation `B` (<=16384) | Max 8-code Repair `B` (<=16384) |
| --- | --- | ---: | ---: | ---: | ---: |
| 1-seed provider-fixture | Decision 1 (absent predecessor) | 3510 | 5534 | 8304 | 8611 |
| 1-seed provider-fixture | Decision 2 (committed fixture feedback) | 5016 | 5790 | 10210 | 10517 |
| 8-seed provider-fixture | Decision 1 (absent predecessor) | 5960 | 6094 | 11678 | 11985 |
| 8-seed provider-fixture | Decision 2 (committed fixture feedback) | 7466 | 6350 | 13584 | 13891 |

### 3. Local control, cancellation and timing observations (`m1_autonomous_transport`, `m1_inference_control`, `m1_provider_fixture`)

- **Progress callback isolation**: `generation_and_repair_post_exact_autonomous_bytes_with_no_progress_owner_calls` verified `0` owner/SQL calls inside curl progress callbacks; `1` fresh owner check occurs only at response admission outside curl callbacks.
- **Check-start lease & single outstanding check**: `one_blocked_check_cannot_block_permit_reads_or_publish_a_late_success` verified 400 ms renewal start, nonblocking callback reads (<50 ms for 1,000 reads while renewal SQL is parked), lease expiry <=2,000 ms from check start, `1` outstanding check (no thread-per-renewal), and rejection of late check completion after expiry.
- **Owned socket closure and process reaping (`actual_control_eof_marker_and_coordinator_loss_close_owned_idle_io_and_reap`)**:
  - `parent_loss` (stdin EOF on active HTTP): socket closed and child reaped in `1011 ms` (bound `<= 2300 ms`).
  - `tls_parent_loss` (stdin EOF during stalled TLS handshake after ClientHello `0x16`): socket closed and child reaped in `1957 ms` (bound `<= 2300 ms`).
  - `signal_run` (`m1-run-stop` marker): socket closed and child reaped in `1007 ms`.
  - `signal_campaign` (`withdraw` / `m1-terrain-record` campaign marker): socket closed and child reaped in `1008 ms`.
  - `coordinator_loss` (no renewal after initial 2,000 ms lease): socket closed and child reaped in `1963 ms` (bound `<= 2300 ms`).
  - `call_limit` (20,000 ms per-call cap inside 40,000 ms active decision window): socket closed and child reaped in `19958 ms` (`>= 19000 ms` and `<= 20500 ms`).
  - `repair_remaining` (500 ms remaining shared clock on repair call): socket closed and child reaped in `440 ms` (bound `<= 1000 ms`).
- **Pre-SQL local stop signal**: `production_stop_withdraw_and_manual_terrain_signal_before_blocked_authority_sql` verified that `m1-run-stop`, `withdraw`, and `m1-terrain-record` write their bounded `STOP\n` marker before blocking on a held campaign advisory lock, while SQL status remains `running` until the lock is released.

### 4. Bound initial disclosure configuration for operator review before any real episode

Before executing a real provider episode, the operator must supply a newly registered campaign with explicit `m1_inference_permission_v3` matching this bound template (with fresh non-nil UUIDs and current UTC window):

```json
{
  "version": "m1_inference_permission_v3",
  "run_profile": "autonomous-dns-provider-fixture-v1",
  "execution_mode": "controlled_fixture",
  "recipient": "https://api.deepseek.com/responses",
  "profile": "deepseek-responses-autonomous-v1",
  "provider_terms_approved": true,
  "provider_terms_ref": "<fresh-non-nil-terms-uuid>",
  "field_classes": [
    "mission_question_constraints",
    "scoped_opaque_references",
    "owner_qualified_summaries",
    "admitted_operator_premises",
    "manifest_availability",
    "omissions_unknown",
    "capacity",
    "admitted_repair_fields_codes",
    "admitted_campaign_decision_feedback"
  ],
  "campaign_limits": {
    "calls": 4,
    "request_bytes": 65536,
    "response_bytes": 262144,
    "output_token_capacity": 16384,
    "wall_time_ms": 90000
  },
  "operation_limits": {
    "context_bytes": 8192,
    "candidates": 8,
    "request_bytes": 16384,
    "configuration_bytes": 16384,
    "response_bytes": 65536,
    "calls_per_decision": 2,
    "output_tokens_per_call": 4096,
    "decision_wall_time_ms": 45000,
    "per_call_wall_time_ms": 20000,
    "shared_wall_time_ms": 40000,
    "phase_wall_time_ms": 50000,
    "cancel_grace_ms": 2000,
    "accounting_window_ms": 3000,
    "connect_time_ms": 5000,
    "proposal_bytes": 8192,
    "repair_bytes": 2048,
    "text_field_bytes": 1024,
    "repair_error_codes": 8,
    "json_depth": 32,
    "collection_entries": 64,
    "concurrency": 1
  }
}
```

### 5. Historical continuous-deadline correction (`DW-FIX-PR107-CONTINUOUS-DEADLINES`, F1/F2 VALID)

External review probes at FIX BASE `a55ff33cfdd31579a7d4a2df47eb38d05dc857b7` demonstrated two real timing gaps with the actual compiled CLI, restricted owners and a loopback reply relay; both are corrected on this branch with repository regressions.

- **F1 — preparation protection started post-claim (VALID, fixed)**: at FIX BASE, a withheld `execution.run_phase_claim` reply left the provider child alive **6621 ms+** with the phase ceiling as the only bound. Fix: the parent supplies a closed launch hint (`DW_M1_RUN_PROVIDER_DECISION`), pipes the provider-decision child's stdout, and runs an independent readiness watchdog armed **at spawn** for 5,000 ms; the child arms its own `Preparation` before its first database wait, verifies the hint against the authoritatively claimed run/phase, and emits the single bounded readiness frame `m1_run_ready_v1 <run> <phase>` only after the coordinator, armed transport and decision envelope exist. Deterministic/acquisition children keep their original 50/60-second ceilings.
- **F2 — no independent whole-decision guard (VALID, fixed)**: at FIX BASE, a withheld `execution.inference_reservation` reply left the run `running` **46,164 ms+** until the outer phase reaper. Fix: `Coordinator::start` arms a decision-envelope guard on the original deadline — permit revocation at the 40,000 ms active bound, then hard `exit(124)` at the 45,000 ms envelope (2,000 ms cancellation + 3,000 ms accounting), clamped to the run deadline; disarmed only on actual return.

**RED evidence** (fresh restricted fixture `dw_native_fixture_b09f91dfca23cf69`, binary built from `a55ff33` with only the new tests added): both regressions fail as required — "parent survived a parked claim session" (14 s bound) and "parent survived a parked reservation session" (50 s bound).

**GREEN evidence** (fresh restricted fixture `dw_native_pr107_deadline_fix_20261021a`, sequential `--test-threads=1`, Windows `libcurl 8.22.0-DEV`/Schannel):

| Case | FIX BASE | Fixed candidate |
| --- | --- | --- |
| Claim reply withheld | parked past 14 s bound (probe: alive 6,621 ms+) | parent exit `5,708 ms` from launch; child reaped by the 5,000 ms launch-clock readiness watchdog; `status="unknown"`, 0 decisions/effects |
| Reservation reply withheld | parked past 50 s bound (probe: alive 46,164 ms+) | reply withheld `44,314 ms`; parent exit `46,213 ms` from launch — the 45,000 ms decision envelope inside the 50,000 ms phase ceiling; `status="unknown"`, 0 decisions/effects, repeated `m1-run-recover` replays nothing |

- **Historical unit/protocol checks**: the earlier readiness regression accepted unrelated prelude lines; C4 supersedes that permissive framing. Pending-readiness and envelope tests passed then; the amendment strengthens their deadlines and deterministic boundary evidence. The historical parent exit times above are receipt/accounting observations, not separate measurements of child termination.
- **Compatibility/positive controls**: normal provider preparation still reaches the selected provider and durably reports a missing key without fallback; the ready inference child survives past 5 s when its decision clock permits work; deterministic two-decision orchestration and all pre-existing `m1_provider_fixture`/`m1_run_postgres`/`m1_run_failures` cases pass unchanged (9/9, 2/2, 11/11).
- **Ratchet**: `check_complexity` gate PASS with 0 regressions after extracting `PhaseChild::arm_guards`, `await_ready` and `verify_launch_hint` from the touched functions.

**Historical post-evidence corrections, superseded by the amendment below**:

- The earlier revision introduced an instrumented preparation override. Amendment C2 removes it completely; no instrumentation exemption applies to the fixed 5,000 ms policy.
- **Historical Codex P1 correction** attached a watched marker closure to `Permit::current()`. Amendment C1 supersedes it because filesystem reads in progress callbacks violate the accepted isolation contract.
- **Historical Codex P2 deferral** recorded the dual-failure receipt defect outside the previous FIX map. The amendment explicitly includes Terrain CLI/tests; F-B below implements the correction rather than treating deferral as resolution.

**Disposition**: **IMPLEMENTED / OFFLINE QUALIFICATION MEASURED; REAL PROVIDER EPISODE NOT EXECUTED** (missing fresh operator disclosure consent and authorized `DEEPSEEK_API_KEY` for a live provider run).

### 6. Continuous-deadline amendment (F-B, C1-C4)

This is the same outcome and FIX-cycle batch, based on `0d699ed9a7f61a80dc7c45ada98a0d4986c92287`; it grants no new effect, provider contact, schema, dependency or accepted authority. Only the amendment's production/test files and this packet plus engineering state change. No production timing/bypass flag is added.

| Correction | Observable implementation and proof |
| --- | --- |
| F-B | CLI composition captures one attempted campaign STOP before connection/store work. Receipt-only JSON `local_control: unresolved` survives configuration failure, authoritative refusal and successful durable publication; the original Terrain reason, identities and history stay intact. Inspect/recover do not signal. `local_stop_failure_*` failed at FIX BASE with a missing annotation; the Terrain CLI suite now passes 8/8. |
| C1 | `Permit` holds only atomics and monotonic instants. The transfer worker runs the binary-injected qualified Binding marker check once immediately before exchange, under an already armed independent guard, then rechecks the permit and call deadline. Fresh Broker/Mission release fences remain for generation and repair. RED: 1,000 permit reads invoked the watched hook 1,000 times. GREEN: generation/repair marker barriers refuse with zero HTTP requests; progress invokes the pre-send hook only once; late successful checks cannot undo STOP; blocked pre-send I/O cannot block EOF/lease hard exit. |
| C2 | Production parsing/forwarding of the preparation override is removed; only the hostile stale-value regression supplies it. Parent readiness cancellation/reaping fits inside its original launch +5,000 ms bound, including a 50 ms enforcement margin; child protection starts before its first DB wait. RED: the stale 120,000 ms value let the actual child survive the fixed bound. GREEN: actual child exit is observed against its launch +5,000 ms plus at most 500 ms OS observation margin, separately from the parent's later receipt. Ready inference survives five seconds. Deterministic/acquisition ceilings remain unchanged. |
| C3 | The actual `m1-run` reservation relay stays parked while the original Broker window timestamp is observed through a fresh restricted connection. Server-relative age is projected onto the local monotonic clock, avoiding Docker/Windows clock skew; sampling round trip and SQL duration must each be <=100 ms. Child exit must precede the original 45,000 ms decision envelope plus 500 ms OS observation margin and the earliest possible phase reaper. Normal measurement: child exit at 45,131 ms from the observed anchor, observation uncertainty 8 ms, with >3 s phase separation; parent accounting ends later. The identical assertion fails on an isolated build disabling only the whole-decision hard exit, with phase protection retained (child still alive at 45,559 ms). No bypass is committed. |
| C4 | Pre-readiness stdout contains exactly `m1_run_ready_v1 <canonical-run-uuid> <exact-decision-phase>\n`, <=128 raw bytes including LF. The private phase preserves its durable identity binding without the public progress print. The parser accepts only the first exact frame: no lossy UTF-8, trimming or prelude skipping. Tests reject wrong UUID/phase, EOF/missing LF, invalid UTF-8, whitespace suffixes (including 200 spaces), padded 128/129-byte inputs and queued late readiness; raw parsing consumes no 129th byte. Terminal draining remains bounded and outside watchdog/SQL loops. |

The reservation relay parks the PostgreSQL prepare reply, so it cannot prove a reservation COMMIT ACK was lost. The C3 test explicitly injects an existing two-call / 45,000 ms charge for the original decision through the real restricted reservation owner while the child is parked. Fresh inspection verifies that charge and original call IDs survive UNKNOWN and repeated DB-only recovery; there are zero provider sends, decisions or DNS effects. This is charge-preservation evidence, not a fabricated commit-loss observation. Existing effect commit/ACK-loss regressions remain separate.

Local evidence uses the fresh `dw_native_pr107_targeted_fix_20261021a` database and generated restricted principals on the retained owned `dwpg-r3` PostgreSQL 17 service/digest. One session owns `D:/DuskWeave-build/rust-1.94.1/normal`; coverage is reserved separately at `D:/DuskWeave-build/rust-1.94.1/coverage/pr107-targeted-fix`. Sanitized action logs and the isolated negative-control build are under `D:/DuskWeave-build/rust-1.94.1/reports/pr107-targeted-fix`. No database, role, container, volume, local Cargo configuration, CI workflow or build profile is deleted or changed.

Offline Windows x86_64 MSVC / Rust 1.94.1 / libcurl 8.22.0-DEV Schannel qualification includes focused native provider fixture 9/9, Terrain 8/8, run failures 11/11, deterministic run PostgreSQL 2/2, unchanged autonomous wire 2/2, helper tests 28/28, transport/control regressions, formatting, locked Clippy and document/dependency checks. The TLS regression initially lacked `openssl` on PATH, then passed with the existing Git OpenSSL executable supplied through its existing test variable. Full legacy suite/coverage remains assigned to final-candidate CI at unchanged 90% total / 80% per-production-file floors; local results do not qualify Linux or a later commit automatically.

QUALITY_BAR section 7 delta review:

| Question | Disposition and candidate evidence |
| --- | --- |
| Q1 Ownership | PASS: receipt composition stays in binary Terrain CLI; transfer/permit safeguards stay in transport; authority renewal/publication stay in Cognition. No persisted Terrain type changes. |
| Q2 Dependencies | PASS: binary injects only a marker-check hook; `Permit` imports no filesystem/owner/database code. Dependency checker passes; no domain-to-filesystem edge or dependency is added. |
| Q3 Simplicity | PASS: remove watched-permit state, override parsing and permissive line search. The private absolute-deadline waiter permits deterministic guard tests; it is used by the real guard. |
| Q4 Single truth | PASS: preparation remains fixed at 5,000 ms; each guard consumes original absolute instants. Public/private inference share the same durable input-binding function. |
| Q5 Requirement fit | PASS for measured offline boundaries: all five corrections preserve legitimate strategy outcomes and existing P/C/B fixtures. Real provider/target execution and milestone acceptance are outside this amendment. |
| Q6 Reachability | PASS: `m1-terrain-record` -> signal -> qualified stores -> annotated receipt; `m1-run` -> launch/readiness supervision -> private inference binding -> Coordinator -> AutonomousProvider pre-send -> pure curl callbacks. Actual CLI/native and loopback tests exercise those consumers. No changed runtime island, orphan or new operator command. |
| Q7 Failure integrity | PASS: independent local STOP loss remains visible with the primary failure; stalled callbacks/SQL cannot extend original guard deadlines; late readiness refuses; UNKNOWN, charges and original identities survive repeated zero-replay recovery. Negative-control failure is recorded before custodian cleanup. |
| Q8 Trust/claims | PASS within tested scope: marker observation before pre-send refusal yields zero sends; a marker racing after that check or already initiated disclosure cannot be recalled. Loopback synthetic keys only. Existing external review evidence is reused; final exact-head CI, security findings and thread dispositions are checked separately. |

The pinned McCabe ratchet against the PR base passes with zero regressions and ten unchanged debt items; touched Terrain `command` decreases from original PR-base 8 to 5. The cumulative runtime diff against the PR base measures 1,856 added/deleted lines across 15 production/SQL files. Runtime diff, module sizes and function spans remain advisory. Retain the >400-line transport/CLI/supervision files because they own cohesive transport, composition and independent child lifecycle responsibilities; their private tests belong to those boundaries. Longer `decide` remains one linear composition sequence; the enlarged reservation regression retains one complete fault/identity/recovery scenario rather than fragmenting assertions for counts.

Tracking disposition: UPDATE this packet and `docs/ENGINEERING_STATE.md` with amendment evidence and the pending PR delivery action. Foundation/M0 seals and exact accepted section 7/4a authority remain unchanged. This candidate is not merged; real provider episode is NOT EXECUTED.

**Fixed-policy CI correction in the same batch:** CI run `39557151765` on the first amendment commit failed under Linux coverage at the reservation barrier: the child never reached readiness within the fixed preparation bound. Durable phase timings were 1,689 ms from phase-start to claim and 5,144 ms from phase-start to the parent's UNKNOWN accounting record; these are coarse phase observations, not a complete per-stage latency profile or permission to extend the clock. Coverage was not produced, so that run does not qualify Linux or the coverage floors.

The coordinator had opened two separately qualified Broker connections for sequential run-current and Terrain/Mission reads on its one worker. It now uses its already-qualified Terrain-basis Broker connection for both, retaining every fresh SQL check, 200 ms statement / 100 ms lock bounds, check-start lease anchoring and independent permit/envelope/parent control paths. This removes a duplicated owner connection/qualification and wait-policy setup; no identity/role check or original deadline is skipped. The ordinary main-thread owner connections remain separate from the coordinator. Requalification uses fresh `dw_native_pr107_targeted_fix_20261021b` and restricted generated logins because the first bootstrap's private credential session ended; the `a` fixture is preserved. Measured evidence is under the existing report directory's `followup` subdirectory. Windows evidence is rerun for the affected coordinator/provider path; replacement final-head CI must qualify Linux and coverage before READY.

Follow-up local evidence: locked Clippy, coordinator 3/3 and the pinned ratchet pass. The provider fixture rerun passes eight cases including both deadline regressions; the unchanged withdrawal writer in the STOP-before-SQL case fails once, then its focused recheck passes 1/1 without code or assertion changes. This observed local variability is retained, not relabeled a clean 9/9 run. Decision child termination is measured at 45,116 ms from the original observed anchor, with 7 ms anchor sampling uncertainty and 3,371 ms separation from the earliest phase ceiling. The two-file runtime delta is reviewed against Q1-Q8: one worker performs the same sequential owner reads with one qualified connection; no callback, authority, identity, freshness, guard or failure semantic is weakened.

**STOP / NOT READY:** replacement CI run `39559834003` again fails `stalled_reservation_reply_hard_stops_at_the_original_decision_envelope` before the reservation barrier under Linux coverage. The fixture reaches claim after 1,512 ms and the parent records UNKNOWN after 5,141 ms from phase-start. The fixed preparation enforcement remains active; this failure does not measure a decision-guard overrun because that boundary was never reached. Coverage generation aborts and the 90%/80% gate is skipped, not passed. No required regression is ignored; no deadline override, threshold/profile change or further runtime correction is performed after this remaining blocker. Both CI failures and the local variability stay part of the delivery record; the PR remains unmerged and the real provider episode is NOT EXECUTED.

The missing proof is a per-stage Linux instrumented preparation profile separating executable hash/claim, qualified owner setup, ContextPack assembly, coordinator setup and fault-relay overhead. Exact diagnostic boundaries are `src/m1_run_cli.rs::{release,claim_child}`, `src/m1_reasoning_cli.rs::{autonomous_owners,autonomous_inference,prepared_basis}`, `src/m1_run_inference.rs::decide`, and `src/m1_inference_control.rs::Coordinator::start`, exercised by `tests/m1_provider_fixture.rs`. The relay in read-only `tests/support/database_wait_proxy.rs` must be distinguished from production cost before attributing the failure; neither relay overhead nor a required wider owner-store change has been established. Owner-store/relay edits remain outside this amendment. Root-cause review and measured re-scope are the next action under the existing FIX-cycle STOP rule, not another blind correction or a relaxed clock.

CodeQL analysis of the last runtime candidate reports zero results with no analysis error; metrics, dependency, structure and link checks pass. The existing distinct CodeRabbit review through the first amendment head found no actionable comments; it skipped four similar files. Its last push status explicitly skips incremental review, so no fresh independent review of the connection correction is claimed. The Q1-Q8 correction review above is the recorded delta review. All actual review threads, including F-B with code/test references, are resolved; those facts do not override the failed runtime gate.

### 7. Relay qualification amendment

The owner-issued `PR107-RELAY-QUALIFICATION-AMENDMENT` authorizes this bounded test-fixture/CI re-scope from `15e1170bf084b99dc5131a75fea72649368d1462`, preserving the same outcome and FIX-cycle history. It supersedes the blanket STOP for the five mapped files only; further production corrections remain paused. Accepted M1 section 7 / ADR-016 section 4a, fixed 5,000 ms preparation and 45,000 ms decision policy, original timing/identity/charge/recovery assertions and Foundation/M0 seals remain unchanged. This supports actual two-decision provider/fixture orchestration without granting provider or target contact.

Baseline Linux coverage run [39561609577](https://github.com/carlitotate12160-tech/DuskWeave/actions/runs/39561609577) remains genuine RED: provider fixture 8 PASS / 1 FAIL at the reservation barrier. The parent records UNKNOWN 5,141 ms after phase-start; that accounting interval does not measure child lifetime or a decision overrun. The owner-provided transport-pattern spike (three Linux samples of 40 exchanges, 3,405-3,444 ms versus 41-57 ms with TCP_NODELAY) motivates the correction but is not actual Rust integration GREEN or evidence that coverage alone caused the failure.

`tests/support/database_wait_proxy.rs::session` now calls one private socket-configuration function before startup forwarding or cloning. Both accepted-client and upstream-server streams require TCP_NODELAY and the existing read/write timeouts; configuration failure returns and closes the session. Framing bounds, Q/P detection, withholding-before-forwarding, exact NUL-terminated completion tags, open parked sockets, POLL and owned shutdown remain intact. No transient startup/authentication/result bytes are logged. New private regressions exercise that actual configuration path, fragmented startup/frames, nonmatching messages and matching Q/P withholding; error, rollback, prefix and non-NUL completion tags cannot signal success. A 40-exchange actual relay measurement reports elapsed time and complete synthetic wire bytes without imposing a performance threshold.

Local Windows x86_64 MSVC / Rust 1.94.1 evidence:

- Socket-policy RED fails the actual configuration test with `accepted client must use TCP_NODELAY`; after correction, relay tests pass 3/3. The synthetic loopback case forwards 1,128 bytes across 40 round trips in 25 ms (one diagnostic sample, not a latency guarantee).
- Native claim/reservation regressions pass 2/2, serially through the existing bootstrap. Launch-to-barrier observations are 680 ms / 1,520 ms. Reservation child exit is observed at 45,121 ms from its original decision anchor, with 7 ms sampling uncertainty and 3,505 ms separation from the earliest phase reaper; UNKNOWN, original IDs, retained charge and repeated zero-replay recovery assertions pass unchanged. Parent accounting remains a separate observation.
- The owned pinned PostgreSQL 17 `dwpg-r3` is retained. Initial fixture `dw_native_pr107_relay_fix_20261021a` was provisioned but its stdin session ended before tests; it is not PASS and its credentials were not recovered. Fresh replacement `dw_native_pr107_relay_fix_20261021b` supplies actual test evidence with generated restricted logins. Both fixtures and all earlier resources remain intact.
- One owner uses ordinary target `D:/DuskWeave-build/rust-1.94.1/normal`; sanitized logs are under `D:/DuskWeave-build/rust-1.94.1/reports/pr107-relay-fix`. Optional coverage remains separately reserved at `D:/DuskWeave-build/rust-1.94.1/coverage/pr107-relay-fix`. Local full legacy suite/coverage is not claimed. Fmt, locked all-target Clippy, links/structure/dependency checks, diff whitespace and pinned McCabe ratchet pass; ten unchanged production hotspots remain, with zero regressions.

The existing `rust` CI job now runs the exact reservation regression under `cargo llvm-cov --no-report` first, using `--container dwpg --ci`, a fresh `dw_native_pr107_relay_<run_id>_<attempt>` fixture and `$RUNNER_TEMP/pr107-relay-focus`. Failure propagates without retry or fallback. Only success permits the existing full-suite step, whose workspace-scoped profile cleanup excludes focused measurements from final coverage while retaining compatible compiled artifacts. Full-suite fixture/report identities stay separate. Both sanitized qualification logs use the existing native-evidence artifact; final coverage upload is skipped if its measurement never ran. No new runner, dependency, binary override, timing flag, policy or threshold is introduced.

QUALITY_BAR section 7 correction-delta review:

| Question | Disposition and evidence |
| --- | --- |
| Q1 Ownership | PASS: socket policy and synthetic tests stay in the test-only relay; CI orders qualification; production owners are unchanged. |
| Q2 Dependencies | PASS: only existing std socket APIs and pinned CI/bootstrap tools; no new cross-domain edge, manifest or dependency. |
| Q3 Simplicity | PASS: one configuration boundary is directly consumed by session and its regression; existing protocol loops are preserved. |
| Q4 Single truth | PASS: timing policy remains in unchanged production code; tests retain original absolute anchors and CI coverage floors remain 90%/80%. |
| Q5 Requirement fit | PASS: actual relay configuration and both native regressions execute; Linux focused/full coverage pass in run 39569287257 on the relay source candidate. |
| Q6 Reachability | PASS: existing integration targets include relay -> session -> socket configuration -> framed forwarding/barriers; CI invokes the real m1-run regression through the existing bootstrap. No production component or orphan is added. |
| Q7 Failure integrity | PASS: failed configuration refuses the session, nonmatching traffic forwards, exact completion controls and parked sockets remain tested; Linux full-suite shared-consumer compatibility passes in run 39569287257. |
| Q8 Trust/claims | PASS for source/local evidence: no raw authentication bytes, external provider calls, widened privileges or substituted test binary; RED/spike/Windows/Linux claims remain separate. |

Retain the workflow above its advisory 400-line reference because the focused step belongs to its existing rust job and artifact lifecycle. Retain the existing provider test file above 500 lines because this delta adds only safe barrier diagnostics to the same complete regression. The relay remains below its 500-line test reference; its configuration helper owns a real socket requirement rather than moving protocol branches for counts. The existing distinct review is reused for unchanged boundaries; no new independent review is claimed by this author delta review.

**Linux qualification:** [run 39569287257](https://github.com/carlitotate12160-tech/DuskWeave/actions/runs/39569287257) on relay source candidate `909b7a732511c09416463831d8f56dc5328e5c2c` (tested merge `cbe26462e880b68baea13e7b7926e120d580cf2e`) passes every gated job and diagnostic geiger. The exact focused instrumented reservation case passes 1/1: launch-to-barrier 4,432 ms; original anchor 2,047 ms from child launch; sampling uncertainty 2 ms; child termination observed at anchor +45,006 ms, with 673 ms separation between the observation deadline and the earliest phase reaper. The existing <=500 ms margin and all identity/charge/UNKNOWN/zero-replay recovery assertions remain intact. This is actual Linux Rust integration GREEN, separate from the transport-pattern spike and Windows evidence; it is not a general preparation-latency guarantee.

The separate fresh full-suite fixture then passes all targets, including provider fixture 12/12 (nine existing cases plus three relay tests), database-wait and COMMIT-ACK consumers. Profile cleanup precedes that full measurement. The original CI gate reports **96.94% production line coverage (10,457/10,787)** and every reported production file >=80%. Both native logs and the coverage artifact are retained; downloaded evidence lives under the existing local report directory's `ci-909b7a7` subdirectory. A direct Windows re-evaluation of the downloaded Linux JSON refuses its absolute Linux source paths; it is not a local coverage PASS or a failure of the original Linux gate. Coverage qualification above comes from that gate's Linux log. CodeQL for the tested merge reports zero results and no analysis error, open alerts are empty, and all eight actual review threads are resolved. CodeRabbit still skips incremental review; no new independent review is claimed.

Tracking disposition: UPDATE this packet and `docs/ENGINEERING_STATE.md`; actual Linux focused/full evidence resolves the earlier readiness STOP for the tested source. The final tracking-only candidate must still pass its own required checks before owner review; current head/check/OPEN state comes from GitHub, not this commit. Further production edits remain paused and there is no automatic merge. The earlier Linux failures, local variability and FIX-cycle history remain recorded. Stop further corrections if focused Linux qualification fails again; report an external sanitized profile separating hash/claim, qualified owner setup, ContextPack, coordinator and relay time. No real provider episode is executed.

**Merge record (tracking reconciliation):** the owner squash-merged this packet's final candidate to `master` at `2af7d01c9aa29c799d159d127a29b5766c60cd5f` on 2026-10-21T09:30:54Z after all required checks on head `17480ed` passed and all review threads were resolved. This records delivery only: the real provider episode remains NOT EXECUTED, live acquisition and M1 seal readiness remain unqualified, and the fixed preparation/decision policy is unchanged by the merge.
