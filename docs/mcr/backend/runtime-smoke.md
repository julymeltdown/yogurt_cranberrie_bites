# BE02 native runtime lifecycle handoff

State: **IN_PROGRESS for BE02; native runtime slice verified, pending integrator
review.** This handoff adds six-service startup, actual health/dependency checks,
controlled outages, service/database restart persistence and owned cleanup to the
previous [database-only qualification](migration-smoke.md). It does not accept
all BE02 packaging/role requirements or BE44.

## Candidate, ownership and provenance

- Worktree: `/Users/lhs/Desktop/mcr-worktrees/be-runtime`.
- Branch: `codex/mcr-be-runtime`.
- Assignment/base: `2d9198a684ec363c476ae75835ddf4289e47762c`.
- Verified implementation commit:
  `ccd46e6bf1cb09f6835f1495cc171c5917b9ae3e`, a direct child of the base.
  The commit containing this document is a documentation-only descendant.
- Exactly three changed paths in this handoff:
  `backend/scripts/mcr/runtime_smoke.py`,
  `backend/scripts/mcr/test_runtime_smoke.py`, and
  `docs/mcr/backend/runtime-smoke.md`.
- Runner and tests are authored MCR tooling, completed from the interrupted BE02
  draft. No application logic or rendering was copied. Runtime source contracts
  are the imported services and `backend/crates/platform/src/{readiness,edge}.rs`
  at the base. The inherited port-rust source is
  `704f42149d5e8e35e1d578676d8ae2dac4f0f9c2`.
- `migration_smoke.py` is reused through its Python API without modification:
  private PostgreSQL ownership, clean environment, explicit URLs, exact SQLx
  receipts, migration/restart checks and guarded database cleanup. No change to
  that helper is required for this runner: signal protection is added around
  its critical calls by the caller.
- Dependencies: Python standard library (tested Python 3.14.3; `file_digest`
  requires Python 3.11+), explicitly supplied prebuilt service executables, native
  PostgreSQL tools, Redis, OpenSSL, and macOS `/usr/sbin/lsof`. No Python package
  install or Cargo build is performed. Actual tool paths are recorded in JSON:
  PostgreSQL 16.14, Redis Cellar `8.10.2_1`, OpenSSL Cellar `4.0.3`.
- The parent supplied the binaries from
  `/Users/lhs/Desktop/mcr-build/backend/debug`. Their hashes below identify the
  tested artifacts; the parent must bind them to its build/source evidence.
  The directory name alone is not proof of binary ancestry.
- The final report records the base as `source_revision` because execution
  preceded the implementation commit. Its exact script hash matches that
  commit; service/migration sources did not change in this patch.
- No push, master integration, frontend/asset modification, central manifest,
  router, configuration-source, infrastructure or `docs/source/` change occurs.
  Subsequent avatar consumers must follow the parent's newly registered JPG
  catalog; C06 synthetic golden metadata is not product artwork.

## Isolation and checks

The runner preflights all six binaries and tools, creates a fresh marker/inode-
validated mode-0700 private PostgreSQL root, and applies the six owner migration
sets in six `mcr_` databases. Generated credentials use SCRAM authentication.
PostgreSQL TCP is disabled: port 5432 exists only within the newly created private
Unix socket namespace, with explicit URLs bound to that socket. No source-default
TCP database listener is contacted.

Every child runs from the owned root with a newly constructed environment.
`HOME` is omitted, never changed; ambient database, Redis, provider, mail, proxy,
AWS and telemetry settings are not inherited. OpenSSL additionally receives
`OPENSSL_CONF=/dev/null`. RSA/Ed25519 keys, workload credentials, HMAC keys and
Redis authentication are generated per invocation. Private key files are removed
once read; remaining owned configuration/logs are removed during cleanup.

Service HTTP/metrics/gRPC and Redis listeners use reserved exclusive nondefault
loopback ports. Before each probe, PID-filtered `lsof` must show exactly the
child's expected loopback listener set. Unexpected listeners or foreign/default
URLs fail before probing. Reservations cannot eliminate an external bind race;
ownership is checked again before health requests, and collisions fail without
attaching to or killing another owner. Restarts allocate **fresh service ports**
and rebuild peer/callback URLs; initial and restarted topologies are both in JSON.
No socket-reuse option weakens exclusivity. Redis keeps its owned port and PID.

Redis is an actual authenticated private server, with persistence disabled for
this fixture. AUTH/PING is separately checked before and after service restart;
Post readiness does not probe Redis. The runner does not claim Redis outage or
Redis restart persistence. Kafka is explicitly disabled, mail destinations and
provider credentials are unset, and optional provider URLs point at a held,
non-listening local port. External email/OAuth/Kafka and Docker/global services
are not used or restarted.

