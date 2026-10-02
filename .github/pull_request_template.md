## Packet

- Packet ID / assignment:
- Verified base (commit):
- Authority references (PRD / ADR / contract):

## Scope

- [ ] Changes stay inside the packet file map; no scope drift
- [ ] Applicable QUALITY_BAR.md file/packet budgets checked; runtime >400 review trigger disposition recorded
- [ ] Functions >50 lines reviewed; McCabe 7 / qualified pure-dispatch 10 measurement or UNVERIFIED status recorded
- [ ] No God Object, no cross-model mutable access (INV-001/002)
- [ ] No `utils/`, `helpers/`, `common/`, `misc/`, `managers/` dumping grounds

## Wiring (runtime packets)

- [ ] Entrypoint -> owner/port -> changed component -> consumer -> observable result traced
- [ ] No dead code, islands, or components consumed only by tests
- [ ] Domain does not import adapters, databases, HTTP/LLM clients, or OS execution

## Tests

- [ ] Behavior change has genuine RED/GREEN evidence; already-correct refactor has passing baseline/regression evidence; document-only N/A explained
- [ ] Failure-path tests assert behavior, not implementation echo
- [ ] INV-001..007 negative-control tests where applicable
- [ ] No unwrap/expect/unsafe on production paths; no dead code

## Sensitive data

- [ ] No raw client content or secret values in state, logs, events, or surfaces (INV-005)

## Review

- [ ] One adversarial review run on final HEAD
- [ ] QUALITY_BAR.md section 7 Q1-Q8 dispositions have candidate-bound evidence or concrete N/A reasons
- [ ] Findings triaged as VALID / FALSE_POSITIVE / UNVERIFIED, blocking impact stated
- [ ] Fix review covers delta/affected claims; required final-candidate checks and READY / NOT READY scope stated

Findings:
