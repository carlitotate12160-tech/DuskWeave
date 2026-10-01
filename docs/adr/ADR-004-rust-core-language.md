# ADR-004: Rust Core Language Selection

| Metadata | Value |
| :--- | :--- |
| **Document ID** | ADR-004 |
| **Title** | Rust Core Language Selection |
| **Status** | ACCEPTED |
| **Acceptance** | Product owner, 2026-09-28: accepted language ownership and the revised reasoning/inference boundaries |
| **Stage** | Stage 3 - Foundation ADRs |
| **Direct Dependencies** | [ADR-001](ADR-001-modular-monolith.md), [ADR-002](ADR-002-domain-boundaries.md), [ADR-003](ADR-003-domain-events.md) - all ACCEPTED |
| **Product Authority** | PRD-000..006 - all ACCEPTED; see [PRD registry](../prd/README.md) |

## 1. Context and problem

The modular campaign core needs distinct model owners, bounded commands and views, deterministic action authority, and recoverable event handling. It must express invalid combinations clearly without growing a universal campaign state or a generic workflow framework. Language selection supports these boundaries; it cannot establish their product meaning or replace runtime checks.

The repository language baseline assigns core semantics to Rust and integration work to Go. This ADR evaluates and proposes that ownership as an architectural decision. It does not authorize a second implementation of domain rules in adapters, select a process protocol, or make every module an independently deployed service.

## 2. Decision drivers

1. Express model identity, epistemic distinctions and explicit outcomes through bounded interfaces with compiler assistance.
2. Restrict mutation to the owning context while retaining the short local decision paths of ADR-001.
3. Separate reasoning, deterministic authority, capability eligibility, dispatch and external execution.
4. Preserve ADR-003 recovery, compatibility and integrity obligations across language boundaries.
5. Keep early implementation understandable: ordinary types and explicit composition before advanced generics, macros or new frameworks.
6. Make safety claims testable and limited; language choice must not imply zero-retention, audit integrity, current authorization or process isolation.

## 3. Options considered

| Option | Benefit | Cost and disposition |
| :--- | :--- | :--- |
| **Rust core with bounded Go integration components** | Ownership, explicit enum outcomes and typed interfaces support core boundaries; Go retains its assigned integration role. | Rust learning and compile-time costs; multiple toolchains and boundary compatibility work. Selected with narrow language ownership. |
| Go core and adapters | One production toolchain and a straightforward integration model; Go provides memory management and static typing. | Core state distinctions and ownership would need a different enforcement design and revision of the existing baseline. Credible alternative, but no demonstrated delivery constraint justifies that change now. |
| Rust throughout core and adapters | One language and less cross-language contract tooling. | Would change the Go integration baseline and require evaluating each external integration's fit. Not selected as a blanket rule; Rust-side infrastructure adapters still implement core ports where appropriate. |
| Python core and integration | Fast experimentation and access to research tooling. | More semantic enforcement would rely on runtime validation and testing; contradicts the research-only baseline without a specific accepted promotion. Retain for research. |
| C/C++ or Zig core | Direct native integration and low-level control. | Adds a different core safety/review burden without a demonstrated low-level core requirement. Retain only the constrained native roles below. |

These are architectural tradeoffs, not benchmark results. No language is claimed fastest, easiest for this team, or sufficient for product correctness. Team proficiency, build time and workload performance remain unmeasured; material evidence may justify a superseding ADR.

## 4. Decision and language ownership

Select **Rust for first-party campaign core domain and application logic**. Keep bounded contexts from ADR-002 distinct within the modular deployable. A language boundary is neither a domain boundary nor an authorization grant.

