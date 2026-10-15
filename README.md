# DuskWeave

DuskWeave is a persistent campaign reasoning and adversary-emulation platform with
strict separation between cognition, deterministic authority, execution, observation,
and proof.

The repository is not a collection of attack scripts. It is a campaign operating system.

## Core Architectural Invariants

- **Five Distinct Operational Models**: `CyberTerrain`, `FootholdGraph`, `AttackPathView`,
  `ObjectiveState`, and `CampaignTrajectory` remain strictly separated.
- **Proposals $\neq$ Execution**: Reasoning workers produce candidate proposals; deterministic
  validation gates all downstream actions. Every reasoning receipt retains `dispatch_granted = false`.
- **Sensitive-Data Isolation**: Client content and operational secrets remain inside isolated,
  ephemeral boundaries under PRD-000 INV-005. Secrets are never exposed to reasoning context,
  logs, or ordinary persistence.
- **Immutable Audit Integrity**: Authoritative evidence is append-only and cannot be altered
  by campaign capabilities.

## Production-Native M1 Inference

DuskWeave includes native LLM-based reasoning inference via the DeepSeek Responses API,
governed by explicit mission consent:

- **`m1-infer`**: Initiates a bounded inference attempt (`m1_inference_permission_v2`),
  evaluating ContextPacks, publishing atomic Cognition decisions, and appending Trajectory history.
- **`m1-infer-inspect`**: Inspects durable decision and history status in a fresh process
  with zero provider calls.
- **`m1-infer-recover`**: Reconciles lost publication or history acknowledgments without
  re-generating proposals or invoking the provider.

For operator guidance, environment setup, and role configurations, see the
[M1 Native Inference Runbook](docs/RUN_M1_NATIVE_INFERENCE.md).

## Documentation & Standards

- [Engineering State](docs/ENGINEERING_STATE.md) — Current delivery tracking and milestone status
- [Build Order](docs/BUILD_ORDER.md) — Architectural roadmap and sequencing
- [Quality Bar](QUALITY_BAR.md) — Engineering standards, complexity limits, and verification gates
