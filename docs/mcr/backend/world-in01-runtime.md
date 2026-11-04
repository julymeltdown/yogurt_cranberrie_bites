# Isolated World startup

Status: `IN_PROGRESS`. World remains `CLOSED`; IN01 two-client execution is
`NOT_RUN`. Optional Age, Social, Gift, Feed and media modes are not prerequisites.

`backend/scripts/mcr/world_in01_prepare.py` owns disposable PostgreSQL, Redis,
six native services, real Auth mail delivery and, with `--bootstrap`, the actual
Concert bootstrap RPC listener. It creates two distinct principals through
Gateway signup, email verification and login. Access tokens remain in memory.
Separate mode-0600 principal files contain credentials for subsequent real login
inside the owned mode-0700 root. No Auth rows, JWTs or sessions are fabricated.

`--concert-bin` runs real Concert migration and ledger-verification commands on
a newly created `mcr_concert`. Only operator configuration is seeded: an event
and one two-person room with selected map4/avatar pins. Admissions, participants
and avatars are asserted empty before actual authenticated bootstrap.

`--publish-catalog` calls the production binary's `--prepare-in01-catalog`.
It loads `ProductReleaseBundle::from_source_bound_files`, validates map4 through
`bootstrap::spatial::load_candidate`, publishes through
`PgPlaceRepository::publish_release`, and checks repository readback. Release
paths, source binding and resource bytes retain the actual bundle hashes.

`--bootstrap` requires catalog publication. Before stack startup, an isolated
configuration hook supplies the actual Gateway credential and actor verifying
keys to Concert and configures Gateway's Concert endpoint. Each real principal
initializes Profile through `PUT /api/profile/{ownUUID}`, then requests
`GET /api/concert/events/{event}/bootstrap`. The harness checks the response's
owner and exactly two resulting admissions, participants and canonical avatars.
It does not enable World settings or register unfinished production owners.

## Actual evidence

The portable receipt is `world-in01-preparation.json` beside this document.
Concert library and production binary were compiled from
`4d5c044fa1b09cfd794a519d963bcd63d22c26d8` with Rust 1.88, locked/offline,
two jobs, debug info disabled and incremental compilation disabled. Both builds
exited 0. Executables and source pins were copied outside the build target;
zero target handles were confirmed before target release and all native runs.
The copied library passed eight Participant tests (four ignored) and seven
Product RPC tests. These checks do not certify HTTP or multiuser World behavior.

Actual isolated startup results:

- All six existing service binaries migrated, verified, survived PostgreSQL
  restart and passed current readiness; Redis and Auth mail delivery operated.
- Two canonical Auth identities completed signup, verification and login, with
  real refresh-cookie attributes; both public Profile writes returned 200.
- The newly compiled Concert binary applied and verified 27 migrations.
  Source-bound map4 catalog publication and repository readback passed.
- The schema probe passed, failed when the required avatar representation
  constraint was temporarily removed, then passed after rollback.
- Both public bootstrap requests returned 200. Actual bootstrap created the
  two admissions, participants and canonical avatars; none were SQL-seeded.
- Concert stopped with exit 0 without forced termination. All owned resources
  stopped and the private root was removed.

Migration 0025 replaces `concert_avatar_contract` with
`concert_avatar_representation`; the corrected production schema probe was
compiled and exercised in this run. The receipt pins compiled Concert source,
all actual migration digests and each separate existing service binary hash.

Example from the repository root:

```sh
python3 backend/scripts/mcr/world_in01_prepare.py \
  --bin-dir /absolute/private/six-service-binaries \
  --concert-bin /absolute/private/concert-service \
  --publish-catalog --bootstrap \
  --report /absolute/private/new-startup-report.json \
  --hold-seconds 300
```

While held, stdout exposes only a private descriptor path. Credentials and
inputs expire when the owned process exits; the default hold is zero. A removed
path is not a live runtime handoff. The command reports `PARTIAL`, never IN01
acceptance. Existing six-service binaries retain their separate provenance;
compiling Concert does not rebuild or qualify all six services.

Still required: the final qualified World native fixes and production-owner
registration, real room/ticket HTTP requests, two actual WebSocket clients,
revocation and shutdown evidence, and the frontend consumer handoff. C05's
motion-profile and input-application timing gap also remains open for full
FE17 prediction/reconciliation; hidden server constants are not a negotiated
public contract. Current map4 startup does not depend on new art or map5.
