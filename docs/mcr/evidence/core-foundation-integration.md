# Backend foundation and HTTP transport integration — 2026-11-03

Reviewed candidate: `360adfdb1e39b43c86f986850e0d9a7f604604cd`.
This qualifies BE01 and FE01 against their bounded task criteria. BE02 remains
IN_PROGRESS; no IA entry, real authentication journey or integration gate passes
as a consequence of these checks.

## Ownership and reviewed behavior

Both visible worker chats started at `2d9198a684ec363c476ae75835ddf4289e47762c`.
Backend implementation `ccd46e6bf1cb09f6835f1495cc171c5917b9ae3e` and its handoff
`0fb22af06fbc139a8f9b6740d8cf780eaa55557a` were integrated as `180d191` and
`a497814`. Frontend implementation `60a457b76373d56ceb39fa110e6321b0883aa00e`
and handoff `d76cbb5ca7ff419cf8c4e6ac1d587e4c69c42644` became `6313950` and
`360adfd`. All eight changed paths match their exact lane assignments: three
backend tooling/documentation paths and five frontend transport/test/documentation
paths. No shared schema, manifest, lockfile, router, style or source archive changed.

The parent reviewed memory session compare-and-set, cancellation before/after
decoding, refresh-flight ownership, strict same-origin routing and bounded error
projection. FE01 tests cover 401 read recovery, 409/429 normalization, network and
abort races, stale identity responses and native FormData bytes. Public requests
do not inject a bearer; credentials have no persistent storage. New transport
code uses retained Ky 2.0.2 and does not copy Bigyeting rendering or modules.

Backend review covered explicit private database URLs, clean child environments,
reserved nondefault loopback listeners, PID ownership before probes, bounded
process shutdown and cleanup ownership. Six inherited binaries remain separate
services; there was no old competing MCR executable to remove. The historical
proposed ports in the source audit are superseded for this native harness by the
actual `configuration.ports` and `configuration.restarted_ports` in the
[integrated runtime receipt](be02-runtime-integrated.json). The service rows bind
each process, database, binary hash and health result. PostgreSQL has no TCP
listener; its port 5432 is only within a fresh private Unix socket namespace.
The gateway database is `mcr_api_gateway`; the other five are `mcr_auth`,
`mcr_profile`, `mcr_friend`, `mcr_post`, and `mcr_notification`.

## Parent execution

| Command / check | Result |
|---|---|
| Pinned Node 24.22.2, `corepack pnpm@11.7.0 test src/shared/api src/shared/ui` | Exit 0; 166 tests (144 transport/error, 22 existing UI). |
| Same toolchain, `pnpm run build` then `pnpm run lint` | Exit 0; strict TS/Vite production build and scoped Oxlint. |
| `PYTHONDONTWRITEBYTECODE=1 python3 -m unittest discover -s backend/scripts/mcr -p 'test_*smoke.py'` | Exit 0; 55 tests (31 runtime, 24 migration). |
| `python3 backend/scripts/mcr/runtime_smoke.py --bin-dir /Users/lhs/Desktop/mcr-build/backend/debug --report /Users/lhs/Desktop/mcr-build/be02-runtime-integrated-01.json` | Exit 0; actual six-service startup, DB/Profile dependency failures, recovery, fresh service PIDs/ports and persistence. Receipt copied byte for byte into this directory. |
| `cargo +1.88.0 build --locked --workspace` in `backend/`, explicit external target, jobs=2, dev debug=0 | Exit 0; [incremental build log](foundation/backend-integrated-build.log). All six resulting binary SHA-256 values match those in the executed runtime receipt. The earlier clean import build remains in [foundation evidence](foundation.md). |
| `python3 scripts/mcr/check-plan.py` | Exit 0; original source checksums, exporter regressions and delivery routing pass. |
| Pinned Node, `node scripts/mcr/check-ui-origin.mjs --root /Users/lhs/Desktop/mcr` | Exit 0; 50 scanned files, 43 content files, zero violations. |
| `git diff 4a070e2..360adfd --check` and exact changed-path review | Exit 0; assignments preserved. |

Runtime receipt reports 84 verified migration rows and six synthetic SQL records
surviving restart. All thirteen owned service/Redis child stops exited zero
without forced termination; the private temporary root no longer exists after
cleanup. PostgreSQL restart and shutdown are separately guarded by its helper.

## Limits and continuation

The persistence records are synthetic SQL records, not service business writes.
Native health, isolation and cleanup do not qualify Docker images, production
database roles, TLS, Kafka, external providers, BE44 or future Concert behavior.
BE02 packaging and a foreground local-development stack are the backend chat's
next scope. The frontend chat is implementing FE02 query/privacy isolation.

FE01 is accepted as the specified mock-tested transport task. Real Secure
HttpOnly cookie rotation, cross-tab coordination, login/reset journeys, late
Set-Cookie races and server enforcement remain BE03/FE09 obligations. Automatic
read refresh is an opt-in port, not enabled product behavior. Generated
operation-specific decoders remain C07/consumer work.
