# BE09 physical beacons and BE19 explicit rally

## Delivery scope and status

Native implementation handoff for `codex/mcr-be-physical-meeting`, original base
`48458d597af73bd695c8d5813b123c6a96e55321`. BE09 and BE19 remain IN_PROGRESS in the
whole-project sense: the owned implementation and synthetic PostgreSQL adapter
proof are delivered; production authority, public transport, public qualified
place revisions, subscriptions/cache invalidation and real-client IN02 are separate.
No real venue, walking route, provider or physical arrival is certified here.

Task authority: implementation specification sections 5.4, 7–8 and BE09/BE19;
C04 candidate `contracts/domain/v1/{beacon,party}.schema.json` and `policy.json`
at the original base. No contract, specification, source archive, manifests,
router or acceptance count was changed by this owner.

All meeting domain/application/service/SQL/test code is newly authored. The
native fixture derives geometry from the existing C06 synthetic
`contracts/fixtures/assets/map-manifest.json` at the base. Its `.invalid` reviewed
place/provenance assertions are test-only. The existing
`backend/scripts/mcr/migration_smoke.py::Cluster` marker/inode-owned harness is
reused; no existing database, cluster, volume or identity store is attached.

## BE09 behavior and boundaries

- Text-only C04 descriptions, explicit physical acknowledgement and party-member
  or selected-user audiences. No world position, GPS, photo URL or inferred arrival.
- Owner-scoped create/update/revoke; updates compare expected beacon revision,
  replace description/audience atomically and repeat all create validations.
  Confirmation time changes only on an acknowledged create/update.
- Operator settings default to 900 seconds and permit C04's 1–900 seconds. An
  explicitly configured event cutoff is mandatory; no settings means no write.
  DB time requires a future expiry within both TTL and cutoff. Reads also honor
  a subsequently shortened cutoff. Expired guidance cannot be renewed.
- Concrete BE08 `select_physical_in_transaction` runs in a nested savepoint inside
  the command transaction; releasing it retains binding/place SHARE locks until
  outer commit. `place_revision` is `concert_place_current_state.revision`.
  Catalog revision is a separate internal stored rebind fence. It is never
  substituted for the public place revision.
- Party beacons also persist exact party meeting and audience revision fences.
  Venue movement or any audience revision change makes old guidance unavailable
  until the owner explicitly reconfirms. Removal does not wait for an outbox job.
- Reads require current actor, owner, audience, relationship, active/unexpired
  state and reviewed/open destination. Expired, revoked, unrelated, blocked,
  removed and stale guidance returns no private description. Current DB expiry
  is checked again after potentially blocking locks.
- SQL enforces scope, revision progression, terminal revocation, TTL and closed
  audience shape with unique UUID recipients. Revocation cannot smuggle guidance
  changes or renew confirmation. Trigger time is sampled after settings locks.
- `BeaconCommands` constructs the canonical BE15 request from its exact typed
  effect input. Authorize/apply/receipt/outbox share a transaction; replay freshly
  reauthorizes. Create/update/revoke receipts contain only safe references, never
  descriptions. `0009` supplies exact C04 failure reasons; MAP_VERSION_MISMATCH
  is used only by beacon create/update, not rally/revoke.

`MeetingAuthority` returns exact-query-bound grants and retained transaction
fences. Its only supplied production default is `UnavailableMeetingAuthority`.
A cached grant, request body, public-profile permission or participant row is not
private meeting authority. Native `SyntheticAuthority` is test-only. Party reads
require both owner and actor still be members; create/update checks every
recipient, while revoke needs actor eligibility and ownership without requiring
continued access to the old audience.

## BE19 behavior and boundaries

`RallyAuthority` supplies the exact current, locked party meeting and member
revision references. Community's `party_meeting_in` is the owning query seam; the
meeting worker never writes party tables. Reads recheck current policy per peer,
filter removed/reapproved membership generations, and fail closed on policy outage.
The service checks current party meeting revision, current reviewed/open BE08
place, configured event cutoff and DB time.

