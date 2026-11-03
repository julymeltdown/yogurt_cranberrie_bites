# BE02 native migration and database restart slice

Result: **PASS for the database-only slice, pending integrator review.** Full
BE02, normal service startup/restart, packaging, runtime roles and BE44 remain
unaccepted/NOT_RUN. This exercise starts only a disposable PostgreSQL cluster
and invokes the six prebuilt services with their database-only CLI flags.

## Candidate and ownership

- Worker: `codex/mcr-be-foundation`, based on
  `909e2bf4e1c5bdf752954c94bf71a3b64871732d` (imported foundation).
- The separate environment/cwd correction is based on the first smoke commit
  `a0174ece47d169f5b79bfac81a5e9661a09a8b08`. The latest report records that
  predecessor plus the exact revised harness hash; migration sources are unchanged.
- Inherited source: port-rust
  `704f42149d5e8e35e1d578676d8ae2dac4f0f9c2`, preserved by the parent import.
- Only authored paths: `backend/scripts/mcr/migration_smoke.py`,
  `backend/scripts/mcr/test_migration_smoke.py`, and this document.
- No Cargo build, source migration edit, contract change, infrastructure change,
  frontend edit, source-checkout write, or push was performed.
- The parent supplies prebuilt binaries explicitly. Their actual SHA-256 values
  are recorded per service in the JSON report. This worker does not infer binary
  ancestry from their directory name; the parent binds build evidence to delivery.
- The source revision in the report is the imported migration-source baseline.
  The report also records the exact authored harness SHA-256, because the
  harness was tested as an uncommitted patch on that baseline before handoff.
  The integrator must record the eventual delivered commit ancestry separately.

## Isolation and lifecycle

The harness requires `--bin-dir` and `--report`. It never compiles binaries and
never starts a service with no arguments. All six binaries and native PostgreSQL
tools must exist before cluster creation. The default tool directory is
`/opt/homebrew/bin`; `--pg-bin-dir` can explicitly select another native install.

Each invocation creates a new mode-0700 `/tmp/mcr-pg-*` directory. Existing roots
and existing data directories are refused. A random ownership marker, directory
inode/device and user ID guard stop/removal. Data/socket symlinks, altered
markers and foreign postmaster identity are refused. `initdb` creates a new
cluster with a generated `mcr_owner` credential and SCRAM authentication. Its
temporary mode-0600 password file is removed after initialization.

PostgreSQL listens only on the owned private Unix socket: `listen_addresses`
is empty, socket permissions are 0700, and the harness checks the server's
reported data directory and disabled TCP setting. Socket port 5432 is local to
that new socket directory, so no existing TCP listener is probed or contacted.
SQLx URLs include the exact owned socket in their `host` query parameter; URLs
without this binding, foreign/default database names, foreign credentials and
ambiguous host parameters are rejected. The native run verifies this SQLx
socket connection path works.

The subprocess environment is constructed from scratch. `HOME` is omitted
entirely; `TMPDIR` remains the private temporary root. Every service and native
PostgreSQL command has an explicit working directory of that owned root, so it
cannot discover a checkout `.env` through its inherited working directory. The
read-only Git revision command explicitly uses the backend source directory.
Existing PG settings,
database URLs, provider/email configuration and service environment values are
not inherited. PostgreSQL clients use the generated socket/credential settings;
psql ignores startup files (`-X`), stops on SQL errors, and cannot prompt for a
password. A service subprocess receives only its generated service-specific
`*_DATABASE_URL` plus the minimal harness environment. No connection string or
password is printed to JSON. Captured command failures redact generated
credentials and PostgreSQL/Redis URLs and bound diagnostic size.

For each database the harness invokes the owning executable's `--migrate`, then
`--verify-migrations`. It independently compares all ledger versions, successful
flags and SQLx SHA-384 checksums against the imported migration files. It writes
one random synthetic record into a separate `mcr_smoke.records` table. It stops
and starts only its marker/pidfile-validated cluster, proves the postmaster PID
and start time changed, reads all six records unchanged, rechecks all ledgers
and invokes each service's verification command again. No application record
or source seed is required.

Cleanup runs after success, command failure, timeout, ordinary exception or
handled SIGINT/SIGTERM. Only owned `pg_ctl -D ... stop` is used; fast stop has an
owned immediate-stop fallback. There is no global process kill, Docker action,
existing-volume attachment or normal service launch. Removal happens only after
owned stop and a renewed ownership check. If ownership is uncertain or stop
fails, cleanup is reported FAIL, the run cannot PASS, and the directory is
retained with a report pointer instead of risking deletion of a foreign/running
cluster. SIGKILL or host power loss cannot execute Python cleanup.

## Executed evidence

Latest corrected execution: **2026-11-03 03:14:45–03:14:48 UTC**. Native PostgreSQL was
**16.14 (Homebrew)**. The supplied tool symlinks resolved into
`/opt/homebrew/Cellar/postgresql@16/16.14/bin/`.

From `/Users/lhs/Desktop/mcr-worktrees/be-foundation`:

```sh
PYTHONDONTWRITEBYTECODE=1 python3 -m unittest discover \
  -s backend/scripts/mcr -p test_migration_smoke.py

PYTHONDONTWRITEBYTECODE=1 python3 backend/scripts/mcr/migration_smoke.py \
  --bin-dir /Users/lhs/Desktop/mcr-build/backend/debug \
  --pg-bin-dir /opt/homebrew/bin \
  --report /Users/lhs/Desktop/mcr-build/be02-migration-smoke-cwd-fix.json
```

Both commands exited **0**. The fixture suite passed **24 tests** covering
foreign/existing directories, altered ownership/pidfiles, symlink refusal,
constructor/initdb/stop failures, ambient environment isolation, foreign/default
URLs, missing binaries, empty/duplicate migration sets, bad ledger rows,
timeouts, redaction, command failure status/exit codes, unexpected exceptions,
cleanup overriding a success and refusal to overwrite an existing report.
The corrected fixtures also assert `HOME` omission, explicit helper cwd
forwarding, and owned-root cwd for initdb, createdb and service execution.
Fixture tests do not certify a real database; the separate native run does.

| Owner | Database | Migrations / receipts before and after | Migrate | Verify before/after | Record after restart |
|---|---|---:|---|---|---|
| Gateway | `mcr_api_gateway` | 17 / 17 | PASS (0) | PASS (0/0) | PASS |
| Auth | `mcr_auth` | 12 / 12 | PASS (0) | PASS (0/0) | PASS |
| Profile | `mcr_profile` | 7 / 7 | PASS (0) | PASS (0/0) | PASS |
| Friend | `mcr_friend` | 14 / 14 | PASS (0) | PASS (0/0) | PASS |
| Post | `mcr_post` | 30 / 30 | PASS (0) | PASS (0/0) | PASS |
| Notification | `mcr_notification` | 4 / 4 | PASS (0) | PASS (0/0) | PASS |
| **Total** | **Six private databases** | **84 / 84** | **PASS** | **PASS** | **Six records survived** |

Postmaster PID changed from **23625** to **23705**. Final cleanup was **PASS**:
the owned PostgreSQL process stopped and the owned temporary directory was
removed. This is database process restart persistence, not application service
restart or a backup restore exercise.

Full redact-safe report:
`/Users/lhs/Desktop/mcr-build/be02-migration-smoke-cwd-fix.json`.

- Report SHA-256:
  `5850c4ff08a0f00e4df3367f95bd3fce1244ce4b830952c3316c968893e611e0`
- Harness SHA-256 used:
  `a46f2dc3a6dcab9310230cd75997dfcd86571c88fd066a973a8a0c3b1a8c6eb8`
- Each service row includes its binary hash, migration-manifest fingerprint,
  actual receipt counts and command exit codes. No credentials or synthetic
  record tokens are retained in the report.

The JSON report is outside the repository because this assignment authorizes
only the three paths above. This document records its result and digest; the
integrator may archive that exact report in an assigned evidence path.

The earlier first-handoff negative CLI invocation used the preceding harness with
`--bin-dir /Users/lhs/Desktop/mcr-build/missing-binaries` and report path
`/Users/lhs/Desktop/mcr-build/be02-migration-smoke-negative.json`. It exited
**1** and truthfully reported FAIL / preflight / missing `api-gateway`, with
zero services attempted and cleanup NOT_RUN because no cluster was created.
Reports use exclusive file creation; reruns must supply a new report filename.

## Limits and remaining BE02 work

This slice demonstrates the six inherited migration chains and receipt
verification on fresh native PostgreSQL 16.14, plus synthetic database record
persistence across a controlled database restart. Checksums bind the actual
applied migrations to the imported source even when the executable's compiled
migration directory originates in the parent's build checkout.

It does not qualify PostgreSQL 17, runtime least-privilege roles/grants, service
health/listeners, service-owned domain mutations, running workers, email,
providers, Redis/Kafka, full service startup/restart, Docker/k3s images or mounted
migration packaging. The disposable superuser is a fixture credential, not a
deployment role design. No new Concert service/migrations exist in this slice.
Normal-service external-mode schema probes, operational environment mapping,
image contexts and recovery/backup rehearsal remain separate BE02/BE44/QA work.
No live database, existing listener, source user data or hardware was accessed.