For each service, metrics `/livez` must return 200. Public-listener `/livez`
responses (404 in this run) are recorded only as listener responses, never as
readiness. Metrics `/readyz` is validated against actual producer field names,
required/optional edge flags, booleans, all check statuses and HTTP semantics.
Gateway's key probe is named `signing-keys`. Both Gateway signing-key refresh
and minimum-refresh settings are explicitly 5 seconds, satisfying its validator.
All probes, including optional ones, must be up before a normal startup passes.

Execution then writes one synthetic record per database in
`mcr_smoke.runtime_records`, independently of migration-only records, and:

1. Stops the owned PostgreSQL postmaster. After readiness-cache expiry, all six
   services retain liveness 200 while readiness becomes 503 and DB checks fail.
2. Restarts PostgreSQL, proves its PID changed and waits for every service/edge
   to recover without restarting the services.
3. Stops only the owned Profile service with DBs available. Gateway and Friend
   fail required readiness (503); Auth and Post retain ready=true (200) with
   degraded=true for their optional Profile edges. All four retain liveness 200.
   These gRPC health probes report peer-local health, not recursive business
   dependency propagation.
4. Stops the remaining services cleanly and starts all six with fresh explicit
   listener/peer ports. Proves each service PID changed, all health checks are up,
   each synthetic runtime record survived and all 84 migration receipts match.
5. Stops every owned child and PostgreSQL and removes only the owned root.

## Failure and termination behavior

Startup/listener/readiness waits, health requests, SQL/native commands and child
shutdowns are bounded. Failed exits, malformed health, timeout and interruption
produce FAIL evidence. Direct unreaped child handles receive termination, then a
bounded kill fallback; no process-group/global kill is used. Forced/unexpected
child exits prevent a PASS result even when cleanup confirms removal is safe.
Cleanup attempts every child even after another stop fails. PostgreSQL is still
stopped after a child-stop failure, but files are retained if a process might
remain. Marker/inode/postmaster mismatches refuse destructive cleanup.

SIGINT/SIGTERM is deferred across root ownership registration, the bounded
migration qualification stage, child spawn/registration, and explicit stop/start
operations. This avoids orphaning an unregistered child or racing `pg_ctl`'s
forked postmaster. Repeated termination signals are ignored during cleanup and
JSON emission. Reports use exclusive mode-0600 creation, bounded/redacted
failure diagnostics and an fsynced JSON result; existing evidence is never
overwritten. SIGKILL, power loss and failure of the report filesystem cannot be
recovered by Python cleanup.

## Commands, results and evidence

Executed from the assigned worktree:

```sh
PYTHONDONTWRITEBYTECODE=1 python3 -m unittest discover \
  -s backend/scripts/mcr -p test_runtime_smoke.py -v
PYTHONDONTWRITEBYTECODE=1 python3 -m unittest discover \
  -s backend/scripts/mcr -p test_migration_smoke.py
python3 scripts/mcr/check-plan.py
git diff --cached --check
PYTHONDONTWRITEBYTECODE=1 python3 backend/scripts/mcr/runtime_smoke.py \
  --bin-dir /Users/lhs/Desktop/mcr-build/backend/debug \
  --report /Users/lhs/Desktop/mcr-build/be02-runtime-final-01.json
```

All commands exited **0**. Runtime guards: **31 tests PASS**; unchanged migration
helper guards: **24 tests PASS**. Planning checks PASS (planning scope only).
Guards cover foreign/default URLs and directories, marker/listener ownership,
ambient connectors, missing binaries, collisions/fresh restart ports, malformed
or contradictory readiness, exact signing-key name and edge flags, timeout and
failed exit semantics, redaction, host OpenSSL configuration isolation,
interruption between spawn and registration, repeated cleanup signals, cleanup
failure retention, and exclusive evidence creation. They do not replace the
native runs.

Final native run: **2026-11-03 12:53:47–12:54:01 KST**
(`03:53:47–03:54:01 UTC`), result **PASS**, cleanup **PASS**.

| Service | Migration receipts | Initial/restarted readiness | Runtime record after restart |
|---|---:|---|---|
| Gateway | 17 | 200 / 200, every probe up | PASS |
| Auth | 12 | 200 / 200, every probe up | PASS |
| Profile | 7 | 200 / 200, every probe up | PASS |
| Friend | 14 | 200 / 200, every probe up | PASS |
| Post | 30 | 200 / 200, every probe up | PASS |
| Notification | 4 | 200 / 200, every probe up | PASS |

All 13 normal service/Redis shutdowns recorded exit 0, without forced exits.
A read-only `ps -p <recorded-PIDs> -o pid=,comm=` check found no remaining recorded
service/Redis/postmaster PID; the exact owned directory was absent after cleanup.

