# M1 Enabling Architecture, Owner Contracts and Sequencing

| Metadata | Value |
| --- | --- |
| Document | DW-DESIGN-M1-ENABLING-20261006 |
| Status | ACCEPTED — product owner, 2026-10-06; selected M1 architecture, owner contracts and sequencing |
| Revision | R1 — corrections reviewed; six scoped ADRs and this owner/sequencing outcome accepted on 2026-10-06. R2 — cognitive/generated-implementation amendment under the owner-approved 2026-10-07 direction, published as a candidate with exact-text acceptance recorded separately |
| Base | `086936d2903b8958c0e5bc69af9891d9710e092f` |
| Product input | [Accepted M1 product contract](M1-external-orientation-decision-loop.md), especially section 11 |
| Authority | Accepted PRD-000..010, ADR-001..008 and the six scoped M1 ADRs; no target permission |
| Delivery | One documentation-only DESIGN outcome; runtime diff = 0 |
| Preserved seals | DW-FOUNDATION-001 and DW-M0-001; no full-domain or M1 seal |

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
effects and relevant campaign-local feedback. At most 8 candidates; no raw provider input,
private aggregates, execution handles, privileged defender feed or all-campaign context.
Include references and explicit omission reasons; visible truncation must not imply that
omitted evidence does not exist. The worker may request another bounded safe owner view
before choosing an effect; such a read does not recollect target data, and any requested
new observation is a new proposal through current authority. No raw content, secrets,
unrestricted repositories, defender oracle or privileged evaluator answers enter the
ContextPack.

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

ADR-013 owns numerical limits and fixed profile versions. ADR-015 selects qualified Linux
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

This DESIGN delivers six ACCEPTED scoped ADRs, this accepted amendment/owner contract,
registry links and factual predecessor/tracking reconciliation, now revised by the R2
cognitive amendment under the owner-approved 2026-10-07 direction. Existing accepted
product scope and M0 evidence are unchanged. Document validation/review are distinct from
acceptance; the R2 text is a publication candidate whose exact-text acceptance is
recorded separately.

The next runtime outcomes are prepared in dependency order — bounded cognitive
context/proposal consumer, then isolated synthesis/admission consumer, then
feedback/reuse and report completion — reusing existing authority/effect capabilities
where implemented. This is an ordering recommendation, not acceptance of a new runtime
slice sequence: each IMPLEMENT packet still needs a measured complete vertical file map
and its exact accepted base.

An IMPLEMENT packet is NOT ISSUED/NOT READY here. After accepted authority is published,
Work must inspect the actual selected runtime source/consumer/fixtures and supported
execution environment, measure a complete observable slice against the exact new base,
reserve correction room and give an exact file map. The 600-line ceiling is unchanged;
this DESIGN does not promise a full M1 loop in one runtime PR or a fixed PR count.
Stop at this enabling DESIGN boundary.