Current C04 parties contain a public place/time and meeting revision but no
explicit private-beacon association. `Rally.beacon_id` is therefore null. No
latest-beacon guess or alias of beacon and party revisions is made.

`ON_MY_WAY`, `HERE`, `MET`, `NEED_HELP` and `LEFT` are explicit acknowledged
self-reports. Actor identity comes only from the authenticated context. The closed
body has no target user or world/GPS fields. First-report races use exact
event/party/actor advisory transaction locks; updates compare the self revision.
Event read-only state denies new reports. Waiting commands recheck DB expiry
before applying. Old meeting-revision commands fail without changing arrival.

Authorized old entries retain their original `meeting_revision` so consumers
render them explicitly STALE and retain the monotonic self revision for a new
report. They must never present an old HERE/MET as current. Removed users cannot
read, write or replay safe receipts. Reapproval does not restore an old report;
a new generation accepts expected revision zero and advances the stored revision.

Delivery approved the C04 receipt convention: resource and target are acting
`user_id`, revision is `RallyEntry.revision`, and exact canonical event/party path
plus full body bind the request. Invalidation is coarse event-level invalidation;
self revisions must never be compared across parties. FE20/FE23 were informed
of qualified place revision, receipt and stale-state conventions directly.

## Production composition requirement

Root HTTP/RPC/proto wiring belongs to delivery. Its raw adapter must validate
C04 bytes, reject unknown/duplicate keys and noncanonical identifiers, resolve
request-bound canonical Auth, carry the idempotency key, map operation-specific
errors, and fetch private projection only after a successful safe receipt.
`mutate_in`/`set_in` are caller-transaction seams, not alternate public endpoints.

Production authority must bind fresh fenced Auth/Friend through BE37 and the
community membership seam. Preserve a compatible lock order: community producers
use event → party root → shared policy pair → receipt budget. A naive BE37 self
pair check before acquiring the party root can deadlock with a party command that
holds the root and waits for that actor's self pair. The meeting port's actor and
audience checks must be implemented by a root-aware owning adapter, rather than
copying Auth policy or shipping that inverted wrapper. This concrete finding was
sent directly to delivery and BE37. The production default stays unavailable.

The public BE08 PlaceDetail still needs qualified current place revision exposure.
No catalog counter or synthetic fixture fills that production gap. Real-owner
fencing, public routes, subscriptions/client invalidation, real devices and IN02
remain NOT_RUN; native fixtures do not establish whole-task acceptance.

## Commits, dependencies and ownership

Meeting-only commits (integrate these, not imported dependency duplicates):

| Commit | Result |
|---|---|
| `f974cec` | First separate BE09 source checkpoint |
| `6ee0dfe` | BE19 source checkpoint after BE09 |
| `49a5286` | Party audience/meeting fences and restart receipt recovery |
| `8d6cb9a` | Durable exact C04 meeting failures and smoke migration dependency |

Explicitly authorized dependency imports in this isolated branch:

| Original owner commit | Local cherry-pick | Purpose |
|---|---|---|
| `204e09a` | `77930ef` | Delivery-owned SafeFailure / migration 0009 |
| `4bf2bd4` | `fdb23f3` | Community party aggregate and typed scope |
| `7dcf5bb` | `ffe464c` | Community maintenance outbox dependency |
| `a336a25` | `161e503` | Current party meeting/member-reference seam |
| `bcd2cd2` | `88644fb` | Typed BE37 policy port |
| `5eb6598` | `f1bb9fc` | Delivery-owned retained ring dependency edge |
| `b71ae36` | `3e65a97` | Concrete BE37 service source, not a production Auth fence |

Only exact library-export conflicts were reconciled while importing dependencies.
Owner implementations are otherwise unchanged. Imports do not claim those peer
changes are delivered to master or accepted; delivery integrates original owner
handoffs separately.

Owned paths:

