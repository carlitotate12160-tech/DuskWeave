## Packet

- Packet ID / assignment:
- Verified base (commit):
- Authority references (PRD / ADR / contract):

## Scope

- [ ] Changes stay inside the packet file map; no scope drift
- [ ] Runtime diff <= 400 LOC; file size <= 400 LOC; McCabe <= 7 per function
- [ ] No God Object, no cross-model mutable access (INV-001/002)
- [ ] No `utils/`, `helpers/`, `common/`, `misc/`, `managers/` dumping grounds

## Wiring (runtime packets)

- [ ] Entrypoint -> owner/port -> changed component -> consumer -> observable result traced
- [ ] No dead code, islands, or components consumed only by tests
- [ ] Domain does not import adapters, databases, HTTP/LLM clients, or OS execution

## Tests

- [ ] TDD: failing test precedes implementation
- [ ] Failure-path tests assert behavior, not implementation echo
- [ ] INV-001..007 negative-control tests where applicable
- [ ] No unwrap/expect/unsafe on production paths; no dead code

## Sensitive data

- [ ] No raw client content or secret values in state, logs, events, or surfaces (INV-005)

## Review

- [ ] One adversarial review run on final HEAD
- [ ] Findings triaged as VALID / FALSE_POSITIVE / UNVERIFIED below

Findings:
