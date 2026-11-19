"""Pure target-partitioning hints and deterministic load balancing for Rust CI.

Scheduling hints only: never alters test budgets, execution deadlines, or
acceptance criteria.
"""

# Policy weight for a target with no measured baseline (seconds); new targets
# enter automatically and stay visible, so the estimate is deliberately
# conservative rather than fitted.
DEFAULT_WEIGHT = 30.0
# Fixed planning allowance (seconds) on worker 0 for its designated prelude,
# real-PG candidate witness, and HTTPS-loop 0037 matrix steps (median of three
# completed baselines: 382s, 380s, 241s).
WORKER0_DESIGNATED_SECONDS = 380.0
# Measured per-target wall seconds: eight heavy hints refreshed from run
# 44438585841; remaining estimates retained from serial baseline job
# 122251064388 (2026-10-27, instrumented). Scheduling hints only.
WEIGHTS = {
    "tests/m1_inference_lifecycle.rs": 231.7, "src/lib.rs": 181.6, "tests/m1_live_dns_sql.rs": 161.8,
    "tests/m1_run_failures.rs": 143.0, "tests/m1_inference_budget_limits.rs": 106.1,
    "tests/m1_provider_fixture.rs": 97.1, "tests/m1_genai_integration.rs": 94.3,
    "tests/m1_run_diagnostics.rs": 85.6, "tests/m1_session_cli.rs": 64.4,
    "tests/m1_dns_selection_postgres.rs": 51.3, "tests/m1_prepared_withdrawal_commit_ack.rs": 40.2,
    "tests/m1_terrain_postgres.rs": 36.1, "tests/database_wait_postgres.rs": 36.0,
    "tests/database_wait_commit.rs": 33.5, "tests/m1_run_postgres.rs": 26.8,
    "tests/m1_terrain_context_postgres.rs": 25.2, "tests/m1_context_prepare_publication.rs": 25.0,
    "tests/m1_prepared_withdrawal_postgres.rs": 23.6, "src/main.rs": 20.6,
    "tests/m1_native_commit_ack.rs": 18.4, "tests/m1_inference_permission_postgres.rs": 18.0,
    "tests/m1_permission_postgres.rs": 16.3, "tests/m1_policy_postgres.rs": 14.7,
    "tests/m1_terrain_context_commit_ack.rs": 14.1, "tests/m1_session_postgres.rs": 13.0,
    "tests/m1_session_privilege.rs": 12.8, "tests/m1_context_prepare_authority.rs": 12.4,
    "tests/planning_bounds_cli.rs": 12.2, "tests/planning_history_cli.rs": 11.9,
    "tests/m1_prepared_withdrawal_concurrency.rs": 11.5, "tests/authority_confirmation_cli.rs": 11.1,
    "tests/database_wait_cli.rs": 10.0, "tests/planning_cli.rs": 9.8,
    "tests/m1_inference_budget_concurrency.rs": 9.5, "tests/m1_terrain_cli.rs": 9.4,
    "tests/withdrawal_cli.rs": 9.3, "tests/planning_history_postgres.rs": 9.1,
    "tests/m1_native_privilege.rs": 8.6, "tests/m1_provider_terminal_metadata.rs": 7.9,
    "tests/m1_inference_permission_cli.rs": 7.8, "tests/m1_context_prepare_postgres.rs": 7.7,
    "tests/planning_baseline_convergence.rs": 7.6, "tests/m1_terrain_context_cli.rs": 7.1,
    "tests/planning_journal_postgres.rs": 6.9, "tests/m1_prepared_withdrawal_cli.rs": 6.7,
    "tests/m1_terrain_commit_ack.rs": 6.7, "tests/planning_withdrawal_history.rs": 6.6,
    "tests/m1_inference_budget_postgres.rs": 6.5, "tests/m1_context_prepare_snapshot.rs": 5.8,
    "tests/m1_native_cli.rs": 5.8, "tests/fixture_supervisor.rs": 5.5,
    "tests/m1_session_concurrency.rs": 5.5, "tests/m1_native_release.rs": 5.4,
    "tests/m1_inference_budget_commit_ack.rs": 5.2, "tests/m1_context_prepare_capacity.rs": 4.8,
    "tests/m1_inference_permission_commit_ack.rs": 4.8, "tests/planning_postgres.rs": 4.4,
    "src/bin/dw-fixture-supervisor/main.rs": 4.2, "tests/planning_positive_cli.rs": 4.2,
    "tests/m1_context_prepare_commit_ack.rs": 4.1, "tests/mission_producer_transactions.rs": 4.1,
    "tests/planning_bounds_postgres.rs": 4.0, "tests/withdrawal_history.rs": 4.0,
    "tests/m1_inference_budget_cli.rs": 3.9, "tests/planning_history_concurrency.rs": 3.7,
    "tests/registration_commit_ack.rs": 3.7, "tests/m1_inference_budget_privilege.rs": 3.4,
    "tests/m1_permission_cli.rs": 3.4, "tests/planning_positive_postgres.rs": 3.4,
    "tests/planning_history_conflicts.rs": 3.1, "tests/m1_policy_cli.rs": 2.9,
    "tests/m1_context_prepare_cli.rs": 2.7, "tests/planning_history_recovery.rs": 2.7,
    "tests/registration_postgres.rs": 2.7, "tests/registration_recovery.rs": 2.7,
    "tests/planning_withdrawal_postgres.rs": 2.3, "tests/planning_c1b_integration.rs": 2.1,
    "tests/planning_withdrawal_cli.rs": 2.1, "tests/withdrawal_concurrency.rs": 1.9,
    "tests/planning_concurrency.rs": 1.8, "tests/m1_native_continuation.rs": 1.7,
    "tests/withdrawal_recovery.rs": 1.7, "tests/planning_recovery.rs": 1.6,
    "tests/withdrawal_postgres.rs": 1.6, "tests/database_config_postgres.rs": 1.4,
    "tests/registration_cli.rs": 1.2, "tests/registration_permissions.rs": 1.2,
    "tests/authority_confirmation_stream_cli.rs": 0.9, "tests/m1_context_prepare_limits.rs": 0.9,
    "tests/planning_v1_compatibility.rs": 0.8, "tests/planning_withdrawal_compatibility.rs": 0.8,
    "tests/database_config_cli.rs": 0.4, "tests/m1_autonomous_wire.rs": 0.1,
    "tests/m1_run_cli.rs": 0.1, "tests/m1_terrain_context.rs": 0.1,
    "tests/m1_autonomous_permission.rs": 0.0, "tests/m1_context_prepare.rs": 0.0,
    "tests/m1_context_prepare_integrity.rs": 0.0, "tests/m1_inference_budget.rs": 0.0,
    "tests/m1_inference_permission.rs": 0.0, "tests/m1_native_admission.rs": 0.0,
    "tests/m1_permission.rs": 0.0, "tests/m1_policy.rs": 0.0,
    "tests/m1_prepared_withdrawal.rs": 0.0, "tests/m1_run.rs": 0.0,
    "tests/m1_schema_contract.rs": 0.0, "tests/m1_session.rs": 0.0,
    "tests/m1_terrain.rs": 0.0, "tests/mission_registration.rs": 0.0,
    "tests/parser_mutation.rs": 0.0, "tests/planning.rs": 0.0,
    "tests/planning_bounds.rs": 0.0, "tests/planning_history.rs": 0.0,
    "tests/planning_positive.rs": 0.0, "tests/planning_withdrawal.rs": 0.0,
    "tests/withdrawal.rs": 0.0,
}


def partition_targets(manifest, workers):
    """Longest-weight-first greedy assignment with worker0 designated allowance."""
    order = sorted(manifest["targets"],
                   key=lambda t: (-WEIGHTS.get(t["id"], DEFAULT_WEIGHT), t["id"]))
    loads = [0.0] * workers
    if workers > 0:
        loads[0] = WORKER0_DESIGNATED_SECONDS
    assignments = {str(i): [] for i in range(workers)}
    for target in order:
        index = min(range(workers), key=lambda i: (loads[i], i))
        assignments[str(index)].append(target["id"])
        loads[index] += WEIGHTS.get(target["id"], DEFAULT_WEIGHT)
    return assignments, loads