Two fresh native interruption runs also passed their expected failure/cleanup
criteria. A Python driver launches the same CLI with a new report, sends signals
only through its own `Popen` handle, and waits for exit. For the migration case it
sends SIGINT after 0.3 seconds, then SIGTERM 0.05 seconds later. For normal-service
startup it polls `pgrep -P <runner-PID> -x api-gateway`, confirms via PID-filtered
`ps -o args=` that the child is the bare service executable (not a `--migrate` or
`--verify-migrations` invocation), then sends SIGTERM followed by SIGINT after
0.05 seconds. Each driver asserts exit 1, FAIL/KeyboardInterrupt, cleanup PASS,
removed owned root and, for the service case, seven stopped children. Both
reports' recorded PIDs were absent in post-run checks. The driver exits **0**
when those assertions pass.

Evidence stays outside Git under the assignment's explicit three-path scope:

| Artifact | SHA-256 |
|---|---|
| `backend/scripts/mcr/runtime_smoke.py` | `da2ac13ca1502e80a2528c982bf28b61c06707b4018c2a436e7f8bf41512e7a8` |
| `backend/scripts/mcr/test_runtime_smoke.py` | `54f90c0061fe9abd7bdab8938aaf308c3e11f5215e7ab227d3252a954f699189` |
| unchanged `backend/scripts/mcr/migration_smoke.py` | `a46f2dc3a6dcab9310230cd75997dfcd86571c88fd066a973a8a0c3b1a8c6eb8` |
| `/Users/lhs/Desktop/mcr-build/be02-runtime-final-01.json` | `76619d6c9a9c83ce2d8cd90775e5fc462afc2723c744045ab6420964691d2e1c` |
| `/Users/lhs/Desktop/mcr-build/be02-runtime-interrupt-migration-01.json` | `a120c2772f4a4c991a99dfbf399983e705ba0b21a355bfddabcc828a0640c4dc` |
| `/Users/lhs/Desktop/mcr-build/be02-runtime-interrupt-services-02.json` | `15ab0aa3dd833ddedd3fbb53e57e24977e2813f826374e17629740cd0ff8aac8` |

The earlier `be02-runtime-initial.json` failure is preserved. Resumption attempt
`be02-runtime-resumed-01.json` also failed honestly on minimum-refresh default
10 exceeding the configured refresh 5; both values were then made explicit.
`be02-runtime-resumed-02.json` passed the simpler database-outage/restart flow;
`be02-runtime-peer-outage-01.json` passed the added peer-outage flow. All cleanup
results were PASS. `be02-runtime-interrupt-services-01.json` interrupted a Gateway
migration CLI, so it is **not** normal-service interruption evidence; the corrected
`-02` report above is. None of these files was overwritten.

Tested service executable fingerprints:

| Executable | SHA-256 |
|---|---|
| api-gateway | `8d7c69bd270971c04cc4fbcd5cc87962911eba0fc8a2162c266a6326365c3672` |
| auth-service | `c746456b2401a5a77ddb6bc050bad267f36e4f4228171348f510054bdb9b7163` |
| profile-service | `b56a3a05b2aa59c1ed018608ce94ef9d673ccbc8d04c746d645b2ecc24587ffc` |
| friend-service | `86de8d434db675a7d5707adb2a6dbfa0e2011e9c9d55c118b616674c010fbdab` |
| post-service | `965f56cd25518ee004f795036979098649b29cf78944a34523a5cda596a7c915` |
| notification-service | `c1be3ea3b1ee80a310e9d6d96a046438ad457718face137100b01d45aa11153d` |

## Limits and integrator follow-up

Parity mode selects inherited loopback-compatible adapters/routes. It does not
implement or accept C03's disabled OAuth policy, real signup/login/refresh/logout,
provider handoff, cookie/origin protections or production security. Auth journeys
and providers are **NOT_RUN**. The synthetic SQL records prove database durability
through actual service/DB restarts; they are not service-owned business mutations.

PostgreSQL 17, least-privilege runtime grants, distribution/image contexts, mounted
migration packaging, TLS, Kafka, backup restore, domain/outbox replay, Concert,
browser/device/multiuser flows and BE44 remain **NOT_RUN**. The temporary database
owner is a fixture superuser, not a production role design. Redis AUTH/PING proves
the owned dependency is available, not that a Post business path used Redis.
The native harness is macOS-specific due to `lsof` and the qualified local tools.

The main integrator should review the exact three paths and ancestry, archive
selected JSON files if desired, bind the supplied binaries to existing build
proof, update its owned progress/evidence registry and decide the remaining BE02
criteria. No out-of-scope source fix was required. Only the main integrator may
merge/publish to `julymeltdown/mcr/master`.
