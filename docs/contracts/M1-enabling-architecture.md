# M1 Enabling Architecture, Owner Contracts and Sequencing

| Metadata | Value |
| --- | --- |
| Document | DW-DESIGN-M1-ENABLING-20261006 |
| Status | ACCEPTED — product owner, 2026-10-06; selected M1 architecture, owner contracts and sequencing |
| Revision | R1 — corrections reviewed; six scoped ADRs and this owner/sequencing outcome accepted on 2026-10-06. R2 — cognitive/generated-implementation amendment under the owner-approved 2026-10-07 direction, published as a candidate. R3 clarifies the owner-confirmed cognitive MVP scope of 2026-10-11; amendment exact-text status follows ENGINEERING_STATE.md |
| Inference amendment | Initial #51 profile and required R2/R3 refinements ACCEPTED by the owner on 2026-10-11 for this lane; merged at `35a31fd`; no blanket amendment acceptance or seal |
| Base | `086936d2903b8958c0e5bc69af9891d9710e092f` |
| Product input | [Accepted M1 product contract](M1-external-orientation-decision-loop.md), especially section 11 |
| Authority | Accepted PRD-000..010, ADR-001..008 and the six scoped M1 ADRs; no target permission |
| Delivery | One documentation-only DESIGN outcome; runtime diff = 0 |
| Preserved seals | DW-FOUNDATION-001 and DW-M0-001; no full-domain or M1 seal |
| Autonomous-run amendment | Section 7 is PROPOSED, 2026-10-19; engineering-selected runtime decisions, not owner acceptance or permission to execute |

## 1. Outcome and decision boundary

Resolve the selected external-orientation lane's evidence, sensitive egress, fixed capability,
execution and owner interfaces before asking the IDE to implement runtime.
The accepted product contract defines scope/cadence. On 2026-10-06, the product owner
accepted this separate R1 amendment, its dependency substitutions, owner contracts and
selected Linux deployment assumptions together with all six scoped ADRs. Publication,
merge and green CI are not the acceptance basis or deployment qualification.

No new PRD is authored: accepted PRD-000/001/002/006/007/008/010 provide governing semantics,
and the accepted M1 product contract selects their bounded behavior. This amendment does not
declare PRD-011..013 accepted or relax any higher product invariant.

## 2. Accepted bounded sequencing amendment

| Existing dependency | Accepted selected-lane disposition | Full-stage disposition |
| --- | --- | --- |
| Stage 4 evidence | Accepted minimum [ADR-009](../adr/ADR-009-evidence-immutability.md) continuity before retaining M1 claim support | Broader evidence/proof coverage remains unclaimed |
| ADR-011 depends on ADR-010 | For non-proof CT/DNS/HEAD only, bind [ADR-011](../adr/ADR-011-sensitive-data-barrier.md) to PRD-010, ADR-008/009 | ADR-010/012 required before proof/fingerprint/key use; custody requires its own coverage |
| Full Stage 5/domain seal before later engines | Implement only the owner/public contracts below, with M0 prerequisite already sealed | No DW-DOMAIN-001 or complete five-model claim |
| Stage 7 Terrain breadth | One sourced DNS routing-binding family and narrow CT/TLS/HTTP claims | Seven-layer Terrain/Key Terrain breadth deferred |
| Stage 8 footholds before Stage 9 paths | Derived external entry candidates need no foothold; no attempted/proven access transition is selected | Access/health, re-entry and validated transition paths retain full prerequisites |
| Stage 10 Objective/Trajectory | Reuse M0 Mission/Trajectory; add only required M1 decision/effect/evidence history contracts | Objective fulfillment engine deferred; declared mission priority is not objective proof |
| Stage 11 PRD-011/Stage 10 input | Bind [ADR-013](../adr/ADR-013-capability-contract.md) to accepted PRDs/M1 selected effects and narrow owner contracts | PRD-011 remains PLANNED for broader capability system |
| ADR-015 depends on ADR-014 | Static ADR-013 profiles plus the bounded generated-implementation candidate catalog (manifest, lifecycle, qualification, admission, promotion) replace a dynamic registry prerequisite for this lane | ADR-014 remains reserved until registry selection |
| Stage 12 PRD-012/013 | [ADR-015](../adr/ADR-015-execution-boundary.md), [ADR-016](../adr/ADR-016-execution-broker.md), [ADR-017](../adr/ADR-017-cross-process-contract.md) resolve this fixed Rust/Go deployment | PRD-012/013 remain PLANNED; native/polyglot ADR-018/019 not selected |
| Stage 13 general first-tool list | CT, DNS and fixed HEAD under the accepted authority above and an issued packet that qualifies isolation | Nmap/httpx/osquery/other tool breadth deferred |

Authoring order: ADR-009 -> ADR-011 -> ADR-013 -> ADR-015 -> ADR-016 -> ADR-017 -> these
owner interfaces. This is dependency-ordered DESIGN authoring, not runtime stage completion.
The owner accepted all six scoped ADRs, this amendment and owner contracts together
on 2026-10-06. Acceptance is limited to this selected lane, not full-stage completion.

The M0 non-acquisition exception ends at the M0 lane. The accepted M1 substitutions
apply only to the selected scope above. Deferred stages preserve their original dependencies
everywhere else. Neither this amendment nor future M1 seal proves full-stage seals.

## 3. Owner/public contracts

### Mission and current action policy

Mission owns an explicit versioned M1 permission attachment: approved purpose and priorities,
exercise mode, exact discovery/disclosure names or label-suffix host class, exact contact
hosts/classes, exclusions, vantage/resolver/provider disclosure, permitted paths, operating
window and positive finite campaign totals for episodes/provider/DNS/follow-up/TCP/HEAD
and concurrency. Episode ceilings cannot reset those totals. An operator authorization is the input; CT/DNS do not create it.
Store accepted attachment and revision/history through Mission's existing bounded authority
boundary; older M0 registration/eligibility semantics stay unchanged.

Discovery/disclosure eligibility and contact authority are separate. A name can be a safe hint
while ineligible for contact. CNAME, certificate/IP sharing and redirects never inherit rights.
HTTPS v1 uses port 443, approved path, one selected current binding and original hostname/SNI.
No arbitrary IP probing. Non-global/special-use destinations default deny; a later explicitly
authorized fixture/lab profile must name exact permitted endpoints and cannot enable a CIDR
exception for a client run. Address classification uses a pinned special-use policy, including
IPv4-mapped forms; unsupported policy version blocks admission.
Current authority rechecks required owner premises and M1 policy at every effect start.

