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

Native operations bind validated input before reserving capacity; inspection and
recovery do not replay provider calls. Current qualification and limitations belong
in [Engineering State](docs/ENGINEERING_STATE.md).

## Windows developer commands

Use Windows PowerShell 5.1 or later, Git, the pinned Rust toolchain, and Python
from the 3.12 series in `.python-version` (matching CI). Scripts use the standard
library; no Python package manager is required. Tests also need OpenSSL, selected
from PATH or Git for Windows. Override tool selection with `-Python` / `-OpenSsl`.

```powershell
.\scripts\dev.ps1 build
.\scripts\dev.ps1 check
.\scripts\dev.ps1 test
.\scripts\dev.ps1 test -Target m1_provider_terminal_metadata -Postgres
.\scripts\dev.ps1 coverage -Target m1_provider_terminal_metadata -Postgres
powershell -NoProfile -File tests/test_dev_entrypoint.ps1
```

`test` defaults to library/application unit tests; `-Target` selects one tracked
integration target. `-Postgres` supports native-fixture targets and uses the existing
bootstrap to inject fresh restricted DSNs into child processes. It requires Docker and owned `dwpg-r3`:
only the pinned PostgreSQL 17.11 image on loopback 5432 is reused/started. A missing
or incompatible container fails without replacement. `dev.ps1 postgres` checks it
without creating a database. Existing containers, volumes and test evidence remain.

The entrypoint never loads `.env`, changes global settings or installs tools.
Inherited DuskWeave credentials and the DeepSeek key are excluded; caller environment
and cwd are restored on success/failure. One same-session owner holds build/fixture
resources. It uses the existing shared ordinary target, separate `coverage/dev`,
and unique `reports/dev` paths; `-ArtifactRoot` overrides the artifact base.
Coverage needs cargo-llvm-cov 0.9.1 and llvm-tools-preview. Reports cover the selected
tests only; the full-suite 90%/80% gate remains in CI. These commands
do not run providers, targets or qualify a production deployment.

## Documentation & Standards

- [Engineering State](docs/ENGINEERING_STATE.md) — Current delivery tracking and milestone status
- [Build Order](docs/BUILD_ORDER.md) — Architectural roadmap and sequencing
- [Quality Bar](QUALITY_BAR.md) — Engineering standards, complexity limits, and verification gates
