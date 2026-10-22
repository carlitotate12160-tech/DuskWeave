# DW-DESIGN-M1-PROVIDER-NEUTRAL-LIFECYCLE

PROJECT: DuskWeave
MODE: DESIGN
WORKSPACE: D:/DuskWeave-ci-complexity
BRANCH: docs/m1-provider-neutral-lifecycle (create from the verified detached base)
BASE: 56039deeb2281ac6149307920ebba9d8d2ae7d3a
STATUS: Executable DESIGN assignment; proposed amendments are not yet accepted runtime authority.
TRACKING: UPDATE the three existing navigation/state files listed below.

Execute this documentation assignment directly. Do not return another architecture plan or implement runtime changes.

## Outcome and authority

Publish one bounded amendment defining provider-neutral inference lifecycle and freshness semantics for the existing autonomous loop:
Mission -> actual LLM decision one -> Broker-admitted DNS fixture -> owner-qualified Terrain -> actual LLM decision two -> durable history/inspect, without intermediate operator input.

The owner's multi-provider requirement is foundational: DeepSeek is the presently available provider, with OpenAI API, Anthropic Claude API, Mistral API, OpenRouter and other qualified adapters supported by the architecture. OpenRouter is one aggregated protocol provider exposing individually qualified model profiles (including the Qwen, GLM and Kimi families), not automatic multi-vendor failover. Their operational support must be reported individually, never inferred from a provider-neutral interface.

Preserve objective-led, authorized adversary emulation, active hypothesis testing and evidence-led adaptation. APT41, Lazarus and Volt Typhoon remain behavioral reference lenses, not actor-equivalence claims, playbooks or permissions. This supporting slice enables reliable cognitive feedback; it adds no target action.

Governing sources: accepted PRD-000 invariants; ADR-002 ownership, ADR-004 replaceable inference, ADR-006 freshness; M1 enabling architecture sections 3/7; ADR-016 section 4a; the accepted external-orientation contract and existing MVP roadmap. The owner's current agreement authorizes preparing the amendment, not marking its final text ACCEPTED.

## Preconditions

1. Follow the repository authority-loading order. Verify origin, worktrees, HEAD, branch and status. The named worktree was tracked-clean and detached at BASE when checked. Preserve other worktrees and unrelated files.
2. PR #114 is merged at BASE; reconcile that predecessor fact. Do not write this candidate's own HEAD, merge or seal into its files. If the relevant base changes, review the delta before proceeding; do not silently transplant this assignment.
3. Use the existing architecture and packet template. No new PRD, ADR number, service, manager, registry framework, dependency, governance gate or skill change.
4. No provider inference, endpoint health check, target/resolver contact, credential inspection or database mutation. Do not read, copy or edit .cargo/config.toml or expose .env/DSNs.
5. Build target, PostgreSQL provisioning and runtime TDD are N/A for this DESIGN-only delta. Do not rerun PR #109 tests.

## Exact allowed files

- D:/DuskWeave-ci-complexity/docs/contracts/M1-enabling-architecture.md
  Add the explicitly PROPOSED revision within section 7; add only the necessary section 3 cross-reference. Preserve historical accepted profiles and acceptance scope.
- D:/DuskWeave-ci-complexity/docs/adr/ADR-016-execution-broker.md
  Add the matching explicitly PROPOSED revision under section 4a. Keep numerical lifecycle/accounting authority here.
- D:/DuskWeave-ci-complexity/docs/ENGINEERING_STATE.md
  Replace stale active-next-action pointers with this pending DESIGN and the unqualified feedback outcome.
- D:/DuskWeave-ci-complexity/docs/BUILD_ORDER.md
  Update current navigation only; preserve stage dependencies and seals.
- D:/DuskWeave-ci-complexity/docs/MVP_AND_DEFERRED_SCOPE.md
  Update existing sections 1.2/1.3/1.4 only where necessary: record multi-provider direction, the lifecycle blocker and the bounded delivery order.
- D:/DuskWeave-ci-complexity/docs/workflows/DW-DESIGN-M1-PROVIDER-NEUTRAL-LIFECYCLE.md
  Retain this assignment and compact source-bound design/review evidence. Do not create a separate audit/report family.

All runtime, SQL, tests, dependencies, CI and other documentation are read-only.

## Decisions to encode

### 1. Preserve ownership and the existing inference port

Mission owns approved recipients, disclosures, terms, windows and permissions. Broker owns call reservation, release, accounting and cancellation. Cognition owns safe ContextPack/proposal semantics and decision publication. Terrain owns claim qualification and current-use eligibility. Trajectory owns durable history. The existing supervisor owns phase lifecycle and observed termination attribution.