- `backend/crates/concert-domain/src/{beacon,rally}/mod.rs`
- `backend/crates/concert-application/src/{beacon,rally}/mod.rs`
- `backend/services/concert-service/src/meeting/{mod,beacon,commands,rally}.rs`
- `backend/services/concert-service/migrations/0008_meeting_beacons.sql`
- `backend/services/concert-service/tests/meeting_{beacons,rally}.rs`
- `backend/services/concert-service/tests/support/meeting/mod.rs`
- `backend/scripts/mcr/meeting_smoke.py` and this document
- Exact `pub mod beacon;` / `pub mod rally;` domain/application exports and
  `pub mod meeting;` service export, reconciled by delivery during integration

No frontend imports, gateway/proto/root router, shared contract, SafeFailure
implementation, party data writes, source archive or selected originals were
changed by the meeting implementation.

## Verification

- `python3 scripts/mcr/check-plan.py`: exit 0, planning provenance PASS.
- `python3 -m py_compile backend/scripts/mcr/meeting_smoke.py`: exit 0.
- Rust 1.88 `rustfmt --edition 2021` on owned Rust source/tests: exit 0.
- `git diff --check`: exit 0.
- SQL-only runner passes on new owned PG 16.14 fixtures: migration application,
  TTL/revision/scope/terminal guards, process restart durability and cleanup.
  These are direct SQL tests, not production migration-ledger evidence.

Bounded shared-target build (explicit lease; no second target):

```sh
env RUSTUP_TOOLCHAIN=1.88.0   CARGO_TARGET_DIR=/Users/lhs/Desktop/mcr-build/backend   CARGO_BUILD_JOBS=2 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0   cargo test --manifest-path backend/Cargo.toml --locked --offline -j 2   -p concert-service --test meeting_beacons --test meeting_rally   --no-run --message-format=json
```

Exit 0 in 4.98 seconds. One unused shared fixture field warning in the rally
binary, no compile error. Both executables were copied and SHA-pinned before the
Cargo lease was actually released to delivery's next owner. Native execution used
only those copies:

```sh
python3 backend/scripts/mcr/meeting_smoke.py   --binary /var/folders/j4/41h_5mjj7j9d8f2j2bzcsngw0000gn/T/mcr-meeting-native-20261104-ecad6b5_/meeting_rally   --binary /var/folders/j4/41h_5mjj7j9d8f2j2bzcsngw0000gn/T/mcr-meeting-native-20261104-ecad6b5_/meeting_beacons   --report /tmp/mcr-meeting-native-20261104-a.json
```

Exit 0. Six native tests (four beacon, two rally) passed before and after actual
PostgreSQL 16.14 process restart, including replay of the original persisted
receipt and reauthorized original beacon. Competing edits produce one winner;
audience replacement, blocked/stranger/removed/outage states, DB expiry, closure,
revision mismatch, explicit actor scope and stale meetings are covered. Owned
cleanup PASS. The report pins every meeting source, migration and test binary.
Meeting source/migration hashes still matched after dependency imports.

Candidate-bound native report:

```json
{
  "scope": "BE09/BE19 synthetic native meeting adapters",
  "result": "PASS",
  "cleanup": "PASS",
  "production_authority": "NOT_RUN",
  "public_routes": "NOT_EXPOSED",
  "IN02": "NOT_RUN",
  "source_revision": "161e50326e2af3d8ec1a4bfa3c05c43953090f48",
  "migration_ledger": "NOT_APPLIED",
  "tests": [
    {
      "phase": "before-restart",
      "binary": "meeting_rally",
      "binary_sha256": "2ae0c5c82d32a6001b6bfcb21808b002fff3b82ce16e5818e0bae0af6ee1d7d6",
      "exit_code": 0,
      "output_sha256": "8b1f5db0d19475ddd94e136c04a278d91c7c60fbee31f0e5ec6dde8952ebf393"
    },
    {
      "phase": "before-restart",
      "binary": "meeting_beacons",
      "binary_sha256": "230ecb73bb4051414f0b8b785e4a680d43a5930d413820850b22c357a7eafcc4",
      "exit_code": 0,
      "output_sha256": "ccaf8fd0911925232922d72cd50c109008929fc14280302b52101bd849390523"
    },
    {
      "phase": "after-restart",
      "binary": "meeting_rally",
      "binary_sha256": "2ae0c5c82d32a6001b6bfcb21808b002fff3b82ce16e5818e0bae0af6ee1d7d6",
      "exit_code": 0,
      "output_sha256": "8b1f5db0d19475ddd94e136c04a278d91c7c60fbee31f0e5ec6dde8952ebf393"
    },
    {
      "phase": "after-restart",
      "binary": "meeting_beacons",
      "binary_sha256": "230ecb73bb4051414f0b8b785e4a680d43a5930d413820850b22c357a7eafcc4",
      "exit_code": 0,
      "output_sha256": "8bca029904a83415fdbfb37b6b39beb28b8cbd8b2caab7f24230406a228daa9a"
    }
  ],
  "source_sha256": {
    "backend/services/concert-service/src/meeting/beacon.rs": "470b1a0ce09f68e91be126aaf25c4325b7991a54794abe3be72e068e1088fe6d",
    "backend/services/concert-service/src/meeting/commands.rs": "f13fa91537cdc779b70c3fd11ac79cca7fbd7a86c51811a23290406a581fea78",
    "backend/services/concert-service/src/meeting/mod.rs": "f7763d36eecdcb7b23b32b720375eaaae60f7afa913c9f19ed41a3c29cb85280",
    "backend/services/concert-service/src/meeting/rally.rs": "0049a533c31777a10bb23735c03866c00204e7a34157dc6b4ec7be2d08261c5b",
    "backend/services/concert-service/tests/meeting_beacons.rs": "215b762c64d1b321187f96a14093ab126e67194f9c104d4042e09693c232e30f",
    "backend/services/concert-service/tests/meeting_rally.rs": "748f256fffd9b7ca582044cfb2d3f79c3f02a8a41db032a1cf34e068c2fc5d18",
    "backend/services/concert-service/tests/support/meeting/mod.rs": "171156836f7ef59bff64ead2ef7bc86ebb12a29b509d0880ed4bed67960890b4",
    "backend/crates/concert-domain/src/beacon/mod.rs": "ae848b599b5ee3d3cecea2d73ecf633ff2ac27d609241b2724500578f6ce7252",
    "backend/crates/concert-domain/src/rally/mod.rs": "426dba776f1e86fb0207c3de60589b72c6879131e6cb938f520517fa0c63e17f",
    "backend/crates/concert-application/src/beacon/mod.rs": "9b7bc5b64a37d2bde09552b7547160536999ea64a098411f1eeef4ebaaf0cd3b",
    "backend/crates/concert-application/src/rally/mod.rs": "0586f26e58cfcfdab9720b5532f0e16e8ac9167a757bf8e74fa4f22d4d3e2e41",
    "backend/scripts/mcr/meeting_smoke.py": "c0a4eb7db9f91490ced2ba6516491b832134069a3b1eafc852160b1434f89775"
  },
  "owned_directory": "/private/tmp/mcr-pg-xufplswd",
  "migration_sha256": {
    "0001_concert_core.sql": "9978e75e569c1ba25cb191ddf5b02bbdffcd5e64f3385173f05a885370d9215a",
    "0002_command_receipts_outbox.sql": "5f5760dfe6384cb2ab5162bcc27fbc0ed75bdca6bb85229012f6389269ee442f",
    "0003_places_map_releases.sql": "b2d4ab07a2addece8ddb4a3e413da2a32605d00ea34082a36f5940d1ff162e7a",
    "0008_meeting_beacons.sql": "b8259eb000c15059ddcba60db64518562d2cede7d2d68c42de3df8a85b44c655",
    "0009_receipt_domain_failures.sql": "e357529103bb998f337508a30b03bddc372fe387b7e587dcb7f96a8b295fa4c6"
  },
  "postgres_version": "16.14 (Homebrew)",
  "direct_sql_guards": "PASS",
  "native_adapters": "RUN",
  "restart_persistence": "PASS"
}
```

## Root-aware authority correction (source checkpoint)