| Responsibility | Language ownership and limit |
| :--- | :--- |
| Five operational models and their intent-specific commands | Rust; separate owners for Terrain, Access, Pathing, Objectives and Trajectory. No universal aggregate or shared mutable model registry. |
| Mission/lifecycle, bounded loop use cases, evidence admission and action authority | Rust; compose narrow ports and immutable views, without absorbing sibling aggregates. |
| Capability Gateway and execution broker | Rust core-side eligibility and dispatch responsibilities remain separate; neither absorbs adapter execution. |
| Tool adapters, collectors, network/integration workers and telemetry adapters | Go baseline; translate admitted requests and bounded outcomes, without owning campaign truth or redefining policy. No generic arbitrary-execution escape hatch. |
| Core infrastructure implementations | May use Rust to implement persistence, messaging or external-client ports. Their implementation dependencies remain outside domain logic; this does not select a library or transport. |
| Native helpers and external SDKs | Zig only for a demonstrated native-helper requirement; C/C++ only for necessary FFI or unavoidable SDK integration. Placement and admission require the later native-boundary design. |
| Research | Python/Nim remain research-only unless an accepted ADR promotes a specific component. Research code cannot enter an execution path by being wrapped in an adapter. |

Rust owns bounded campaign use cases that assemble sourced context, govern reasoning-episode budgets and lifecycle, and admit reasoner proposals as untrusted input. Selecting Rust for the core does not require inference in Rust. Inference language, runtime and provider remain unselected. Inference behind an external interface is the starting direction, not a permanent deployment constraint or a choice of HTTP, vLLM or Candle.

A separately supplied inference backend does not determine the language of DuskWeave's use cases. DuskWeave-owned Python reasoning code may enter production only through an accepted ADR admitting that specific component; placing it behind a service interface does not bypass this requirement. Whatever its implementation, a reasoner has no model-mutation or execution authority. Using a Rust SDK or a Go transport client grants neither.

"Thin orchestration" limits coordination responsibility, not reasoning depth. Within its bounded episode, a reasoner must be able to compare hypotheses, consider observing more or taking no target action, and identify evidence that would disconfirm its proposal, as required by PRD-006. Each loop keeps its bounded use cases; no universal orchestrator absorbs their models or decisions. Proposal admission is not model acceptance or execution authorization.

Malformed proposals may be returned for correction through bounded reasoning attempts. A corrected proposal is fresh untrusted input and must pass the applicable validation; the caller cannot silently reinterpret malformed content into an authorized action or relax checks to obtain acceptance. Exhausted correction budgets leave the episode unresolved without dispatch from the invalid proposal. Retrying an LLM call never authorizes repeating an external action with an unknown outcome; ADR-003 reconciliation obligations still apply.

Caller language alone does not determine model capability, and cross-language outputs are not presumed identical. Model version, tokenizer, chat template, sampling configuration and inference runtime can affect results. Future evaluation must record the relevant configuration and measure proposal quality and campaign-boundary compliance on comparable synthetic cases, including valid no-action choices. This is an empirical verification obligation, not a claim of deterministic LLM output or an evaluation-platform design.

### 4.1 Types express local guarantees

Use domain-specific identifiers, explicit alternatives for outcomes, private aggregate state, and intent-specific command interfaces. Parse external values into bounded types at admission. Parsing establishes structural validity; the receiving owner still checks semantics, provenance, campaign, exercise mode and current dependencies.

Typestate is optional where it makes a local transition clearer. It must not encode every campaign combination in a generic global state machine. A typed authorization result, historical success or validated-position view can become stale: dispatch must consult current authority and required premises. A candidate belongs to Pathing until Access accepts a validated position; a typed foothold has no direct tool-execution capability.

External serialization carries contract data, not Rust memory layout or aggregate internals. Receiving code re-establishes its own guarantees; a deserialized tag such as "authorized" or "fact" is not proof. ADR-003 publisher identity, campaign scope, version compatibility, ordering and deduplication requirements survive serialization. Wire schemas, codecs and transport are deferred.

### 4.2 Dependencies and composition

Domain modules depend on their own rules and bounded contracts, not sibling implementations, database clients, LLM clients, telemetry backends or OS execution code. Application use cases compose ports; infrastructure implements them at the outer boundary. Do not create an all-domain common crate, global service locator or shared mutation handle.

Package and crate layout are deferred. Enforcement must cover module visibility and actual dependency direction even if multiple contexts initially share a deployable. One crate per context, async everywhere, trait-per-struct and generic repository frameworks are not requirements.

### 4.3 Memory safety is a limited guarantee