Retain the Provider port in src/m1_reasoning/inference.rs as the architectural seam. Do not add an all-domain ProviderManager, routing service or mutable global catalog. Deployment composition supplies a qualified adapter through narrow dependencies. A provider adapter translates approved input and untrusted output; it cannot grant execution authority or mutate Terrain.

### 2. Separate provider identity, protocol and model profile

Define the meaning of a versioned deployment profile: exact approved recipient/endpoint and protocol revision, requested model, provider-native settings, supported features, credential reference, template/schema/adapter versions, capacity limits and timing profile. Keys stay in adapter-only secret configuration.

The shared contract is the safe context, proposal meaning, lifecycle and provenance, not the DeepSeek Responses wire format. OpenAI Responses, Claude Messages, Mistral Chat Completions and OpenRouter's OpenAI-compatible endpoint may have different envelopes, stream events, output controls and usage fields. Do not assume changing base_url creates compatibility; OpenAI-compatible claims do not establish wire-fit. Requested settings must be supported by that profile or rejected before reservation/send; never silently drop them or translate reasoning effort as if equivalent across vendors.

An aggregated provider adds one more layer: the recorded disclosure recipient is the aggregator itself, which may route requests to upstream vendors under its own terms and data policies. Approved provider terms must therefore cover the aggregator and its upstream routing, and each exposed model remains a separately qualified model profile beneath that single provider identity. An OpenRouter profile such as `openrouter-chat-completions-v1` would expose Qwen, GLM, Kimi and other model profiles through one endpoint and credential without granting cross-model substitution; the selected model profile stays pinned for the admitted run.

Record requested versus reported model/revision separately; unavailable metadata remains unavailable. Normalize only supported safe result/error categories, retaining terminal reason and usage availability without persisting raw provider text or private reasoning. Missing usage is not zero; provider token categories are not automatically comparable or additive.

The immutable context/configuration and actual provider request must remain bound to the selected profile. Adapter-specific encoding cannot silently omit mandatory premises, change references or replace the proposal schema. Require wire-fit qualification for each adapter, including complete generation and repair requests.

### 3. Select one qualified profile per run initially; distinguish failover

The first successor runtime slice permits explicit selection among installed, qualified provider profiles at registration/start. Pin that selection for both decisions and conditional repair in the run. Record it in immutable configuration and history. A new qualified provider must not require changes to Terrain, Mission's domain meaning, action authority or the supervisor's phase algorithm.

Only DeepSeek is currently available for real qualification. Other providers, including OpenRouter and its model profiles, are planned, not supported. Deterministic/local protocol fixtures can test interchangeability without keys; they do not establish real-provider qualification.

Multi-provider support does not imply automatic failover, hedging, parallel calls or role-based model routing. None is selected for this first slice. No implicit SDK retries, ambient credential discovery, proxy routing, arbitrary endpoint strings or provider/model switch after failure.

Document the extension boundary for later explicitly approved failover: all recipients/field classes/terms/windows must already be authorized; each new attempt needs its own durable call identity, reservation, release fence and remaining budget; uncertain prior consumption stays charged; no deadline reset or target-effect replay. Existing unresolved-obligation barriers are not bypassed. Enabling that policy requires its own bounded delivery, not another generic architecture subsystem.

### 4. Resolve freshness independently of inference completion

For the proposed successor lane, select this behavior:
- The pack contains a source-qualified snapshot eligible when released, with original perception time and expiry. Freshness is never refreshed by packing, inference or publication.
- TTL-only expiry during an already admitted call removes present routing eligibility but does not by itself revoke permission to wait for the screened reasoning result.
- Mission withdrawal/window expiry, STOP, control-lease loss, incompatible owner revision, repudiation/conflict or integrity hold still stop affected inference under the authorized cancellation bound. Do not weaken those checks while separating TTL.
- Cognition may durably record a screened reasoning result and its actual admission disposition even when a premise has become stale. A stale-dependent action is blocked/deferred until owner requalification. Do not rewrite the model's selected intent into a model-chosen defer or claim a currently eligible action.
- Revalidate the owner's present premises before consequential use/dispatch. Historical reasoning grants no authority. New acquisition, if needed, is a separately admitted effect with budget; no automatic DNS refresh, timestamp extension or replay.

