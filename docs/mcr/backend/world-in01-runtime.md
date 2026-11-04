# Isolated World startup

Status: `IN_PROGRESS`. Public backend C03 and a bounded actual two-client World
flow passed. A new diagnostic production cohort is held through 04:55 UTC on
2026-11-04 and its expiring private bundle has reached the designated frontend
team. Sustained World serving previously failed intermittently; that incident
remains unresolved and browser IN01 remains `NOT_RUN`. The historical startup
baseline below is superseded by the dated followups. Optional Age, Social, Gift,
Feed and media modes are not prerequisites.

The portable `root-native-e6921cf2.json` receipt records the coherent diagnostic
build, both shutdown-result tests, and a passing actual ungated HTTP owner
qualification. The latter uses genuine signup, verification, login and Profile
creation, then verifies terminal delivery and settlement on the public router.
Its reset and expiry cases deliberately retain owner quarantine until isolated
teardown. This is distinct from the earlier gated-body framing qualification.
The ongoing World hold has no final cleanup or browser acceptance result yet.

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
# Supervised World and C03 composition followup

Fresh source `f2898a642848cbf015cb32664ce3a41b9d83314d` compiled both the
Concert main test executable and production executable. Both supervision tests
passed after the build target was released. The actual mTLS launcher
`backend/scripts/mcr/world_in01_live.py` then created two real Auth principals,
initialized Profile through public HTTP, bootstrapped them, and exercised the
public C03 probe. All 18 HTTP steps passed, including stale-CAS 409 and anonymous
401. Two canonical Profile native commits were present.

The overall serving proof FAILED: after a bounded three-second read-only
observation, all 16 Root scopes were ACTIVE/serialized with no delivery
completion, both owner completion counts were zero, and 32 prepared owner
transactions remained. All owned processes stopped and the private cluster was
removed; this cleanup is not settlement acceptance. The exact sanitized receipt
is [root-startup-f2898a64.json](root-startup-f2898a64.json). An earlier harness-only
failure incorrectly expected SQL NULL for `profiles.bio`; the retained NOT NULL
column stores an empty string for the public `bio:null` projection.

The actual Gateway retained-body layer is mounted. The identified defect is that
its exact inner size hint allows HTTP/1 transport to finish after the declared
DATA byte count without polling the terminal completion future. The followup
must mask scoped size hints and remove scoped Content-Length, then qualify real
HTTP framing and rerun this same two-principal public/DB proof. No manual scope
completion or SQL owner settlement is an acceptable workaround.

The Root entrypoint now mounts the opt-in Participant factory, exact C03 method
policy, Profile-only Decision verifier, Auth/Friend-only settlement, one shared
Delivery publisher, and a supervised Participant recovery loop. It refuses
simultaneous independent Product and Participant owners before opening the
database; a future union must share the canonical scope and delivery identity.

When World is enabled, Bootstrap and Participant use the exact validated release
`Arc` supplied by World settings, including the World feature capability and
source digest. The gRPC listener waits for World’s one-shot readiness after real
replay catchup, room claims and owner maintenance. A dropped startup sender fails
closed. After startup, the World future remains supervised. Component failure
requests shared shutdown and joins peer futures so native replay and membership
cleanup can finish, rather than abandoning them on the first error.

`--inspect-product-release` validates the same release configuration and prints
only its digest without opening a database/listener. Runtime tooling can use
this value for the Gateway and Participant release bindings rather than copying
a digest from a different feature configuration.

## Actual EOF correction and World hold qualification, 2026-11-04

Gateway source `3710a029d083e4b5123df45b82867056f64b3c8b` removes scoped
Content-Length and hides the inner exact byte count until response completion.
All three advertised build targets passed with Rust 1.88, locked/offline, two
jobs and no debug/incremental output. The copied executables were frozen and
the target was released with zero handles before execution. Three platform
body tests and three actual TCP framing tests passed, including fixed-length,
zero-length and unchanged unscoped responses. This fixes the concrete terminal
polling defect without manually completing an owner scope.

The same real two-principal public C03 flow then passed all 18 HTTP checks.
All 16 Root responses reached durable delivery completion, both Auth and Friend
recorded 16 scope-settlement proofs, two canonical Profile receipts and native
commits existed, and no prepared transaction remained. The later held runs
strengthened the oracle: each owner had 16 leases in `COMPLETED`, and both
participant ledgers contained zero `PENDING` rows. The first public C03 report
did not query those additional columns; its evidence keeps that distinction.
The passing C03 evidence is in
[public-c03-3710a029-after-eof-fix.json](participant-live-evidence/public-c03-3710a029-after-eof-fix.json).

The World-enabled launcher used actual public signup/login and the same frozen
Gateway plus Concert `f2898a64` production binaries. Probe source `c3a0d4b4`
obtained two public tickets, authenticated two real sockets, verified mutual
membership, consumed movement and stop acknowledgements, peer replication and
disconnect removal. Read-only database checks confirmed two issued/consumed
tickets, zero remaining memberships and zero prepared transactions. The bounded
run stopped Concert with unforced exit 0 and removed its owned cluster. The
probe's ten local tests are separate fixture coverage, not this live proof.

The prolonged hold is still a real open gate. Its first attempt exposed a
launcher-only readiness parser mismatch after startup; source `50e7f0b9`
corrected that scope. The next attempt passed C03 and the World probe but Concert
then exited 1 with `world authority is unavailable`. Owned cleanup passed, but
this is not sustained-serving acceptance. The World owner is investigating the
actual maintenance/replay/lease path. Neither expired private descriptor was
handed to the frontend, and no browser consumer is reserved against it.

[root-native-3710a029.json](root-native-3710a029.json) preserves exact build,
binary, original-report and test-log hashes, source revisions, measured counts,
release ordering and both failed holds. Source-bound map4 and the approved
selected static avatar remain the runtime release. New spatial/art candidates,
optional producers, a negotiated movement profile, real-stack revocation,
browser IN01 and physical/device validation remain separate unqualified gates.

The qualified World source `82803760` was integrated as `2658000b`, including the
missing immutable migration 0026 and the full 18-path final packet. Earlier
individual fixes alone were insufficient because their replay implementation
was not yet present in Root. Readiness followups `68b68338`/`4c0a0468` are mapped
to `36adc136`/`55470af8`; the preceding cleanup regression test was retained in
the test-file conflict. Participant factory `e7c0cbb8` maps to `161a3bd6`.
These mappings do not certify the combined Root entrypoint: its fresh compilation,
supervision tests and actual public C03/World serving proof remain NOT_RUN at
this source checkpoint.