First-party domain and application core code must forbid unsafe Rust. This is not a claim that the standard library or dependencies contain no unsafe code. Dependency review must consider unsafe internals, native code and build scripts; a safe public API alone is not a complete supply-chain or isolation assessment.

Necessary native integration stays outside domain/application code behind a bounded adapter and a reviewed safety contract. This ADR grants no permission to load an execution tool or sensitive-proof component into the core through FFI. Required isolation remains governed by ADR-001/002 and later execution/proof decisions.

Ownership and borrowing help control memory access. They do not prove business correctness, prevent all logical races, bound resource use, or isolate mutually distrusting components. Raw client proof material remains inside the isolated ephemeral proof boundary. Any separately authorized operational secret remains inside campaign-scoped custody and the eligible execution boundary; Rust core code receives only an opaque reference and safe metadata. Memory deallocation is not proof of erasure; destructors are not a sufficient cleanup guarantee for abnormal termination.

## 5. Failure, concurrency and recovery obligations

- Expected failures use explicit bounded results and domain errors. Production paths must not use unwrap or expect. Errors, debug representations and panic diagnostics must not expose raw sensitive content or unauthorized defender context.
- Panics, aborts and core crashes are failures, not successful command completion. Recovery preserves ADR-003 obligations and reconciles unknown execution outcomes before retry. Catching a panic alone does not establish that state is consistent or an action is safe to repeat.
- Cancellation is not rollback or proof that an external action never happened. Withdrawal blocks dependent dispatch and initiates the required stop handling without waiting for an event backlog; reconcile any uncertain in-flight outcome.
- Concurrent access must preserve the owning context's invariants. Shared-memory synchronization or a compiler-accepted program does not establish a current, atomic view across models.
- Deterministic decisions receive explicit evaluation context, including relevant time and revisions. Replay does not invoke capabilities or reinstate authorization; rebuild equivalence uses the same inputs, evaluation time and reconciliation rules.
- Cross-language timeouts, malformed responses and version incompatibilities remain explicit boundary failures. Transport success is not observation acceptance, model acceptance or objective fulfillment. A worker cannot widen an authorized request to recover from an error.

The exact runtime, scheduling model, panic strategy, resource limits and recovery mechanisms require later bounded implementation decisions. Deferral does not waive these obligations.

## 6. Consequences and complexity control

Rust makes certain invalid representations and memory-access patterns rejectable before execution. Explicit ownership also fits ADR-002's intent, but architectural ownership still requires visibility, dependency checks and review. Go integration adds contract and release coordination costs; add a component only for an actual integration need, not to demonstrate polyglot coverage.

Prefer explicit cohesive functions over elaborate generic machinery. The [quality bar](../../QUALITY_BAR.md) keeps McCabe complexity at **7 per function** and the production/tooling file hard cap at **400 LOC**. Current cumulative-diff review triggers/ceilings and test/document budgets follow that quality bar; these delivery controls are independent of model ownership. Splitting must preserve meaningful responsibilities; moving branches into arbitrary helpers does not establish maintainability. No metric alone detects spaghetti, redundancy, verbosity or semantic coupling.

When runtime work is authorized, pin supported toolchains and dependency resolution, select compatible dependencies, and establish the existing formatting, linting and dependency-audit gates. Configure checks for the actual forbidden patterns; a generic warnings-as-errors invocation does not by itself enforce every quality rule. A McCabe measurement must not be replaced by a differently defined complexity metric. No toolchain version, analyzer, workspace manifest or CI gate is installed by this document.

## 7. Invariant compliance

| Invariant | Language-design obligation |
| :--- | :--- |
| INV-001 No God Object | Bounded composition; no universal state, global service locator or abstraction exposing all models. |
| INV-002 Separate models | Distinct model owners and mutation interfaces; cross-context references do not confer ownership. |
| INV-003 Reasoning != execution | Types and ports preserve proposal, authority, Gateway, Broker and Adapter boundaries; no bypass of a preceding boundary. |
| INV-004 Observation != fact | Parsing and types do not promote evidence. PROVISIONAL transient observations retain their limits; exploitation tempo does not lower evidence tiers. |
| INV-005 Sensitive isolation and secret custody | Core contracts exclude raw client proof and operational secret values; ownership or destructors do not substitute for isolated custody, bounded lifetime, or cleanup. |
| INV-006 Audit integrity | Explicit recoverable outcomes and ADR-003 event obligations; language or adapter code gains no authority to rewrite authoritative evidence. |
| INV-007 Defender Knowledge Boundary | Mode/campaign admission applies to data, cached context and outcomes; blind reasoning excludes privileged feeds while permitting effects visible from its authorized acquisition context. |