Preserve the current DNS expiry formula: perceived_at + min(lowest applicable DNS TTL, 60 seconds). This revision changes the relationship between expiry and inference waiting, not Terrain's TTL. Describe the result/admission distinction without inventing a new operational model.

Keep v1/v2/v3 behavior and consent historical; no silent upgrade. The amendment becomes applicable only through an explicitly versioned successor permission/profile after acceptance and implementation. In the current source TTL still clamps the transport permit; do not describe this new behavior as implemented.

### 5. Define coherent, immutable timing profiles

Separate connection, transfer inactivity where available, semantic-event observation, soft completion, shared generation/repair, decision, preparation/phase, run, stop escalation and accounting boundaries. Profile values are finite, versioned and fixed for the admitted run; runtime only clamps to legitimate remaining authority/budget.

Soft completion leaves room for cooperative cancellation and database-only disposition within the original envelope. The independent hard stop remains the fallback for blocked transport/callbacks. Semantic events cannot extend a hard authority deadline. No LLM settlement call is required to save already committed facts.

A model profile reflects model/settings/output capacity and measured latency. Do not derive adequate inference time solely from prompt bytes, assume 45/60/300 seconds is an industry guarantee, or claim one successful sample establishes a percentile.

Inventory the current 20s call / 40s shared / 45s decision / 50s inference phase / 60s CLI phase / 150s run / 90000ms two-decision inference reservation, plus campaign/window limits. Explain which guard begins at which lifecycle point. Validate finite profile feasibility at admission, including conditional repair and accounting reserves. A run need not permit every phase to consume its maximum; insufficient remainder yields a visible disposition.

Do not invent successor production timer numbers in this DESIGN. The amendment defines their qualification and consistency rules; exact values and measured margins are prerequisites for the subsequent runtime packet and fresh disclosure. The existing operational profile remains unchanged until then.

Renewal cadence and lease duration are a separate revocation/availability contract, not a function of how long the model thinks. Name the current 400ms target / 500ms check bound / 2s lease and DB/scheduler failure modes. Preserve them pending explicit qualification; do not quietly enlarge them with the LLM deadline or infer DB overload from query count alone.

### 6. Preserve screening if internal streaming is selected later

Streaming is optional transport observability, not mandatory for all providers. Distinguish connection bytes, keep-alive, semantic provider events and completed validated output. None proves reasoning quality; keep-alive never renews semantic progress or authority.

Any streaming implementation must assemble the final bounded result before admission and apply key exclusion and sensitive-data screening to decoded/assembled content as well as relevant raw input. A key can be split across deltas. Bound cumulative wire bytes, decoded content, event sizes and counts before growth; reject malformed, incomplete, oversized or contradictory terminal results. Emit only allowlisted safe metadata, never raw deltas/private reasoning.

Use the provider's documented event contract; do not assume DeepSeek's terminal events, Anthropic events and Chat Completions [DONE] are interchangeable. Automatic SSE reconnect/resubmission is not authorized.

### 7. Attribute termination through existing owners

Extend the current parent diagnostic publication path conceptually. The parent can record its own deadline/kill; child guard reasons require an explicit bounded safe signal. Signal handling must not delay stopping or require database writes before hard termination.

Differentiate observed cause from unavailable cause. Exit 124 and elapsed time alone never identify a guard or prove provider latency. If the parent dies or publication fails, inspection reports the last durable marker and pending/unknown attribution honestly. Recovery remains database-only and never regenerates the proposal.

Preserve already committed DNS/Terrain/history outcomes separately from failed inference. Existing may_send_uncertain reconciliation against provider billing is operational debt, not a prerequisite or an assumed available API; keep the reservation charged and do not fabricate usage/refunds.

## Required source/consumer map and counterexamples

Record a compact map in the packet, with exact repository paths and proposed semantic impact:
- Profile/configuration: src/m1_reasoning/request_autonomous.rs; src/m1_inference_permission.rs; migrations/0016_autonomous_inference_profile.sql.
- Provider boundary/admission: src/m1_reasoning/inference.rs; src/m1_reasoning/transport.rs; src/m1_reasoning/transport_control.rs; src/m1_reasoning/postgres_terrain.rs.
- Freshness: src/m1_terrain_view.rs; src/m1_inference_control.rs; migrations/0015_m1_terrain_context_view.sql.
- Outer guards/diagnostics: src/database_wait.rs; src/m1_run_supervision.rs; src/m1_run_cli.rs; migrations/0017_autonomous_fixture_run.sql; migrations/0020_m1_run_diagnostics.sql.
- Feedback/consumers: src/m1_context.rs; migrations/0019_provider_fixture_feedback.sql; tests/m1_provider_fixture.rs; tests/m1_run_failures.rs; tests/m1_run_postgres.rs.

