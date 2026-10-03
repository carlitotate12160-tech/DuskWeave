# Delivery Report: DW-FIX-CI-MCCABE-GATE

## 1. Worktree and Authority
- **Repository:** `carlitotate12160-tech/DuskWeave`
- **Worktree:** `D:/DuskWeave-ci-complexity`
- **Branch:** `fix/ci-mccabe-gate`
- **Base SHA:** `73b0b6ff49a2d5ec08f8a71c5ace5d2144e015cc`
- **Packet:** `DW-FIX-CI-MCCABE-GATE`

## 2. Implementation Overview
- Added `scripts/check_complexity.py` to enforce the McCabe cyclomatic complexity limit using `rust-code-analysis-cli 0.0.25`.
- Added `tests/test_complexity.py` as a regression test suite covering the 50-line review trigger, standard 7-cap, and the single reviewed pure dispatch 10-cap allowance for `src/main.rs`.
- Injected strict Linux measurement gate into `.github/workflows/doc-check.yml`.
- Reconciled tracking states in `docs/ENGINEERING_STATE.md` and `docs/BUILD_ORDER.md` to record the exact status, including PR #22's merge and the intentional CI McCabe gap discovery.

## 3. Verified Violations (Baseline Rejection)
The gate successfully rejects the unchanged protected-base Rust source code due to 13 inherited cap violations:
- `src/planning.rs`: `validate_contract` (own=8), `assessment_labels` (own=9)
- `src/postgres_mission.rs`: `commit_tx` (own=9), `prior_assessment` anonymous closure (own=8), `assess` (own=16)
- `src/postgres_mission_basis.rs`: `decode_basis` (own=14)
- `src/postgres_planning_history.rs`: `predecessor` (own=12), `append` (own=10)
- `src/postgres_trajectory_history.rs`: `append` (own=9)
- `src/postgres_trajectory_journal.rs`: `matches` (own=10), `existing` (own=10)
- `src/registration.rs`: `register` (own=10), `reconcile` (own=10)

## 4. Final Disposition
- **Status:** DRAFT PUBLISHED, NOT READY FOR MERGE
- **Evidence:** This packet delivers a working detector. Expected rejection of that baseline is evidence that enforcement works.
- **Next Action:** Owner-bounded remediation of the 13 inherited complexity findings and gate qualification. No new exception was granted. Work owns remediation and gate qualification before a green gate merge.
