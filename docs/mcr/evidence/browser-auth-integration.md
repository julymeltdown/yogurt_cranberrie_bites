# BE03 email ingress integration review

Date: 2026-11-03. Candidate `689e4d27c67debcd05d7280394184619cb4aea3b`.
Worker implementation `397e1d6` and handoff `41fb33c` were integrated as
`9f8d5f0` and `689e4d2`. The bounded email adapter increment is **INTEGRATED**;
BE03's complete provider/browser scope and C03 runtime coverage are not accepted.

Parent reviewed the composed executable/router boundaries, exact origin and
intent policy, no-store responses, unavailable legacy/OAuth paths, reserved Auth
admission/journal mapping, high-risk live-session checks, producer credential
classification, reset URL validation and regression changes. No frontend source,
contract, manifest/lock or migration changed. The complete 22-path handoff matches
the assignment. The generic lane guard still rejects the two assigned scripts and
`infra/mcr/backend/verify_local_auth.py`; these three exact delegated tool paths
were manually reviewed. This is not a blanket infrastructure exemption or a claim
that the generic guard passed. HTTP mod.rs composition was explicitly delegated.

Every public Gateway mutation now requires Origin and browser intent, including
non-Auth/admin routes. This is an intentional C03 browser boundary; internal calls
use authenticated gRPC. Downstream clients must send the browser header on writes.
Auth credentials never enter the inherited durable mutation journal at the new
`/api/auth/*` paths. Optional OAuth and email enumeration routes remain unmounted.

## Integration checks

All commands ran from the integration checkout and exited 0.

| Command | Result |
|---|---|
| `CARGO_TARGET_DIR=/Users/lhs/Desktop/mcr-build/be03/target CARGO_PROFILE_DEV_DEBUG=0 cargo +1.88.0 test --manifest-path backend/Cargo.toml --locked -j 2 -p api-gateway --test browser_auth_http --test unit` | 12 composed browser-policy tests plus 209 inherited unit tests pass |
| `python3 -B -m unittest discover -s backend/scripts/mcr -p test_browser_auth_smoke.py` | Nine ownership/failure/redaction tests pass |
| `python3 -B backend/scripts/mcr/browser_auth_smoke.py --bin-dir /Users/lhs/Desktop/mcr-build/be03/bin --report /Users/lhs/Desktop/mcr-build/be03/integrated-689e4d2-native.json` | 53 real HTTP checks, three private local mail deliveries; six services healthy; cleanup PASS |

The [native receipt](browser-auth/native-email.json) binds the integration SHA,
128 Gateway/Auth production source hashes, four frozen inputs, the harness,
six actual binary hashes and migration manifests. Parent verified every source
and input hash matches and the disposable cluster directory was removed.
The run reused the worker's qualified binaries: their production sources match
the integration candidate exactly. It is not a claim of a new production binary
build at the integration SHA. Worker build, 124 Auth unit tests and ten targeted
Auth configuration tests are separately bound in the [handoff](../backend/browser-auth.md).

Native checks include denied requests with zero Auth-table/mail effect, exact
UTF-8 password boundaries, real verification/reset delivery, refresh rotation,
immediate reuse conflict, family revocation outside grace, expiry, logout and
password-reset invalidation. Time-based fixtures alter only the owned database.
No existing service, database, mail provider or browser profile was used.

## Remaining scope

Real Chrome/Safari cookie handling, multiple tabs, frontend state cleanup,
production TLS/domains, configured OAuth handoff, global erasure, multiuser flows,
devices, PostgreSQL 17 and release/load tests remain **NOT_RUN**. The unavailable
Docker fixture is preserved as NOT_RUN. The parent gate counts remain unchanged.
The assigned email increment and its review work are delivered; the worker may
complete that scope goal and start the next explicitly assigned persistent goal.