These are read-only references, not an implementation file allowance. A later runtime map must include all actual consumers and additive SQL migration needs; do not edit applied migrations.

Specify observable future assertions, not claims that tests already ran:
1. The production CLI path works with two different local protocol fixtures under the same owner/lifecycle semantics; no real provider qualification is inferred.
2. Unsupported provider/settings or missing permission/key availability refuses before release, without another provider being tried.
3. Inference can finish after TTL-only expiry, but a stale-dependent effect receives zero release; history preserves original times and the actual model intent.
4. Withdrawal/lease loss stops a long call; keep-alive-only traffic cannot prolong its hard bound.
5. A longer inner profile is not silently killed by unchanged 50s/60s outer guards; insufficient run/window remainder is detected and reported.
6. Fragmented UTF-8/events and split-key deltas cannot bypass screening or admit partial output.
7. Slow DB renewal and scheduler gaps produce honest control-unavailable outcomes; diagnostics do not block stopping.
8. Parent/child/publication failures retain original identities and uncertainty; repeated inspection/recovery yields zero provider/DNS replay and no refund.

## Tracking and evidence

Reconcile PR #114's actual merge. Record the owner-directed multi-provider requirement and this DESIGN as pending. Preserve accepted Foundation/M0 seals and M1's unsealed status.

The 2026-10-22 provider-fixture run 4a4ce477-5734-4b92-bb6a-dce74f430114 established first real decision, committed fixture DNS/Terrain and correctly linked second input; second inference ended exit 124 with may_send_uncertain and no admitted second proposal. It did not demonstrate complete feedback or establish the exact killer. Safe retained evidence is at:
D:/DuskWeave-build/rust-1.94.1/reports/provider-fixture-demo-20261022a/immediate/demonstration-report.md
Use only safe summaries; do not import raw provider data/credentials.

Delivery order in the existing roadmap:
1. Review/accept this bounded amendment.
2. Prepare one runtime packet through the existing CLI/owners, with qualified finite values, DeepSeek as the first real adapter and a second local protocol fixture proving interchangeability.
3. After runtime qualification, prepare one freshly authorized real DeepSeek fixture demonstration; no inherited consent/window.
4. Add OpenAI/Claude/Mistral adapters and one OpenRouter adapter (covering Qwen, GLM, Kimi and other qualified model profiles under its aggregated terms) individually through the same contract suite when their credentials and qualification are available. Multi-provider support remains an explicit required outcome; automatic failover is a separately qualified policy.

## Checks, review and completion

Run the existing documentation checks from the worktree:
- python scripts/check_links.py
- python scripts/check_structure.py
- git diff --check 56039deeb2281ac6149307920ebba9d8d2ae7d3a
- Inspect the exact six-file allowance, conflict markers, terminology, authority links and status consistency.

Runtime TDD/wiring/coverage/McCabe execution is N/A: executable files are unchanged. Keep the future counterexamples as acceptance requirements, not fabricated GREEN evidence. Existing CI scope classification remains authoritative; do not change CI to skip checks.

Apply QUALITY_BAR section 7 self-review, then the existing distinct adversarial review by a different reviewer instance. Do not claim self-review as that review or create another gate. Challenge freshness/use separation, provider disclosure, cross-layer budget fit, cancellation attribution and zero replay.

Completion: one cohesive documentation candidate in English, exact diff/check results, no new unresolved architecture choice delegated back to the IDE, and honest remaining calibration/runtime/provider-qualification prerequisites. Final status is READY FOR DESIGN REVIEW when checks pass; no self-acceptance, runtime readiness, provider support claim, M1 seal or automatic merge.

STOP with the concrete conflict for missing required authority, incompatible relevant base changes, violated invariants, unsafe preservation of unrelated work, or material scope expansion. Do not stop for advisory document size alone; retain cohesion without splitting into helper documents.

## Primary references

Checked 2026-10-22; references explain protocol/engineering behavior and do not grant DuskWeave authority.
- https://api-docs.deepseek.com/guides/responses_api/
- https://api-docs.deepseek.com/quick_start/rate_limit/
- https://developers.openai.com/api/docs/guides/migrate-to-responses
- https://platform.claude.com/docs/en/api/messages/create
- https://docs.mistral.ai/api/endpoint/chat
- https://openrouter.ai/docs/api-reference/overview
- https://www.python-httpx.org/advanced/timeouts/
- https://aws.amazon.com/builders-library/timeouts-retries-and-backoff-with-jitter/
- https://html.spec.whatwg.org/multipage/server-sent-events.html


