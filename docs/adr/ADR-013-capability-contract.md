# ADR-013: Fixed M1 Capability Profiles

| Metadata | Value |
| --- | --- |
| Status | ACCEPTED — product owner, 2026-10-06; M1 selected lane only |
| Date | 2026-10-06 |
| Revision | R1 — scoped architecture accepted by the product owner on 2026-10-06 |
| Authoring base | `086936d2903b8958c0e5bc69af9891d9710e092f` |
| Product authority | Accepted PRD-000..010 and M1 product contract; no new product breadth |
| Direct dependencies | accepted PRD-000/001/006/007/010, M1 product contract and ADR-009/011; selected-lane rebind in accepted M1 enabling section 2 |
| Scope | [M1 enabling lane](../contracts/M1-enabling-architecture.md); no full-stage acceptance |

## 1. Context and problem statement

M1 selects exactly CT lookup, bounded DNS and unauthenticated HTTPS HEAD.
Reasoning needs a fixed action vocabulary without creating a generic tool registry.
Contract acceptance is not current execution authority.

## 2. Decision drivers

Explicit effects/disclosures, semantic version binding, actual-operation accounting,
narrow results and deterministic rejection before any contact.

## 3. Considered options

| Option | Benefit | Conflict / disposition |
| --- | --- | --- |
| Arbitrary command/URL and flags | Flexible | Unbounded effect and sensitive surface; reject |
| Dynamic registry/plugins now | Extensible | No selected need; defer ADR-014 |
| Three statically compiled versioned profiles | Reviewable exact effects | Select; changes require explicit profile admission |

## 4. Decision outcome

R1 profile IDs are ct-existing-v2, dns-routing-v2 and https-head-v1. The prior proposed v1
CT/DNS identifiers were superseded before R1 acceptance; no deployed compatibility
or migration is claimed. The deployment allowlist binds each version to reviewed image/
binary digest, schema and finite limits. SHA-256 identifies release code/config, not proof.
Unrecognized versions/digests reject; no dynamic registry or runtime plugins.

| Profile | Fixed request | Effect / limitation |
| --- | --- | --- |
| ct-existing-v2 | Approved disclosed base domain and disclosure class, crt.sh endpoint | One existing-record GET; bounded safe new hints/partial intake; no submission/rescan/redirect/pagination/provider fallback |
| dns-routing-v2 | Approved hostname and declared resolver IPv4/vantage; parent-selected UDP or TCP | One recursive A/IN question and bounded reply; no worker retry, transport switch or extra alias question |
| https-head-v1 | Admitted host/IPv4, 443 and approved path | One TLS connection and HEAD, original hostname/SNI, normal certificate validation; no GET/redirect/auth/body |

Provider bootstrap uses separately issued DNS attempts for the fixed provider hostname,
outside the target candidate list but inside the same campaign/episode budgets. CT receives
one pinned admitted provider address; no ambient resolver/proxy/Happy Eyeballs, library
replay or connection reuse. Resolver recursion is disclosed but not a DuskWeave direct query.
Bootstrap is a fixed prerequisite of a selected CT proposal, not an extra candidate-ranking
comparison. Each DNS attempt still needs its own current admission, identity and counters.
Failure returns the unmet dependency to the reasoner; it cannot trigger hidden alternatives.

TCP DNS is an explicit new attempt after a UDP truncation, or an admitted initial transport.
At most one explicitly proposed DNS follow-up per query subject per episode may address
truncation or timeout; it gets a new identity and current authority and consumes another
question, plus a TCP connection when applicable. No jittered background worker retry.
Lost acknowledgement/UNKNOWN is never automatically replayed; it retains the original
possibly-consumed count and disposition. A later new observation is separately admitted.

Interpret alias lineage from the bounded response. Missing terminal A, loop/depth overflow,
withheld required alias, truncation or disallowed address leaves an incomplete binding.
A changed RRset at a different time is not automatically contradictory; owner semantics apply.
Target connections use a pinned admitted address. After a clear failed connection, the
reasoner may propose another admitted address as a new HEAD attempt inside remaining
budget, not an automatic fallback. Unknown effects stay unknown. No IP probing is created.

HEAD is HTTP/1.1, fresh connection, no compression/keepalive. Close after status/header
admission; unexpected body stays inside capture and is not application output. Unsupported
HEAD, redirect and TLS failure remain limited results; another method needs owning admission.

### Finite R1 ceilings and accounting

Mission supplies positive finite campaign totals for episodes, provider calls, DNS questions,
DNS follow-ups, TCP connections and HEAD requests, plus window/deadline/concurrency. Missing
totals reject acquisition; there is no built-in unlimited campaign preset. Effective bounds
are the minimum of remaining campaign totals, episode ceilings, attempt limits and time.
Only one episode/effect runs in a campaign at a time. New episode/restart cannot reset totals.