### Terrain Observation, material and claim

Terrain receives only ADR-011/017 admitted CT/DNS/HTTPS projections correlated to an issued
attempt or separately admitted safe operator input. Operator input retains its source label
and cannot stand in for DuskWeave acquisition in milestone qualification.
Terrain owns Observation identities, safe technical material, per-claim EvidenceEnvelope
evaluation, environmental claim revision and accepted routing relationships.
The evidence functions do not become a sixth model or publish sibling transitions.

A claim key contains campaign, subject, claim family and vantage/resolver where applicable.
Separate observation identity distinguishes repeated perceptions of that claim.
Routing binding means: declared resolver returned this host-to-address binding at time T,
with bounded reported alias lineage and limits. It means no organizational ownership,
same backend, origin discovery, trust grant or universal reachability.

Admission supplies attributable source family, purpose/mode, vantage, perception/effect
time and uncertainty, method/profile/version, completeness, safe request/result technical
basis and original attempt/proposal references. Terrain checks current claim revision,
source eligibility, contrary material, claim-relative burden and required fields.
Outcomes: owner-qualified claim/view at stated status; duplicate; pending predecessor;
integrity conflict; insufficient basis; withheld/ineligible. None grants dispatch.

M1 direct CT/DNS/TLS/HTTP perceptions can support narrow Tier 1 OBSERVED claims.
No claim is automatically CORROBORATED. Actual route/trust/global service claims requiring
Tier 2/3 remain unmet. External pre-access vantage does not imply PROVISIONAL access origin.
Timeout/HEAD rejection/redirect preserves limited observation and uncertainty.
A DNS binding expires under ADR-013's returned TTL/window. Different RRsets perceived at
different times may both be historically true; rotation is not automatically contradiction.
A newer qualified binding revises the current routing premise with expected revision and
invalidates proposals dependent on the prior binding. Material that disputes the same
declared subject/vantage/time applicability suspends affected use until owner reconciliation;
arrival order alone cannot resolve that conflict. Returned TTL may be remaining cache
lifetime, and TTL zero is valid but cannot support a later separate R1 contact.

Owner acceptance uses expected revision and a short PostgreSQL transaction for safe history,
current representation and TerrainChanged publication obligation under ADR-009.
Public immutable view: campaign, revision/frontier, evaluation time, selected qualified claims,
evidence references, status/tier, freshness and unresolved/conflict limitations.
Safe CT hints may contain new purpose-admitted names; policy class is distinct from the
bounded acquired list. Terrain assigns stable hint refs and accepts only their narrow
source/time meaning. Withheld aliases or truncated responses cannot create complete routing.
Public commands supply admitted material, request claim reconsideration or append correction;
there is no generic setter or externally supplied accepted-fact flag.

### Pathing candidate ownership and invalidation

Pathing owns only rebuildable external entry candidate views, keyed by campaign + approved
host/port/path reference. Premises name exact Terrain claim/material revisions and Mission
policy/priority references; capability availability comes from the static profile contract.
No sibling repository or mutable aggregate is supplied. Missing Access/Objective engine is
not replaced by fictitious access/progress state.

Candidate disposition distinguishes a supported bounded check, observe-routing-needed,
unresolved-response premise, insufficient/stale/conflicting dependency, contact-prohibited,
and defer/no-action. It is an analytical projection, never a proven path or scope grant.
A safe retained historical response cannot restore a currently unusable binding.

TerrainChanged requests invalidation/reconsideration for declared dependencies only.
At-use Pathing reads current owner views and recomputes time eligibility even without a delta.
A lagged/unsupported/conflicting view is unavailable for consequential use. Projection update
and consumer completion are durable together only when persistence is actually needed;
otherwise rebuild directly from current owner views and do not invent a primary path store.
Each required invalidation obligation remains visible until satisfied or the consumer has
verified current owner premises. Mission withdrawal stops execution regardless of projection lag.

### Cognitive reasoner and bounded context

The M1 reasoner is a short-lived cognitive worker: it proposes hypotheses, compares
alternatives, identifies missing information, chooses the next proposed action and
adapts after outcomes. Deterministic mechanisms retain authorization, bounds, schema
validation, current-use checks, execution accounting and owner-specific reconciliation;
they may reject a proposal but do not replace strategy with a mandatory action sequence
or a fixed ranking the model only narrates. The prior deterministic selector remains a
test control or explicitly labeled non-cognitive operating mode.