## 8. Review cases and future verification

| Case | Required result |
| :--- | :--- |
| A caller substitutes a target identifier for a foothold identifier | Typed internal interfaces reject the mismatch; external admission validates the actual referenced entity and scope. |
| A command or dependency reaches a sibling's private aggregate | Visibility/dependency verification rejects the path; bounded owner commands remain usable. |
| A typed action proposal survives authorization withdrawal or presumed origin loss | Current dispatch checks stop dependent action; the old value conveys no enduring authority. |
| A Go worker returns an "accepted fact" or a larger execution scope | Core admission rejects the authority claim or scope change; receiving owner applies its own rules. |
| An observation comes from authorized transient read-only orientation | It remains PROVISIONAL; parsing cannot create a validated foothold or authorize follow-on state-changing action. |
| A crash follows external dispatch but precedes outcome recording | Recovery treats outcome as unknown and reconciles; no blind replay or assumed failure. |
| Duplicate, conflicting or incompatible boundary events arrive | ADR-003 deduplication, integrity-resolution and compatibility rules hold without duplicate semantic effects. |
| Replay runs after authorization expires | Projections may rebuild under explicit evaluation rules; replay cannot execute actions or restore authority. |
| A worker emits raw sensitive output or privileged defender context into a blind result | The applicable boundary blocks propagation to core models, prompts, logs and durable event records. |
| A function exceeds McCabe 7, or refactoring creates a universal helper | Complexity verification or architecture review blocks delivery until cohesive correction. |
| Evidence supports observing more or taking no target action | The reasoner may select that outcome with sourced rationale and disconfirming evidence; orchestration does not force an action. |
| A malformed proposal is repaired or the correction budget is exhausted | Every correction is validated as untrusted input; exhaustion leaves no dispatch from the invalid proposal. |
| An LLM retry proposes repeating an action whose outcome is unknown | Reconciliation remains required; a new proposal cannot clear the unresolved execution state. |
| A Python reasoning component is wrapped as an inference service | Its DuskWeave-owned reasoning role still requires specific ADR admission before production use. |
| Rust and Python callers or inference configurations are compared | Evaluation records relevant configuration and measures proposal quality and boundary compliance; identical output is not assumed. |

These are future compile-time, contract, integration and recovery verification obligations using synthetic fixtures. They are not implemented or executed tests. Document review checks accepted authority, ownership, failure semantics and decision scope only.

## 9. Technical references and acceptance

Language facts were checked against primary documentation on 2026-09-28: [Rust ownership](https://doc.rust-lang.org/book/ch04-01-what-is-ownership.html), [unsafe Rust](https://doc.rust-lang.org/book/ch20-01-unsafe-rust.html), [Drop and its caveats](https://doc.rust-lang.org/std/ops/trait.Drop.html), and [Go FAQ](https://go.dev/doc/faq). These explain language facilities and limitations; the architectural choice is a DuskWeave design judgment.

ADR-004 is **ACCEPTED** by the product owner on 2026-09-28, including the revised reasoning/inference boundaries and review cases. It selects no database, schema, graph representation, IPC protocol, service count, runtime framework or package layout. ADR-005 is the next design dependency; ADR-005..007 are not authored at this acceptance. This records approval of ADR-004 only: the foundation seal defined in build-order section 30 still requires ADR-005..007 and coherent foundation authority. Runtime implementation remains unauthorized. On 2026-09-29 the product owner authorized the INV-005 campaign-scoped secret-custody boundary amendment reflected here; `DW-FOUNDATION-001` was explicitly resealed by the product owner on 2026-09-29 against authority baseline `f93087b52c480822544bad0fb5d99d17eedf8ac0`.