The lock-order finding now has one owning fix rather than documentation-only
closure. BE37's original `f5e5d44` and `49ce464` add/strengthen
`PgRelationshipPolicy::authorize_actor_in(ParticipantKey)` with no relationship
pair read, insert or lock. It validates current actor/session/event/member evidence
and pins its earliest proof deadline; success is not target/audience permission.
Local dependency copies are `87ff594` and `9315bde` (with qualification-document
prerequisite `a5e84da` imported as `90a0205`). Peer validation remains owned by BE37.

The meeting-owned `src/meeting/authority.rs::PgMeetingAuthority` binds a canonical
actor/event per request, consumes that actor-only guard before the community root,
then uses actual `party_meeting_in`/`party_audience_in` and full BE37 pair decisions
in sorted recipient order. No party policy or Auth identity is duplicated. The
injected remote owner authority still defaults to BE37's unavailable authority
until a genuine through-commit owner fence exists.

`tests/meeting_authority.rs` exercises the actual BE37, Community and meeting
adapters, with only remote Auth/Friend proof synthetic. It creates/approves a party
through Community's command API, then holds the party root in a writer transaction.
A reader passes the actor-only guard and demonstrably waits on that root
(`pg_blocking_pids` identifies the writer). A second beacon command must demonstrably wait on the same root before taking
the global BE15 admission budget. The writer must acquire both the full self
pair and budget within 500 ms while both waiters remain blocked, create a meeting
effect and roll back. Both reader and command then complete; the rolled-back beacon must not exist. Actual community removal
must immediately deny meeting/rally access and stale the old audience fence.
This is the shared transaction schedule that the original synthetic-authority
suite could not establish.

The forward-only migration 0016 adds deferred meeting-table commit guards for beacon expiry,
rally meeting deadline, configured cutoff and any pinned BE37 owner-proof deadline.
This closes the window between protected apply and BE15 receipt/outbox commit.
Reads check proof freshness after blocking work. A separate native regression
lets either beacon or proof expire after apply and requires commit failure with
no remaining beacon. The direct SQL fixture applies successfully before the forward-only correction (`/tmp/mcr-meeting-sql-20261104-f.json`, exit 0, restart/cleanup PASS).

The correction source and new common-adapter/commit-deadline tests are NOT_RUN
until a new explicit bounded shared Cargo lease and the owned native run. The
six-test historical report above does not certify these later changes. The
existing production-unavailable adapter behavior and IN02 limits remain in force.


Published migration 0008 was restored byte for byte from `c7b0b59` after delivery
confirmed its remote publication. The deadline changes belong only to newly
reserved `0016_meeting_deadline_guards.sql`. Its ALTER adds the legacy deadline
using a constant epoch default, then drops that default. Existing report fields
and revisions remain unchanged; legacy reports have no qualified current arrival
and require a new explicit report. `tests/meeting_migrations.rs` qualifies actual
SQLx ledger fresh install, upgrade with a preexisting 0008-era report, exact 0008
checksum preservation, and replay after restart. The earlier draft `d382f13` must
not be integrated because its temporary 0008 edit is superseded; clean replacement
commits are `c299344` (concrete bridge/actor-root order) and `d78e035`
(forward 0016/deadline guards). New native migration/contention checks are pending.


Beacon admission also prelocks all old/new party roots in sorted order before
pair decisions and the BE15 budget. Destination/settings locks are retained
before budget admission, including typed stale-input refusals; apply still emits
the typed domain outcome. This prevents a beacon holding budget while waiting on
a party root whose writer needs that budget. Revoke remains actor/owner gated
without requiring membership in a former audience. No owning policy or shared
BE15 implementation was changed. The actual common-adapter contention schedule
covers this budget/root cycle, not only the actor/self-pair cycle.

The forward-upgrade fixture inserts an authentic 0008-era HERE row, verifies that
all original fields and revision survive 0016, then verifies it is absent from
current rally output. A new explicit NEED_HELP report with expected revision zero
must become current at stored revision two. SQLx replays and exact published 0008
checksum checks run on both fresh and upgraded databases before/after restart.