Input is a purpose-scoped immutable ContextPack: authorized mission question and limits,
eligible owner views with revisions/frontiers, source/time/completeness, uncertainty and
conflicting claims, eligible capability manifests, remaining budgets, pending/UNKNOWN
effects and relevant campaign-local feedback. Pack capacity follows
[ADR-016's inference profile](../adr/ADR-016-execution-broker.md#initial-inference-accounting-profile-scoped-acceptance);
no raw provider input, private aggregates, execution handles, privileged defender feed
or all-campaign context.
Include references and explicit omission reasons; visible truncation must not imply that
omitted evidence does not exist. The worker may request another bounded safe owner view
before choosing an effect; such a read does not recollect target data, and any requested
new observation is a new proposal through current authority. No raw content, secrets,
unrestricted repositories, defender oracle or privileged evaluator answers enter the
ContextPack.

The per-pack candidate bound applies to one ContextPack, not the campaign's complete set of
hypotheses or assets. A bounded summary/page can expose population counts, coverage,
grouping basis and explicit omissions, only as supported by the owner view. Additional
pages name their owner revision/frontier; incompatible or changed premises require a fresh
eligible pack, never an invisible mixture. Reads, pages, bytes and inference remain finite
under Mission/deployment limits; missing limits disable the operation. A view read does
not collect new target data.

Terrain owns sourced environmental attributes/groupings; Pathing owns derived transitions
and their premises; cognition chooses mission relevance. Deterministic eligibility checks
and transparent sampling may prepare a view, but cannot silently choose strategy. Naming
patterns remain hypotheses, and a provider/origin label needs its own evidence.
Macro-to-micro navigation is optional, not a required two-stage playbook; current R1
acquisition/intake ceilings are unchanged.

A proposal names the question, relevant evidence references, an alternative considered,
expected observation, falsifying observation, uncertainty and the proposed next
operation — validate, observe more, synthesize a candidate implementation, retain, defer
or stop. Concise decision rationale is recorded, not private model chain-of-thought; free
text is untrusted explanation and cannot create an endpoint, accepted fact or execution
handle. Comparison still weighs declared mission priority, an action's ability to resolve
the named unmet question, supported current premises and permitted cost — as material the
reasoner must reason over, not a hidden selector. Output remains one typed proposal or
no-action disposition per episode step; it cannot widen authority, accept model state or
manufacture missing evidence. Expected/disconfirming outcomes and conditions for changing
the choice precede dispatch.

The proposal classes above are intent categories, distinct from ADR-013's externally
visible effect profiles. Parameters select only an admitted profile/version/schema and
eligible references. The reasoner may describe conditional future steps or a missing
capability, but cannot authorize their execution as a batch. DNS MX and TLS SAN expansion
are not implied by current R1 A/IN and HTTPS HEAD profiles; new effect/output semantics
require their owning decision. Synthesis may introduce novel internal computation under
the admitted interfaces, not arbitrary new I/O.

After an owner-qualified outcome, the reasoner revises beliefs and unresolved questions
before proposing again. Out-of-envelope redirect leaves the route premise unresolved; an
informative permitted B check may beat repetition of A. Sufficient in-envelope A response
can legitimately retain A; timeout remains inconclusive. 401/403/unsupported HEAD does
not permanently refute the asset or identify a defense. Repeated equivalent proposals
without new evidence or a changed premise produce a visible no-progress disposition; there
is no automatic abandonment for one failed route and no endless retry chasing a success
label. Reflections are revisable hypotheses stored through Trajectory's decision/history
responsibility with a bounded retrieval view, never promoted domain facts and never
silently retrieved as current advice; applicability is rechecked before reuse.

Inference is a replaceable adapter, not a model-specific core architecture; deployment
selects the exact model/configuration and approved placement, recording model,
provider/runtime revision where available, prompt/template and sampling configuration
with each decision. External inference requires permission for the specific safe fields
and recipient under terms compatible with the engagement; otherwise use an eligible local
deployment or defer cognition — model switching is not an undeclared disclosure fallback.
Provider timeout, malformed output and schema-repair attempts consume inference budget,
and repair cannot dispatch; provider loss produces a visible deferred/failed state, not a
silent fallback or non-cognitive success claim. Finite cognition/build limits follow
ADR-016; cognitive sessions may span episodes while every effect still needs a fresh
eligible episode and current premises. Provider- or target-derived text remains
source-labeled untrusted input and cannot change system policy, authorize tools or
rewrite acceptance criteria; generated source, reflections, traces and report narration
obey the same INV-005 sensitive-data boundary as every other surface.

#### Initial inference contracts (scoped acceptance)

The owner accepted the initial #51 profile and required R2/R3 refinements for this
lane on 2026-10-11; unrelated amendment text keeps its status. This is preparation
for actual LLM reasoning, not completion of cognitive MVP, acquisition or execution.
Mission owns disclosure consent/limits; Broker owns reservation/consumption/cancellation;
Cognition owns safe context assembly, proposal validation and decision publication;
Trajectory owns accepted history/inspection/recovery. Use narrow public views/ports,
never a CognitiveManager, GlobalContext, new operational model or sibling mutable import.

**Mission attachment — `m1_inference_permission_v1` and `v2`.** Required fields: attachment identity,
version (`v1`/`v2`), engagement/campaign, operator/authority references, resulting current
Mission revision, goal/purpose reference, exercise mode, approved recipient, profile version
(`deepseek-responses-v1` or `deepseek-responses-envelope32-v1`), safe field classes, valid UTC window,
finite campaign/per-operation limits and provider term approval. Native `v2` requires the fixed ADR-016
profile, envelope depth 32, and `admitted_repair_fields_codes`; ContextPack/proposal depth stays 8.
INITIAL-CONSENT co-creates this on new registration (absent Mission revision 0 -> 1); later amendments
require their own lane. Existing campaigns are not silently upgraded. Missing attachment disables inference;
existing crt.sh disclosure grants no inference rights. Consent is not an API key or dispatch authority;
environment values, caller JSON or prepared sessions cannot substitute for the Mission owner view.

Safe field classes are closed: mission question/constraints; scoped opaque references;
owner-qualified non-sensitive claim summaries with source/time/status; admitted operator
premises labeled as such; manifest availability; omissions/UNKNOWN dispositions; capacity;
and admitted proposal fields/static validation codes for conditional repair. Each actual
field must be both safe and permitted by consent. No class admits raw client content,
secrets, private chain-of-thought or privileged defender context in blind mode. Missing
required disclosure classes defers; redact/drop optional fields only with visible omissions.
Before every generation and repair, require current Mission authority, unchanged revision,
matching campaign/purpose/mode/recipient/profile, valid window, field eligibility and Broker
capacity. Withdrawal, expiry, unsupported profile or unavailable authority blocks the call;
ADR-016 owns the send/stop race, reservation and cancellation semantics.

**Provider — `deepseek-responses-v1` and `deepseek-responses-envelope32-v1`.** Select only HTTPS POST to
`https://api.deepseek.com/responses`, model `deepseek-flash`, text-only non-streaming
Responses (`stream=false`), `reasoning={"effort":"high"}`, no tools (`tools=[]`,
`tool_choice="none"`), and `text.format={"type":"json_schema",
"name":"m1_inference_proposal_v1","schema":...}` with the proposal schema below.
`max_output_tokens` and all numerical limits are canonical in ADR-016, not SDK defaults.
Use explicit versioned instructions; omit temperature/top_p and record those omissions.
No image/file input, conversation continuation, redirects, automatic transport retries,
ambient proxies, provider fallback or model-switch fallback. Normal certificate/hostname
validation is required. The provider API key stays in adapter-only infrastructure secret
configuration, outside the pack, request provenance, SQL, diagnostics and receipt.

Official [Responses reference](https://api-docs.deepseek.com/api/create-response) and
[compatibility guide](https://api-docs.deepseek.com/guides/responses_api), checked 2026-10-11,
explicitly document this model, endpoint, text input/non-streaming output, JSON Schema
text.format, high effort and reasoning-inclusive max_output_tokens. This is not an OpenAI
SDK-equivalence claim. Unsupported parameters may be silently ignored: send only this
selected supported subset, and locally validate every output. The
[model reference](https://api-docs.deepseek.com/quick_start/pricing) currently maps the alias
to DeepSeek-V4.1-Flash; hidden provider changes remain a reproducibility limit. Record
requested and reported model/configuration separately, profile/schema/template versions,
adapter release, and provider revision/fingerprint when actually supplied, otherwise
unavailable. Do not manufacture a fingerprint or infer identical output on replay.
The API's stateless/store-false description is not zero-retention certification; automatic
provider caching and engagement-compatible terms still require disclosure approval.
[Keep-alive documentation](https://api-docs.deepseek.com/quick_start/rate_limit) allows blank
lines while waiting: they count toward the envelope and never extend the deadline.

**Context — `m1_context_pack_v1`.** An immutable strict UTF-8 JSON object assembled locally
from eligible owner contracts, not a caller-supplied accepted view. Required fields:
- `version`, engagement/campaign, decision identity, permission identity, Mission revision,
  purpose/goal, scope and exercise mode bind the question and its authority provenance.
- `question` is the admitted mission question; `evaluated_at` states the evaluation time.
  `owner_views` names each owner, availability, revision/frontier and evaluation time;
  unavailable Terrain/Pathing/Access/Objective stays unavailable, never empty-as-complete.
- `references` assigns pack-local IDs to scoped owner/entity/revision references, with
  safe meaning, source lineage, perception time/time uncertainty, epistemic status/tier,
  freshness and completeness. Supplied JSON or a name is not source qualification.
- `candidates` references eligible premises, not a forced ranked A/B answer. Safe operator
  premises are separately admitted and source-labeled; they do not qualify acquisition.
- `omissions` gives unavailable/withheld/stale/truncated reasons and supported coverage
  only; `pending_unknown` names unresolved work and limitations, not fabricated outcomes.
  For the four always-unloaded owner views (`foothold_graph`, `attack_path_view`,
  `objective_state`, `campaign_trajectory`), per-entry `reason` is omitted and
  `pending_unknown` records `other_owner_views_not_loaded_by_profile` for those four
  owners; Terrain retains its specific unavailable reason, operator references and
  candidates retain their explicit premise status/freshness/completeness, and
  `remaining_capacity` retains `reservation_granted = false` and
  `shared_call_clock = not_started`.
- `manifests` lists admitted profile/version/digest and actual availability, never inferred
  from an ADR alone. `remaining_capacity` is Broker's safe reservation/remainder view,
  distinguishing bytes, tokens, calls and time; it is not authority to consume them.
A missing Mission view defers the decision. Missing other views permits honest limited
reasoning, not manufactured access, path, environmental or objective claims. When the
validated, disclosure-permitted loaded Terrain view has no fresh usable `Observed` claim
for the originally selected candidate prefix under consent, existing unavailable/stale/
withheld/conflicted/limited-reasoning behavior is preserved (`context_size_deferred` if
mandatory metadata itself cannot fit). When at least one such eligible claim exists for
that prefix, successful preparation must retain at least one eligible claim, a linked
retained candidate, its pack reference, and its `TerrainDependency`; byte fitting drops
optional unlinked operator candidates first and may omit surplus claims, but never removes
the final supported premise or its last linked candidate, deferring instead with
`context_bytes_insufficient` before inference reservation or provider work. Another safe
view request is `observe_more` intent: obtaining changed context needs a new decision and
reservation; repair cannot expand the pack. Questions/alternatives remain open-ended.

**Proposal — `m1_inference_proposal_v1`.** Require a strict JSON object with `version` equal
to that contract identifier; the question/decision binding comes from the local pack, not
a model-selected identity. Remaining required fields are
`hypothesis`, `considered_alternative`, `references` (pack-local IDs), `expected_observation`,
`falsifying_observation`, `uncertainty`, `rationale`, and `next_intent`. Each text field is
nonempty bounded safe explanation, not private reasoning; uncertainty names missing or
conflicting support, and falsification states what would change the hypothesis.
`next_intent` has exactly one category: `validate`, `observe_more`,
`synthesize_candidate_implementation`, `retain`, `defer` or `stop`; it carries eligible
reference IDs and an optional manifest reference, not a command, arbitrary endpoint or
execution handle. Only `observe_more` may additionally carry one typed `view_request`
naming the owner view (`cyber_terrain`, `foothold_graph`, `attack_path_view`,
`objective_state` or `campaign_trajectory`) and bounded pack-local selectors; deterministic
ContextPack assembly decides admission, so the request stays explanation, not a fetch.
Missing-capability/conditional future intent remains explanation and cannot dispatch a
batch or grant synthesis/build permission. Empty reference lists may support an honest
defer/stop, never an unsupported factual premise.
The schema uses required fields, closed objects (`additionalProperties=false`), string/
array/object types and closed category/version values. Local byte/nesting/collection
bounds follow ADR-016 even if provider schema enforcement differs. Reject duplicate or
unknown keys, malformed/trailing JSON, wrong types/version, empty required text and extra
proposals structurally. Reject invented/cross-campaign/stale references, digest mismatch,
mode/provenance laundering, widened scope, unsupported manifest or current-use failure
semantically. Recheck required owner premises and Mission authority before current-use
validation; unavailable authority cannot become a validated current proposal. A later
invalidation preserves the historical record but blocks dependent use. Schema validity
is neither truth nor permission; even valid hypotheses require owner reconciliation.

**Local request binding — `dw-inference-binding-v1`.** Let P be the exact admitted immutable
pack UTF-8 bytes; B the exact serialized HTTP JSON entity sent; C the immutable UTF-8 JSON
configuration bytes recording recipient/method/content-type, profile/schema/template
versions and their exact schema/instruction text, requested model/settings (including
omissions), adapter release and effective transport/resource settings. Preserve admitted
P/C/B with the local decision provenance; do not reserialize them during verification.
Digest is lower-case hex SHA-256 of ASCII `dw-inference-binding-v1` followed by a zero byte,
then length(P), P, length(C), C, length(B), B; each length is an unsigned 64-bit big-endian
byte count. C/B exclude credentials and contain no digest field, avoiding circular binding.
Each call has its own binding, including actual repair material; P stays byte-identical.
Transport must send B unchanged under C, and validation compares against the locally
reserved binding, never model-supplied digest text. A provider digest/identity field is
untrusted extra output. No digest exists without retained bound safe bytes, and a digest
proves neither source truth, provider identity nor current permission.

The adapter accepts only a complete bounded response envelope and completed assistant
message carrying proposal text; reasoning items are discarded without logging, storage,
repair forwarding or operator exposure. Unexpected tools, unsupported item/protocol,
failed/incomplete/truncated responses, error bodies and transport failures yield safe
categories, never partial cognitive success. Raw response/error/chain-of-thought bytes
stay transient inside the adapter admission boundary; only admitted safe proposal fields,
usage/provenance and fixed error categories may leave. Size-bounded free text is not
inherently safe. If safe admission cannot be established, reject without echo or retention.
Repair follows ADR-016 only: same question, exact pack and references plus static errors
and optionally bounded independently admitted proposal fields labeled untrusted. It never
resends arbitrary raw response or repairs authority/scope/provenance failure. Every
repaired proposal passes the same complete structural, reference and current-use checks.

Provider failure remains a visible cognitive failure/defer/unknown. A non-cognitive
selector requires an explicitly selected and qualified operating mode with separate
provenance and current admission; failure alone never activates it or satisfies cognitive
MVP acceptance.

### Trajectory and narrow consumers

Trajectory owns accepted decision, authorization disposition, reserved attempt, possible
effect-start, safe result/rejection/unknown, owner acceptance/correction and next-decision history.
Each logical record names its accountable producer and scoped original identity.
Required histories/outbox/inbox use ADR-003/005; final receipts distinguish durable/pending/
unknown history, not fictional global acceptance.

| Published input | Consumer rights | Required consequence |
| --- | --- | --- |
| TerrainChanged | Pathing invalidates/rebuilds its projection; Trajectory records acceptance/correction | Durable completion or current-premise fallback before dependent contact |
| ReasoningDecision | Trajectory records alternatives/selection/expected evidence | Required decision history before effect start |
| ExecutionDisposition | Trajectory records actual/unknown limits; Terrain considers admitted material | History and owner admission remain separate; no direct Terrain write |
| Mission stop/change | Broker fences/cancels; Pathing reconsiders; Trajectory records | Stop via direct current authority path, never await normal backlog |

The same history responsibilities carry cognitive records in one link chain: mission
revision -> ContextPack references -> decision -> artifact version and qualification ->
admission -> attempt/effect disposition -> observation -> owner claim. Persist safe links
and structured rationale with existing append-only and expected-revision semantics.
Corrections append and identify what they correct; a newer model, artifact or hypothesis
cannot rewrite historical evidence or renew expired premises.

Operator inspection reads safe owner/history views with purpose/mode restrictions.
Show selected/alternative candidates and the decision question, actual/unknown effect,
owner-qualified result, phase/failure code, safe counters/times, source completeness,
unmet dependency and next permitted step. One direct eligible observation may support its
declared burden; do not require a new contact before showing an honest limited observation.
Additional verification is claim-specific. Inspection is not client-proof release.
No read/reference fetches raw capture. History and projection replay call no capability.
Composition retains only narrow application ports; no object acquires all five models.

#### Initial decision publication and inspection — scoped acceptance

Future production flow: existing CLI dispatch under CommandGuard -> current Mission public
view -> Cognition pack assembly -> Broker durable consent/quota admission -> replaceable
inference adapter -> Cognition complete proposal validation -> durable decision/outbox ->
Trajectory consumer -> bounded receipt. No stage imports a sibling mutable aggregate.
The first delivery adds no target dispatch, Terrain/Pathing/Access/Objective mutation or synthesis run.

A core-allocated decision operation identity is scoped by engagement/campaign and producer
`cognition`; its generation/repair call identities are that scope plus decision and call
role, never provider-assigned IDs. Broker owns the call ledger; Cognition owns immutable
`ReasoningDecision` v1 publication and its validation disposition. Each event has a stable
core event ID, decision ID, Mission/permission references, pack/request bindings, call
references, occurrence/recording times, source/mode and admitted proposal or safe failure
category. Validation and publication acceptance are separate from provider consumption.
Cognition's accepted decision and required `trajectory.reasoning_history.v1` obligation
commit together under ADR-003/005; no best-effort callback after a standalone save.
Trajectory verifies producer provenance through the wired owner port and committed source,
scope/version/identity, Mission predecessor and bound bytes, then atomically appends accepted
history and consumer completion. Historical admission checks the recorded basis, not a new
inference or renewed consent; current use remains separately checked. Pending predecessor
or unavailable publication stays pending/unknown, not completed or a validation rollback.
Same scoped identity/content redelivers idempotently; changed question/pack/config/proposal
under the identity is an integrity conflict, not last-writer-wins. Anomaly dominates a prior
completed row; resolution appends verified correction under ADR-003, without rewriting it.

Receipt/inspection `m1_inference_receipt_v1` carries scoped decision/call/event IDs, proposal
validation disposition, publication and consumer status, reservation/consumption states,
requested/reported configuration, safe counters/times and a fixed recovery category.
Distinguish validated proposal + completed history; proposal + pending/unknown history;
deferred/failed inference; uncertain provider consumption; and integrity conflict. These
axes may coexist and must not collapse to success. All receipts say dispatch is not granted.
Inspection reads owner/Trajectory views; explicit bounded recovery may redeliver committed
safe obligations but never invoke inference, repair, refill quota or reinterpret an absent
record as a known no-send. If validation happened but decision commit is unknown, recovery
reads the original identity; if safe proposal persistence never completed, leave it failed/
unknown rather than regenerate. Fresh processes must reconstruct these statuses from durable
records, not receipt text. Accepted history retains only admitted safe proposal/provenance,
not API keys, raw responses, private chain-of-thought or raw client content.

### Minimum report

The minimum M1 deliverable is one machine-readable report and one human-readable
engagement summary derived from the same pinned report snapshot: scope/window/mode/
vantage, mission question, supported observations/findings, evidence references and claim
tier, UNKNOWN/failed/deferred outcomes, coverage omissions, contradictions, limitations
and next permitted recommendations. Empty supported findings is a valid result. Owner
revisions and a history cutoff are pinned; if a consistent snapshot cannot be obtained,
defer final export or label it partial — do not imply atomicity. Report generation
performs no target I/O, capability dispatch or mandatory recollection. LLM narration may
explain source-backed facts, but structured claims and references are checked against the
pinned projection; unsupported sentences are omitted or labeled as hypotheses. Corrections
create a new report revision linked to the old one. Only eligible safe material is
exported — no raw capture or credential values. A HEAD response is not compromise, a
heuristic is not confirmed impact, and an orientation result is not objective completion
or production readiness. The report is a read-only projection of eligible owner
state/history, not a new evidence owner.

## 4. Fixed execution and sensitive decisions

ADR-013 owns numerical limits and fixed profile versions for its execution scope; the
scoped accepted ADR-016 inference profile owns inference accounting numbers. ADR-015 selects
qualified Linux
worker isolation; ADR-016 owns current authority/effect admission and cancellation;
ADR-017 owns private framing and phase binding; ADR-011 owns pre-core safe egress and
honest disposal/contamination. They are concrete accepted decisions, not choices delegated
to the IDE. Platform/deployment qualification is UNVERIFIED until actually demonstrated.
Existing Windows M0 qualification is reused only for unchanged behavior.

The owner-approved 2026-10-08 [infrastructure configuration contract](../RUN_DATABASE_CONFIG.md)
requires `DW_DATABASE_CONFIG_MODE=file` for Linux effect-enabled M1 launch.
The core's protected DSN file and private provisioning directory are outside
capture/exporter/build/generated-worker access. Workers receive neither its value,
selection/path environment, descriptor nor mount; the launcher must enforce and
qualify that separation under ADR-015. `env-local` is only Windows/Linux M0
compatibility, not an M1 deployment profile. This configuration FIX implements no
launcher or worker isolation, accepts no unrelated R2 text and qualifies no target
effect. Existing `m1-policy-check` remains an informational name-policy snapshot.

The selected threat model covers hostile network input and capture attempts at arbitrary
output through a distinct isolated egress process. The trusted exporter/launcher and kernel
remain TCB; digest does not prove absence of runtime compromise. Host administrator/kernel/
hypervisor compromise and forensic RAM recovery remain outside the initial model. No
forensic erasure, tamper-proof administrator audit, exactly-once external effect or complete
discovery claim is made. Safe diagnostic categories do not imply raw debug retention.

R1 selects ct-existing-v2 and dns-routing-v2 with safe dynamic hints, parent-selected UDP/TCP
and counted new DNS follow-ups; https-head-v1 stays fixed. All TCP <=5/episode, DNS <=10,
HEAD <=2; campaign totals remain finite and durable. Start consumption <=1s, communication
lease <=2s and renewal interval 500ms are distinct, deadline-clamped selected assumptions.
Qualification is future evidence, not a benchmark or production throughput claim.
Stop-only Mission writer plus a monotonic host STOP latch and shared host grant/STOP
section covers Broker-unreachable withdrawal; pending/unknown receipt and no-auto-resume
cover unavailable persistence. No SQL lock spans network or host-control waiting.

R2 adds the generated-implementation lane: Go candidate source bundles are compiled by a
trusted build adapter under ADR-015's isolated build boundary, qualified against the
fixed effect vocabulary under ADR-013, admitted per campaign, and dispatched only through
typed nested effect requests under ADR-016's unchanged fence, accounting and cancellation
rules. A generated worker never becomes the raw-facing capture process or trusted
exporter. Inference is a replaceable adapter with deployment-selected model/placement
under ADR-004; its disclosure is separate from target contact authority. Acceptance of
this architecture is not deployment readiness: toolchain image, model selection, limit
values and generated-worker containment remain UNVERIFIED until measured qualification.

## 5. Deferred triggers and STOP

- Proof/client release: accept ADR-010/012 and applicable proof-boundary coverage first.
- Credentials/authentication: accept PRD-010 custody mechanism and secret-enabled execution
  coverage before collecting, retaining, resolving or presenting any value.
- Registry/native helper/extra language/Observer: select a concrete capability and obtain
  its owning authority first. R2 selects the replaceable inference adapter for bounded
  cognition only; a different provider class, privileged feed or unbounded use still
  requires its owning decision. A provider under unchanged boundaries needs a source
  profile; changed effects/trust/sensitivity/owner rights require the owning decision.
- Native Windows acquisition or multi-host writers: accept and qualify equivalent isolation/
  authority coordination; this single-host Linux decision does not silently cover them.
- Unqualified egress/disposal, uncoordinated authority writer, unsupported version, unavailable
  current premises, ambiguous possible effect or required-history gap blocks affected use.
- No new gate, service provisioning, per-slice workspace, full M0 rerun or milestone seal.

### R1 disposition of the reviewed additions

R1 includes purpose-admitted new hints, safe diagnostics, explicit DNS transport/follow-up,
new admitted address alternatives, campaign budget/recovery, DNS rotation, stop-only fallback
and pre-core egress correction. Revision authorization initially left acceptance pending;
the owner subsequently accepted the scoped R1 architecture and dependency amendment
on 2026-10-06. Deployment qualification and runtime delivery remain unverified.

Provider alternatives, GET/body signatures, dangling-DNS/takeover validation, scheduled
discovery, batching/concurrency, warm cross-attempt workers and client-proof reporting remain
demand-driven backlog. They are not selected runtime features or additional milestones here.
Provider profile additions under unchanged boundaries need no new PRD/ADR by default;
new effects, secret presentation, trust/disposal or disclosure rights change their owning
contract first. Provider integration credentials are distinct from target campaign custody.
Google CT log enumeration is not assumed to be domain search; no Censys/free-tier access,
provider SLA or takeover proof is invented. Header/title whitelist and body truncation do
not admit sensitivity automatically. A dangling alias alone does not prove takeover.

The accepted post-M1 access pull stays registered. Broader sources/protocols are added for
a concrete unresolved mission need; no mandatory re-observe gate, universal raw capture,
full-domain scaffold, scheduler or comparative superiority claim is introduced.

## 6. Completion and next packet

R1 delivered six ACCEPTED scoped ADRs and the accepted owner/sequencing contract.
R2 was published under the owner-approved 2026-10-07 cognitive direction. Published R3
records scope/phase confirmation of 2026-10-11 and the detailed cognitive MVP clarification.
The owner accepted the initial #51 inference profile and its required R2/R3 refinements
for this lane on 2026-10-11; unrelated amendment text remains pending exact-text acceptance.
M0 evidence and seals are unchanged.

`DW-IMPLEMENT-M1-INITIAL-INFERENCE-CONSENT` merged through PR #52 at `1ea0ea7`.
The current runtime packet is `DW-IMPLEMENT-M1-INFERENCE-RESERVATION`: a
Broker-owned pre-send capacity ledger — durable reserve/recover/cancel with
campaign/operation accounting and session/inference mutual exclusion.
Later consent amendment/rebinding and real-inference ContextPack/proposal/history,
bounded repair and recovery remain required. These are partial enabling
delivery, not cognitive MVP completion. Subsequent connected outcomes must provide
owner-qualified action feedback and LLM belief/question revision, bounded applicable
campaign-local retrieval, isolated synthesis/admission and report completion within M1.
Their exact order follows real dependencies and measurement, not a compulsory tool
sequence or fixed PR count. Each IMPLEMENT packet needs its own accepted base and
complete observable entrypoint/consumer map.

Engineering delivery follows the canonical policy in [QUALITY_BAR.md](../../QUALITY_BAR.md). Each later packet requires a measured complete path
and exact map; full provider-chain fit/deployment remain UNVERIFIED.
No fixed PR count, cognitive MVP completion, provider call or seal is granted here.

### Required future inference qualification

These assertions are future requirements, not tests executed by this DESIGN packet.
Use deterministic provider fixtures, real restricted PostgreSQL and the real CLI/consumer;
separately qualify actual DeepSeek behavior only with authorized safe disclosure inputs.
- Valid completed proposal produces one durable decision and one completed Trajectory item;
  one eligible schema repair keeps P/references unchanged and yields separately charged calls.
- Malformed/empty completed output uses safe error codes only; incomplete/truncated output
  never repairs. Invalid repair exhausts capacity without a selector fallback or dispatch.
- Forged references, mismatched bound bytes/digest, cross-campaign identity reuse and unsafe/
  privileged-source text reject; no prohibited sentinel reaches prompts, SQL or inspection.
- Missing consent, revised Mission, expiry, withdrawal and unavailable authority block each
  generation/repair release; race withdrawal against release and prove owned cancellation.
- Duplicate/concurrent identity makes no additional call; conflicting content yields anomaly.
  Insufficient campaign capacity prevents generation; contention cannot overspend remainder.
- Restart after may-send/lost response retains uncertain consumption and counts zero replay;
  unused reserved repair stays distinguishable from sent work, never resumed by recovery.
- Commit reservation durably, assert it through a fresh restricted connection, then lose the
  physical COMMIT ACK: caller sends nothing; recovery preserves identity/capacity. A fault
  before commit or a mock returning an error is not this qualification.
- Inject decision publication/consumer failures and consumer commit/ACK loss; fresh-context
  recovery completes only durable safe obligations once, or reports pending/unknown/conflict,
  with zero provider calls and no regeneration of a lost uncommitted proposal.
- Oversize headers/body/keep-alives, stalled connection and slow completed output respect
  ADR-016 byte/deadline bounds; cancel/parent loss leaves honest uncertain disposition.
- Exercise the actual CLI -> Mission -> pack -> Broker -> provider adapter -> validated
  publication -> Trajectory -> inspection/recovery path; a unit-only export fails reachability.
  Record candidate/platform/configuration and actual call/history counts. Fixture success
  is not provider availability, storage/cancellation qualification on another platform,
  action feedback, synthesis, reporting, cognitive MVP completion or an M1 seal.

## 7. Proposed autonomous-run amendment (2026-10-19)

**Status: PROPOSED.** These are the engineering-selected decisions for the
owner-requested first autonomous outcome, against merged PR #70 (`535eab5`).
They do not alter acceptance of sections 1–6, existing consent, seals or target
permissions. The matching time/supervision amendment is [ADR-016 section 4a](../adr/ADR-016-execution-broker.md#4a-proposed-bounded-run-supervision-2026-10-19).

### Run entrypoint and ownership

Select a foreground Rust `m1-run` application use case, with provider-free
`m1-run-inspect` and `m1-run-recover`. One strict initial request binds an existing
current Mission, admitted seed asset/name references, purpose/mode, fresh inference
permission, deadline and run profile. For this new lane select initial registration
with `m1_inference_permission_v3`: retain v2's fixed numerical/provider constraints
and add an explicit `admitted_campaign_decision_feedback` disclosure class for the
preceding committed safe proposal/explanation, decision references and owner-qualified
outcome. It admits no raw response, private reasoning or arbitrary history. Old v1/v2
consent never implies this class; upgrading/rebinding an old campaign is out of scope.
Registration remains Mission-owned and occurs
before start; no mid-run operator input, shell workflow, hand-edited observations,
context files or copied operation IDs. Input data cannot assert current authority.

The supervisor owns only run identity, bounded lifecycle, phase dispositions and
references to owner records. Persist this application bookkeeping in PostgreSQL;
it is neither another operational model nor a universal campaign aggregate.
One run owner per campaign is fenced durably. Duplicate start inspects the original
identity; conflicting input rejects. A lost supervisor never auto-resumes effects.
Composition injects narrow Mission, Cognition, Broker, Terrain and Trajectory ports;
the controller receives no sibling mutable aggregate or infrastructure repository.

Select fixed child-phase supervision for inference and acquisition/owner delivery,
using the reviewed binary and closed phase commands with run/operation references.
No arbitrary executable, shell, path or environment forwarding. The supervisor
preallocates and durably binds phase identities before launch. Child output is
bounded safe contract data; raw capture/attach bytes never traverse its result pipe.
Existing CLI entrypoints remain compatible; the run uses application contracts,
not parsing human CLI output or feeding `approve`/`continue` tokens.

### Bootstrap and proposal-to-effect mapping

Bootstrap uses Mission-admitted operator seeds as labeled premises with unknown
environmental freshness. An absent Terrain view stays unavailable. Existing
ContextPack semantics already permit limited reasoning without a usable claim;
do not require invented OBSERVED evidence or weaken the eligible-premise retention
floor once Terrain evidence exists. Mission's goal/question and declared priority
are not environmental claims. Seed asset/name binding comes from current Mission
policy, never an endpoint inferred from LLM prose.

Cognition assembles the immutable pack and proposal; Pathing retains ownership
of derived route/candidate views. Add a versioned autonomous profile to the
existing ContextPack contract, named `m1_context_pack_autonomous_v1`, preserving
`m1_context_pack_v1` and its initial profile unchanged. Its
manifests list only deployment-qualified available profiles with release binding.
Its bounded predecessor view references the first committed decision and admitted
outcome; the first pack has an explicit absent-predecessor disposition. The second
contains stable decision/event, selected intent/seed, attempt/result and
Terrain-publication references and outcome dispositions. An optional previously
admitted hypothesis summary is at most 128 UTF-8 bytes, omitted with a reason if
it cannot fit; it is never required over the retained current Terrain premise.
Disclosure requires the new class above; missing permission defers without a send.
Source/time/uncertainty/omissions remain explicit. This is not an all-run
history dump. Actual UTF-8 pack/request/configuration/repair fit must be measured
before the later runtime packet; no capacity-fit claim is made by this DESIGN.

Keep `m1_inference_proposal_v1` intent categories. In the autonomous profile,
`observe_more` with exactly one eligible seed reference and a qualified
`dns-routing-v2` manifest proposes one DNS observation. The manifest binds UDP or
TCP explicitly; no worker-selected transport switch, retry or alias question.
`observe_more` with `view_request` is an owner-view read and never DNS dispatch;
combining an effect manifest and view request rejects as ambiguous. Free text
cannot supply action parameters. Deterministic authority/Gateway validate the
selected reference/profile and current bounds; Broker alone releases the effect.

The first delivery profile allows at most two decisions, one DNS attempt and one
acquisition episode. It is a bounded execution limit, not a strategy: valid
retain/defer/stop choices and capability-gap outcomes remain possible; the
controller cannot rewrite them into DNS. The qualification scenario selects DNS
in decision one and cites its accepted outcome in decision two. Further effects
are not executed by this profile. This proves feedback wiring, not full adaptation
or M1 completion. Unsupported effects produce a visible capability-gap disposition.

### Effect ownership, safe source and Terrain intake

Prequalify Linux core/Broker and ADR-015's attempt-local Go capture/exporter,
host egress/STOP/disposal and ADR-017 framing before advertising DNS availability.
Prepare images/toolchain before the run; no campaign-time install or image pull.
An unqualified deployment blocks before provider expenditure or target release.
Same-user Windows capture and the historical Python helpers are not substitutes.

Preserve inference/session mutual exclusion: finish the first inference's durable
publication/accounting and fence its sender before claiming an acquisition session.
Broker activates a separately bound effect session under ADR-016; a
`prepared_no_effects` record is never upgraded by reinterpretation. Drain the pair,
close egress and reconcile required result/history obligations before closing that
effect session and admitting decision two. Financial consumption remains charged;
closing exclusive ownership is not a refund. Unresolved handling stops progression.
All authority writers, including withdrawal and premise-invalidating owner writes,
participate in the fence. Host STOP precedes SQL/history waits; no role widens scope.

Broker reserves episode/attempt/counters and requires durable decision/authorization
history before release. Persist an admitted exporter result plus its publication
obligation before result ACK. Terrain's intake accepts a restricted read/reference
to that issued result, independently binding campaign, session generation, proposal,
attempt, profile/schema/release, request, phase/counters and original perception time.
Caller JSON, an `authorized` flag, a source enum or a matching hash is insufficient.
Rejected/READY-only/may-start records cannot masquerade as an observed DNS response.

Use `m1_terrain_dns_issued_observation_v1` for issued acquisition and a new migration;
preserve historical operator-input contracts and migrations. Keep the exact manual
label; add `issued_dns_acquisition` only after binding verification.
Update Terrain view SQL, typed dependencies, ContextPack and history/inspection
together. Terrain independently qualifies claim status/tier, conflicts and freshness;
Broker cannot write accepted Terrain state. Intake uses expected revisions and safe
atomic owner publication. The second decision reads a new immutable owner-qualified
pack and explicitly references the first decision/result; it cannot reuse the first pack.

### Budgets, failures and verification boundary

Use the proposed ADR-016 supervision rule without increasing its 20-second shared
decision, existing 30-second command or 60-second acquisition ceilings. New run
authority must cover two decisions and their conditional repair reservations;
old consent is never reused, widened or refilled. Preserve perceived time and TTL:
expiry is `min(perceived_at + TTL, perceived_at + 60s)`. A stale/short-lived result
may yield defer, not timestamp refresh, DNS replay or forced second-call success.

Persist separate acquisition, Terrain, inference and publication outcomes, with safe
phase, last-observed milestone, terminal/UNKNOWN category and original references.
Failure after successful DNS must not relabel DNS as failed. A may-send marker is
not send evidence; child exit without terminal evidence retains uncertainty.
Inspection/recovery uses only durable records and at most three database-only
tries/invocation; it performs no provider call, unused repair, DNS or refill.

The later runtime delivery must include real CLI→child→Broker→Go exporter→Terrain
→second decision→history tests, actual serialized-byte boundaries and Linux/SQL
role/fence qualification. Inject withdrawal-before-release (zero effects), parent
loss/lease expiry, child deadline/exit, forged source, TTL expiry, conflict, result
commit/ACK loss and history failure; assert fresh recovery counts without replay.
Dependency enforcement, named touched Terrain debt and phase diagnostics follow
the concrete [roadmap dispositions](../MVP_AND_DEFERRED_SCOPE.md#13-audit-disposition-attached-to-these-deliveries).
An autonomous target-effects packet additionally requires its exact source map,
measured wire fit and qualified deployment prerequisites. A scoped infrastructure
IMPLEMENT packet may deliver and test the missing boundary in isolation; its
completion is not the first operational run. DESIGN completion asserts neither.