---

## Revision R1 — owner acceptance reconciliation (2026-10-22)

PROJECT: DuskWeave
MODE: DESIGN — correction and owner-acceptance reconciliation
PACKET: DW-DESIGN-M1-PROVIDER-NEUTRAL-LIFECYCLE (same packet, revision R1)
WORKSPACE: D:/DuskWeave-ci-complexity
BRANCH: docs/m1-provider-neutral-lifecycle
BASE: 56039deeb2281ac6149307920ebba9d8d2ae7d3a
TRACKING: UPDATE existing state/navigation and the same packet.
DESIGN STATUS: ACCEPTED by the product owner on 2026-10-22 (Asia/Jakarta).

Execute this correction directly on the existing candidate. Do not return a new plan, create another packet/worktree, or implement runtime code.

## Outcome, authority and preconditions

Reconcile the owner's explicit acceptance of the provider-neutral lifecycle/freshness design, including the agreed user-supplied API key, OpenRouter routing and documentation-ownership corrections. The owner explicitly approved the design and asked that its active status no longer be PROPOSED; do not request that same approval again.

The outcome remains Mission -> actual first LLM decision -> Broker-admitted DNS fixture -> owner-qualified Terrain -> actual second LLM decision -> durable history/inspect, without intermediate operator input. Preserve objective-led authorized adversary emulation and evidence-led adaptation; APT41, Lazarus and Volt Typhoon remain behavioral references, not playbooks or permissions.

Governing authority remains PRD-000 invariants, ADR-002/004/006, M1 enabling architecture, ADR-016, the external-orientation contract and the existing roadmap. Apply the new owner acceptance to the successor-lane design; existing v1/v2/v3 operational profiles retain their original semantics.

Verify origin, worktrees, branch, HEAD and status. At handoff, HEAD equals BASE and this branch contains five modified Markdown files plus its untracked packet. Those six files are the existing candidate, not unrelated dirt: preserve and correct them in place. Review any intervening relevant delta before proceeding. Do not reset/clean, recreate the branch, or overwrite other worktrees.

Use the repository authority-loading order, QUALITY_BAR section 7 and existing packet template. Runtime build targets, DB provisioning and executable TDD are N/A. No provider inference/health check, target/resolver contact, credential inspection, database mutation, or PR #109 test rerun. Do not read/copy/edit .cargo/config.toml or expose .env/DSNs.

## Exact allowed files

1. D:/DuskWeave-ci-complexity/docs/contracts/M1-enabling-architecture.md
2. D:/DuskWeave-ci-complexity/docs/adr/ADR-016-execution-broker.md
3. D:/DuskWeave-ci-complexity/docs/ENGINEERING_STATE.md
4. D:/DuskWeave-ci-complexity/docs/BUILD_ORDER.md
5. D:/DuskWeave-ci-complexity/docs/MVP_AND_DEFERRED_SCOPE.md
6. D:/DuskWeave-ci-complexity/docs/workflows/DW-DESIGN-M1-PROVIDER-NEUTRAL-LIFECYCLE.md

All other files are read-only. Reuse the original packet's source/consumer map. Do not add a new ADR/PRD, UI implementation, service, generic manager, catalog framework, dependency, CI rule or tracking document.

## Required corrections

### A. Record acceptance accurately

Change the active 2026-10-22 amendment metadata, status paragraphs, section 3 cross-reference, packet instructions and state/roadmap pointers to ACCEPTED. Record the product-owner conversation approval date and scope; this is owner acceptance, not executor self-acceptance. Distinguish that accepted design from the delivery/check/merge status of this documentation candidate.

Rename only the new 2026-10-22 amendment headings to:
"Provider-neutral lifecycle and freshness revision (2026-10-22)".
Update all affected links within the six-file map to:
#provider-neutral-lifecycle-and-freshness-revision-2026-10-22

Do not globally replace historical PROPOSED text. Preserve the older 2026-10-19 headings/anchors and acceptance at 0615fb72c946fd6abf0823692be621096091377c, Foundation/M0 seals and historical failures.

Remove active claims that acceptance is still pending. Describe applicability as a versioned successor permission/profile after qualified implementation and fresh run authorization. Acceptance does not invent timer values, activate providers or upgrade old consent. Do not embed the candidate's own HEAD SHA or claim its merge.

### B. Make user-owned connections a product requirement