| Resource | Selected ceiling |
| --- | --- |
| Episode | 60 seconds; one campaign/vantage; concurrency 1; at most 4 reasoning comparisons/revisions |
| Disclosure/contact rules / acquired candidates | At most 8 exact-name or label-suffix rules per purpose; 8 candidates per episode; name <=253 ASCII bytes |
| CT fetch | 1 request, 10 seconds, 256 KiB total inbound including TLS/HTTP; at most 64 completely parsed records |
| DNS questions | 10 total, including at most 2 provider bootstrap questions and all follow-ups; 2 seconds per attempt |
| DNS follow-ups / TCP DNS | At most 1 follow-up per query subject and 2 total follow-ups/episode; at most 2 TCP DNS connections/episode |
| DNS response / records / CNAME depth | 4096 bytes / 32 records / 4 links; strict TCP framing bound before allocation |
| IPv4 destinations | At most 4 per host; each contact requires a separately admitted pinned address |
| HTTPS target attempts | At most 2 HEAD validations/episode, 1 connection+HEAD each, 5 seconds per attempt |
| All external TCP connections | 5 total: CT <=1, TCP DNS <=2, targets <=2; every actual connect counts |
| Target headers / inbound TLS | 8 KiB decoded headers / 64 KiB network bytes per validation |
| Approved path | <=256 ASCII bytes; starts slash; no query/fragment/userinfo/controls |
| IPC request/result/control | 16 KiB / 16 KiB / 1 KiB per frame; 16 frames/attempt; raw-facing candidate also <=16 KiB |
| JSON nesting / collection | 8 levels / 64 entries; hostname arrays additionally bounded by candidate/lineage limits |
| Automatic network replay/redirect/pagination/recollection | 0; declared new DNS follow-up is counted separately above |
| Database-only recovery | At most 3 tries/invocation, then visible pending obligation |

Broker owns durable campaign/episode reservation and consumption. Reserve each question/
connection/request before its effect-start; phases charge their own counters without double
charging. Provably not released may free its reservation through an accountable record.
May-have-started/UNKNOWN remains charged; ack loss or episode reset cannot refund it.
Retry/follow-up and address change consume remaining totals; insufficient budget yields defer.

Bounds act before unbounded buffer allocation. CT may emit safe hints from complete parsed
records preceding a byte/record bound with bounded completeness; preserve that limitation,
not an empty/full-discovery claim. Missing/negative/zero/overflow and zero-as-unlimited reject.
Network durations run from the first effect release; pair setup/READY must fit the remaining
episode and absolute request deadline and cannot extend it. Candidate output additionally
respects the remaining 8-name episode capacity, including admitted seeds and prior hints.
The ceilings are selected assumptions, not benchmark results. The future affected profile
qualification records provider availability, intake limits, latency and safe partial outcomes
on the supported deployment; failure cannot be hidden by raising limits or adding providers.

DNS expiry is min(perception time + lowest relevant returned TTL, perception time + 60s).
Returned TTL may be remaining resolver-cache lifetime; it proves no authoritative freshness.
TTL zero is a valid observation but cannot support a later separately issued HEAD in R1;
immediate non-cached resolve-and-connect remains a deferred profile decision. CT is historical;
HTTP/TLS is point-in-time and ranking-eligible for at most 60s with unchanged dependencies.

Request binds engagement/campaign, episode/proposal/attempt, purpose/mode, profile/schema/
release, Mission revision, approved query/endpoint/policy refs, vantage, deadlines and counters.
No target operational secrets or provider credential values are carried. Result binds that
context to ADR-011's safe projection/diagnostics; it cannot assert accepted/fact/authorized.
Authority owns permission/premises; Gateway owns fixed request/version/bounds; Broker owns
effect accounting/lifecycle. A batch reservation or historical result bypasses none of them.

## 5. Consequences

Small fixed profiles avoid shell and registry authority. IPv4, 443 and one declared resolver
are v1 coverage limits, not claims of complete discovery. Another method/port/family/provider
requires a declared profile change under the owning accepted boundaries. Product breadth
changes require the applicable product/architecture decision.

## 6. Invariant compliance matrix

| Invariant | Obligation |
| --- | --- |
| INV-001 | Static capability specs have no campaign model references or dynamic execution handles. No universal manager or workflow engine. |
| INV-002 | Terrain, Pathing and Trajectory retain distinct owners; no Access/Objective mutation. |
| INV-003 | Proposals, evidence, history and worker results never confer dispatch authority. |
| INV-004 | Narrow source-qualified claims, explicit uncertainty and owner reconciliation. |
| INV-005 | Only purpose-admitted safe semantics cross ordinary boundaries; no custody/proof implementation. |
| INV-006 | Runtime histories append; sensitive remediation has separate authority and safe accountability. |
| INV-007 | Campaign/mode/source isolation survives reads, derivation, correction and recovery. |

## 7. Verification and acceptance boundary

DESIGN checks links, ownership, dependencies and counterexamples under QUALITY_BAR section 7.
The cases above are future assertions, not executed runtime tests. The product owner
accepted R1 for the selected M1 lane on 2026-10-06. Runtime requires an issued bounded
packet; acceptance grants no target permission, qualified deployment, merge or seal.
