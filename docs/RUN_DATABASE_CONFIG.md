# RUN_DATABASE_CONFIG — Local PostgreSQL credential boundary

## Scope and authority

`DW-FIX-DATABASE-CONFIG-BOUNDARY` selects a binary-private infrastructure
credential adapter under ADR-005 least privilege, QUALITY_BAR section 6 and
[RUN_M0A](RUN_M0A.md)'s literal-loopback lane. The owner approved this operational
configuration change on 2026-10-08. It changes no PRD, ADR, invariant or seal.
It performs no target actions and does not implement PRD-010 campaign custody.

The production path is CLI -> `src/database_config.rs` -> validated
`postgres::Config` -> NoTls loopback connection -> existing
`qualify_runtime` role/durability check -> existing bounded receipt.
Configuration failures occur before business use and contain only categories.
No source, DSN, password, path or rejected file contents are printed.

## Source selection

`DW_DATABASE_CONFIG_MODE` defaults to `file`. Only the exact Unicode strings
`file` and `env-local` are accepted; no whitespace normalization or profile aliases.

- **file:** Linux only. Supply an absolute `DW_DATABASE_URL_FILE` path.
  `DW_DATABASE_URL` must be absent, even if its value would be empty.
- **env-local:** explicitly selected M0 developer/test compatibility on Windows
  and Linux. Supply `DW_DATABASE_URL`; `DW_DATABASE_URL_FILE` must be absent.
  Missing, blank or non-Unicode DSNs reject. This is not an M1 deployment profile.
- A conflicting variable rejects even when empty or non-Unicode.
- No fallback to another source, pgpass, dotenv, command-line password or another
  file. `.env.example` documents names/placeholders; the binary loads no `.env`.
  An IDE may supply test credentials through its authorized process environment;
  a local configuration file is not a production source adapter.

On Windows and other non-Linux platforms, file mode returns
`database_file_platform_unqualified` without reading the file or falling back.
No Windows ACL validation or Windows M1 qualification is claimed. Existing
Windows M0 commands require explicit `env-local` after this change.

## Linux protected-file provisioning contract

The deployment identity's provisioning directory must be trusted and private.
Provision one regular file owned by the effective UID or root, link count one,
with permission bits exactly `0400` or `0600`. Group/other access, execute bits,
setuid, setgid and sticky bits reject. Ownership eligibility does not bypass
kernel access permissions. Neither the file nor any parent may be a symlink.

Put only the runtime DSN in the file; surrounding whitespace is trimmed, not
internal content. Keep credentials outside source control, reports, logs,
ordinary/core SQL, reasoning context and client proof. Schema/migration/admin
credentials must never be the runtime credential source. Provision through the
operator's authorized secret/configuration boundary, not a command-line password.

Linux uses target-specific pinned `rustix = "=1.1.5"` with `fs` and `process`.
The adapter opens once with `openat2`, `NO_SYMLINKS`, read-only, close-on-exec and
nonblocking flags. It validates the opened descriptor and reads that same
file descriptor; there is no path validation followed by reopening.
Unsupported kernel/openat2 or inaccessible sources fail closed as
`database_file_open_failed`; no weaker open or shell fallback is used.
Directories, multiple hard links, unsafe ownership/modes and FIFOs reject;
nonblocking opening prevents waiting for a FIFO writer.

Read at most 16 KiB + 1 byte. A source over 16 KiB rejects; an exactly 16 KiB
source is not oversized but must still satisfy DSN policy. Invalid UTF-8,
blank/malformed DSNs and missing credentials reject without value diagnostics.
The environment compatibility lane also rejects DSNs over 16 KiB.

## Shared connection policy

Accept exactly one literal TCP host: `127.0.0.1` or `::1`.
Reject hostname aliases, Unix socket paths, multiple hosts and every nonempty
`get_hostaddrs()` list, including a loopback override. A textual loopback host
cannot authorize a different address through `hostaddr`.

NoTls is limited to this local lane. A DSN that requires TLS fails rather than
being downgraded to plaintext; there is no alternate transport fallback.
Every connected client retains the existing least-privilege and durability
qualification before use. A valid credential source does not establish role
suitability, historical eligibility, current dispatch permission or acquisition
qualification. `m1-policy-check` remains informational only.

## Linux effect-enabled M1 deployment requirement

The [M1 deployment contract](contracts/M1-enabling-architecture.md) requires file
mode for Linux effect-enabled M1 launch. Capture/exporter/build/generated workers
must not inherit its credential value, path/selection environment, descriptor,
mount, or access to its provisioning directory. Launchers and worker isolation
are not implemented here; qualification remains a separate required outcome.

## Bounded configuration categories

- `invalid_database_config_mode`: invalid or non-Unicode profile.
- `database_config_conflict`: forbidden alternate source variable is present.
- `missing_env`: compatibility DSN absent, blank or non-Unicode.
- `missing_database_url_file`: Linux file-mode path absent.
- `database_file_absolute_required`: Linux path empty or relative.
- `database_file_platform_unqualified`: file mode outside qualified Linux lane.
- `database_file_open_failed`: missing/inaccessible/symlink source or unsupported open.
- `database_file_unsafe`: descriptor metadata/type/link/owner/mode refusal.
- `database_file_read_failed`: opened descriptor could not be read.
- `database_config_oversize`: source exceeds 16 KiB.
- `database_file_invalid_utf8`: file bytes are not UTF-8.
- `invalid_dsn`, `missing_credentials`, `non_loopback_host`: shared DSN policy.
- `connect_failed`, `unqualified_runtime`: existing connection/qualification refusals.

Categories do not themselves establish a timeout bound. Connect/startup,
server statement/lock and active-invocation wait bounds plus the exit-124
stop contract live in [RUN_DATABASE_WAIT_BOUNDS](RUN_DATABASE_WAIT_BOUNDS.md).

## Verification and limits

`tests/database_config_cli.rs` exercises the real CLI with synthetic secrets and
bounded child supervision: explicit/exclusive profiles, invalid Unicode, DSN and
host/hostaddr refusals, Windows file refusal, and Linux descriptor-source negatives.
Linux foreign-owner setup uses passwordless `sudo -n chown 65534` only on a test-owned
synthetic file; inability to set up that fixture is failure, not skipped evidence.

`tests/database_config_postgres.rs` uses the unchanged registration DB fixture,
restricted runtime login and current migrations. It proves explicit `env-local`
inspection on both supported platforms and Linux protected-file-to-real-DB
inspection with no environment DSN in the child. Runtime role/durability and TLS
requirements still reject unsuitable sources. Existing scenario assertions remain
unchanged; their CLI construction explicitly selects the compatibility profile.

Native Windows verification covers its compatibility/negative lane; exact-candidate
Linux CI must execute protected-file cases and the existing all-targets instrumented
suite with 90% total / 80% per-production-file coverage. Neither Windows-only nor
no-run evidence establishes Linux qualification. Candidate results and SHA belong
to the external delivery report, not this runbook.

This boundary does not defend a compromised same UID/root, a privileged DBA, or
forensic RAM recovery. It claims no memory erasure, tamper-proof audit, general
secret manager, complete worker isolation or production deployment readiness.