The future user-facing flow is: select supported service -> provide that service's API key -> select an available compatible model -> use the saved connection for a campaign. This packet specifies the behavior; it does not build a web UI. Web delivery and multi-tenant deployment are not prerequisites for the first CLI fixture demonstration; their isolation assertions apply when those consumers are delivered.

Separate:
- User/organization-owned connection: service, approved recipient and opaque credential reference.
- Protocol implementation: request/response encoding, streaming and cancellation.
- Model profile: exact model ID/settings, supported capabilities, capacity and bounded timing class.
- Routing policy: approved processing recipients/upstreams and permitted behavior.

A DeepSeek key belongs to a direct DeepSeek connection. An OpenRouter connection uses an OpenRouter key; GLM/Qwen/Kimi do not each require another direct key when accessed through that connection. Do not silently submit a direct-provider key to an aggregator or probe multiple services to guess where it belongs.

Secret values remain in scoped backend/infrastructure secret custody, outside ContextPack, ordinary campaign persistence/history, logs and reports. Persist only credential references there; prohibit cross-user/organization credential substitution. Existing Mission/Broker authority still governs every release. Having a key does not widen a campaign's disclosure permissions.

### C. Reuse protocols and qualify model profiles proportionately

One OpenRouter integration can serve compatible GLM, Qwen, Kimi and other models. Providers sharing a qualified wire protocol may reuse its implementation while retaining their own recipient/authentication/profile bindings.

Do not require a Rust module, PRD, ADR or universal provider rewrite for every new model. Compatible model additions are validated profile/configuration changes. Reuse adapter contract/wire-fit tests; qualify model-specific settings, structured proposal behavior, context/output limits and timing evidence. A genuine protocol or trust-boundary change still needs its owning review.

A catalog entry establishes availability, not DuskWeave qualification. Catalog metadata cannot grant authority, silently activate a model or mutate an admitted run. Record requested and reported model/revision separately, with unavailable metadata labeled honestly.

DeepSeek remains the only presently available provider for real qualification. OpenRouter/direct alternatives remain unimplemented/unqualified until delivered. The first runtime slice uses DeepSeek plus a different local protocol fixture; that fixture proves interchangeability only. Adding real adapters follows without changing Terrain/action-authority semantics.

### D. Make OpenRouter routing enforce the agreed disclosure

Distinguish the connection service/aggregator, the model author/model ID and the actual upstream inference host. Naming only OpenRouter as the HTTP recipient is insufficient to express the processing boundary.

Campaign setup approves OpenRouter plus a bounded set of eligible upstreams and applicable data policies once; no intermediate operator approval is added. For the initial no-failover lane, resolve one qualified upstream endpoint from that set before admission and pin it with the model/routing profile for the run.

The future OpenRouter request must explicitly enforce:
- provider.only restricted to that selected upstream endpoint;
- provider.allow_fallbacks = false;
- provider.require_parameters = true;
- the approved data-collection/retention constraints, using supported controls and confirmed policy scope.

Use specific endpoint/region slugs when required by the approved boundary; a broad provider slug must not silently widen it. If the selected model, parameters, upstream or data policy cannot be satisfied, refuse/defer without trying another route. No auto/latest model router, cross-model fallback list or implicit request transform may change the approved model or mandatory context.

Record requested routing constraints and observed upstream/model when actually available; provider metadata is not independent proof. Apply defaults explicitly rather than relying on account defaults. The ban on ambient proxies does not ban an explicitly approved aggregator.

Future failover remains a separately delivered policy: already approved recipients, distinct admitted attempts, honest uncertain consumption, remaining budgets and zero target replay. Do not enable it here.

### E. Reduce duplication; retain one owner for each rule

M1 enabling architecture owns connection/model/routing semantics, disclosure, safe context and the distinction between recorded reasoning and currently eligible action.

ADR-016 owns timers, reservation/accounting, renewal/cancellation and termination attribution. Move/reduce duplicated numerical/lifecycle detail accordingly and use direct links. Keep historical measurements in the packet clearly historical. A short cross-reference is sufficient; do not copy a full normative rule into both documents.

Preserve accepted successor behavior: TTL-only expiry can leave a screened reasoning result recordable, while stale-dependent action receives zero release until owner requalification. Never rewrite the model's actual intent into a model-chosen defer. Withdrawal/window expiry/STOP/lease loss/incompatible owner revision/conflict/hold still stop affected work. Keep original evidence times, Terrain TTL, screening and database-only recovery.

Do not choose unmeasured successor timer numbers, enlarge leases, remove hard-stop protection or claim the historical exit 124 killer is known. Finite values and coherent inner/outer budgets remain prerequisites for the later runtime packet.

Retain cohesive documents without imposing a 600-line hard cap. Correct duplication now; no index/amendment document family or broad document reorganization. Size alone is neither a blocker nor proof of good architecture.

## Completion criteria and checks

The six files agree on ACCEPTED design status and owner/date/scope, while accurately showing documentation delivery and runtime qualification status. Tracking's next action is document correction/review/check completion, then preparation of the bounded runtime packet; it must not ask for this design approval again.

Retain original future regression requirements and add explicit observable assertions:
- Two users/organizations cannot substitute credential references; credentials never enter ordinary context/history.
- Compatible OpenRouter model profiles reuse the same protocol implementation without changing domain behavior.
- The serialized request contains the exact pinned model/upstream and routing controls. Unsatisfied parameters/policy or attempted substitution yields no alternate upstream/model request.
- Stored reasoning with a stale premise yields zero dependent effect release, including through the real backend admission path.
- Repeated inspect/recover yields zero provider/DNS replay.

These are requirements, not executed runtime tests. No live key is needed for this correction.

Run from D:/DuskWeave-ci-complexity:
- python scripts/check_links.py
- python scripts/check_structure.py
- git diff --check 56039deeb2281ac6149307920ebba9d8d2ae7d3a
- git status --short
- git diff --name-only 56039deeb2281ac6149307920ebba9d8d2ae7d3a

Include the currently untracked packet in the final six-file candidate; tracked diff output alone omits it. Verify all changed anchors and active-status claims. Follow unchanged required CI/delivery rules. Runtime compilation/coverage/McCabe execution is N/A for this documentation-only delta.

Apply existing QUALITY_BAR section 7 self-review and one distinct adversarial review by a different reviewer instance; do not claim self-review is that review. Review checks conformance to the already approved decisions, not another product-approval round.

Return exact file/check/review results and actual branch/head/delivery state. Do not auto-merge or continue into implementation. STOP only for a concrete authority/invariant conflict, incompatible relevant changes, unsafe preservation of unrelated work or material scope expansion. Advisory size and the already supplied owner acceptance are not STOP reasons.

Primary routing references already checked on 2026-10-22:
- [OpenRouter quickstart](https://openrouter.ai/docs/quickstart)
- [Provider routing and parameter enforcement](https://openrouter.ai/docs/guides/routing/provider-selection)
- [Model metadata](https://openrouter.ai/docs/guides/overview/models)


---

## Delivery record — corrected candidate (2026-10-22)

Executed on branch `docs/m1-provider-neutral-lifecycle` from verified base
`56039deeb2281ac6149307920ebba9d8d2ae7d3a` in `D:/DuskWeave-ci-complexity`;
other worktrees preserved, PR #114 merge reconciled as predecessor fact. R0
produced the PROPOSED amendment; R1 records the owner's explicit 2026-10-22
acceptance (conversation approval, Asia/Jakarta) and applies corrections A–E.

### Files changed vs BASE

| File | Change |
| --- | --- |
| `docs/contracts/M1-enabling-architecture.md` | New section 7 revision "Provider-neutral lifecycle and freshness revision (2026-10-22)", ACCEPTED status: four bindings (user/org connection, protocol implementation, model profile, routing policy), user-key connection flow, protocol reuse and proportionate model qualification, OpenRouter routing enforcement (`provider.only`, `allow_fallbacks=false`, `require_parameters=true`, approved upstream set + data policies, explicit defaults), freshness independence, immutable timing semantics, streaming screening, single-profile pinning and failover boundary; section 3 Provider cross-reference; metadata row. Numerical inventory and termination-attribution rules delegated to ADR-016 by link. Accepted 2026-10-19 text, profiles and seals unchanged. |
| `docs/adr/ADR-016-execution-broker.md` | Matching ACCEPTED revision under section 4a owning numerical lifecycle/accounting: profile-pinned accounting, usage normalization, failover accounting boundary, full timing inventory (20s/40s/45s/50s/60s/150s/90000ms plus 400ms/500ms/2s renewal contract), freshness-vs-lifecycle axes, termination attribution and database-only recovery; metadata row. |
| `docs/ENGINEERING_STATE.md` | Current Phase records PR #114 merge at `56039de`, the unqualified feedback outcome and the 2026-10-22 acceptance; active next action is this candidate's checks/review then a bounded runtime packet; PR #114 added to reconciled predecessor merges. |
| `docs/BUILD_ORDER.md` | Current permitted work repointed to PR #114 baseline and the accepted revision; next action corrected to checks/review then runtime packet; accepted 4a/section 7 exact-text note retained. |
| `docs/MVP_AND_DEFERRED_SCOPE.md` | Section 1.2 records the multi-provider direction, user-owned connection/key flow, OpenRouter routing enforcement, the run `4a4ce477` blocker, acceptance and the bounded delivery order; section 1.4 next-action updated. Section 1.3 unchanged — its supervision/diagnostics row already covers the attribution obligation. |
| `docs/workflows/DW-DESIGN-M1-PROVIDER-NEUTRAL-LIFECYCLE.md` | R0 assignment, R1 revision verbatim and this record. |

All runtime, SQL, tests, dependencies and CI files unchanged; runtime diff = 0.

### Checks executed (corrected candidate)

- `python scripts/check_links.py` — exit 0; 231 relative links OK, 0 broken
  links, 0 broken anchors (the renamed
  `#provider-neutral-lifecycle-and-freshness-revision-2026-10-22` anchor
  resolves in both documents; historical `proposed-*` anchors intact).
- `python scripts/check_structure.py` — exit 0; 0 conflict markers, 0 forbidden
  paths, 0 hygiene findings. Advisory warnings only; the enabling contract is
  now 1094 lines against the 600 markdown reference point — retained for
  cohesion per R1-E (size alone is not a blocker).
- `git diff --check 56039deeb2281ac6149307920ebba9d8d2ae7d3a` — clean.
- `git status --short` / `git diff --name-only <base>` — exactly five tracked
  documents plus this untracked packet: the six-file allowance.
- Active-status audit — no pending-acceptance or stale PROPOSED claims remain
  for the 2026-10-22 revision; 2026-10-19 PROPOSED headings/anchors preserved.
- Runtime TDD/wiring/coverage/McCabe: N/A — executable files unchanged.

### Added future assertions (R1, requirements not executed tests)

- Two users/organizations cannot substitute credential references; credentials
  never enter ordinary context/history.
- Compatible OpenRouter model profiles reuse the same protocol implementation
  without changing domain behavior.
- The serialized request contains the exact pinned model/upstream and routing
  controls; unsatisfied parameters/policy or attempted substitution yields no
  alternate upstream/model request.
- Stored reasoning with a stale premise yields zero dependent effect release,
  including through the real backend admission path.
- Repeated inspect/recover yields zero provider/DNS replay.

### Self-review (QUALITY_BAR section 7, document-level)

| ID | Disposition | Evidence |
| --- | --- | --- |
| Q1 Ownership | PASS | Contract owns connection/model/routing/disclosure and recorded-vs-eligible distinction; ADR-016 owns timers/accounting/renewal/termination attribution; no new manager, catalog framework or model. |
| Q2 Dependencies | PASS | Provider port seam unchanged; deployment composition supplies adapters; documents cross-link instead of duplicating normative rules (correction E). |
| Q3 Simplicity | PASS | One bounded revision per document; no index/amendment family, UI, service or tracking family added. |
| Q4 Single truth | PASS | Numeric inventory/lease values and termination rules now live only in ADR-016; contract links rather than copies. |
| Q5 Requirement fit | PASS | Corrections A–E applied: acceptance recorded with owner/date/scope, headings renamed, user-connection flow, protocol reuse, routing enforcement, dedup; historical acceptance/seals preserved. |
| Q6 Reachability | N/A | Document-only delta; link/anchor checks executed. |
| Q7 Failure integrity | PASS | Freshness separation keeps cancellation/lease/hold barriers; stale result recordable with zero stale-dependent release; uncertain consumption charged; zero replay/refund fabrication. |
| Q8 Trust and claims | PASS | Acceptance recorded as owner conversation approval, not executor claim; keys in scoped custody with reference-only persistence; aggregator routing pins approved upstream; catalog ≠ qualification; no runtime/merge/HEAD claims embedded. |

### Remaining prerequisites

Distinct adversarial review (separate reviewer instance) and delivery/CI per
repo rules; runtime packet with qualified finite timer values and a second
local protocol fixture; fresh disclosure/terms/window and key for the next
DeepSeek fixture demonstration; individual qualification for each later
provider (OpenAI/Claude/Mistral, then one OpenRouter adapter).

Status: **design ACCEPTED by owner (2026-10-22); corrected documentation
candidate delivered** — conformance review pending; no runtime readiness,
provider support, M1 seal or merge claimed.
