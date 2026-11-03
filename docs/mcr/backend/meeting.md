# BE09 physical meeting beacons and BE19 rally

## Scope and provenance

Implementation owner: `codex/mcr-be-physical-meeting`, source base
`48458d597af73bd695c8d5813b123c6a96e55321`. All meeting domain, application,
service, migration and tests are newly authored MCR code. No upstream rendering,
identity store, source archive, place authority or artwork is copied. The
native fixture derives geometry from the existing C06 synthetic
`contracts/fixtures/assets/map-manifest.json` at that base; its `.invalid`
review/place assertions are explicitly test-only, never real venue evidence.
The existing marker/inode-owned PostgreSQL `migration_smoke.Cluster` harness is
reused, not a running/shared database or Docker volume.

Task definitions: implementation specification sections 5.4, 7–8 and BE09/BE19;
C04 candidate `contracts/domain/v1/{beacon,party}.schema.json` and `policy.json`
at the source base. These remain candidate contracts, not whole-task acceptance.

Write scope: `concert-domain/src/beacon/**`, `concert-application/src/beacon/**`,
`concert-service/src/meeting/**`, meeting-prefixed native tests, their
`tests/support/meeting/**`, `backend/scripts/mcr/meeting_*.py`, this document,
reserved migration `0008_meeting_beacons.sql`, and exact `pub mod beacon` /
`pub mod meeting` exports. Rally scope follows a separate commit.

## BE09 behavior

- Text-only C04 descriptions, explicit physical acknowledgement, party or selected
  user audience. No world coordinates, GPS, photo URL or inferred presence input.
- Owner-scoped create/update/revoke; update compares the expected beacon revision
  and repeats destination, audience and expiry validation. Updates replace the
  description and audience atomically; earlier audiences cannot fetch new details.
- Default operator TTL 900 seconds, configurable within C04's 1–900 seconds.
  `concert_meeting_settings` must explicitly supply the event cutoff; absence
  fails closed. Requested expiry must be future and within TTL and cutoff.
- DB `clock_timestamp()` controls reads, writes and final expiry checks after
  potentially blocking locks. An expired beacon cannot be renewed; revocation
  never renews last confirmation. SQL triggers enforce TTL/revision/terminal state.
- Concrete BE08 selection runs in a nested savepoint inside the caller's receipt
  transaction. Its release preserves current binding/place SHARE locks until the
  outer commit. Meeting `place_revision` checks the current place-state revision;
  catalog revision is a separate internal stored fence, never substituted for it.
- Private reads require fresh actor, audience, relationship and owner authorization
  followed by active, unexpired, still-reviewed/open destination checks. Missing,
  unrelated, removed, blocked, stale destination, expired and revoked reads disclose
  no description. Policy outages fail closed with no cached-allow fallback.
- `BeaconCommands` constructs the BE15 canonical fingerprint from the same typed
  mutation it executes. Receipt replay reauthorizes actor and ownership. Mutations,
  safe receipt and metadata-only event invalidation share one transaction.

`MeetingAuthority` is a transaction-aware, exact-query-bound port. The only
provided production default is `UnavailableMeetingAuthority`. Composition must
bind current canonical Auth, BE37 and the community membership seam; a request
body, cached grant or public-profile permission cannot grant private meeting
access. Native tests use `SyntheticAuthority`, which is never exported as a
product adapter. Party audiences require both owner and reader to remain members;
revoke needs actor eligibility and ownership without requiring the old audience.

## Adapter contract and remaining work

Delivery owns root HTTP/RPC/proto wiring and operation-specific wire errors.
The external adapter must validate raw bytes against C04, reject unknown/duplicate
keys and noncanonical identifiers, resolve request-bound Auth, pass the idempotency
key, and fetch current private projection only after the safe receipt. Do not put
private descriptions into receipts/outbox or infer globally monotonic member
revisions. Internal `mutate_in` is a caller-transaction seam, not a public endpoint.

Concrete production Auth/Friend fencing, public qualified place-revision exposure,
HTTP/RPC routes, privacy cache/subscription invalidation and IN02 are NOT_RUN.
Delivery is resolving the public BE08 place revision gap; a catalog counter is not
a valid public place revision. No real venue or walking-route safety is certified.

## Evidence at first source checkpoint

Status: IN_PROGRESS. Source implementation and native fixture authored; Cargo and
native PostgreSQL execution NOT_RUN pending explicitly serialized shared target
lease. This source commit is not a validated integration handoff.

- `python3 scripts/mcr/check-plan.py`: exit 0, planning provenance check PASS.
- `python3 -m py_compile backend/scripts/mcr/meeting_smoke.py`: exit 0.
- Rust 1.88 `rustfmt --edition 2021` on owned Rust source/test paths: exit 0.

Native qualification command after a lease-controlled locked test build:

```sh
python3 backend/scripts/mcr/meeting_smoke.py \
  --binary /Users/lhs/Desktop/mcr-build/backend/debug/deps/meeting_beacons-EXACT_HASH \
  --report /absolute/new/meeting-native-report.json
```

The runner creates a fresh owned private-socket PostgreSQL cluster, applies only
explicit meeting dependencies, runs adapters, restarts the actual PostgreSQL
process, compares durable beacon bytes, reruns on independent fixtures, and
cleans only its verified owned root. Direct SQL application is not production
migration-ledger/release evidence. Tests cover privacy, receipts, competing
revisions, database expiry, destination closure, invalid acknowledgement,
configured cutoff, and transaction-fenced removal. Results remain NOT_RUN until
reported with exact commands and exit codes.

## BE19 explicit rally source checkpoint

`RallyAuthority::party_in` must consume community's current, transaction-locked
party meeting/member-reference seam. The current party has a reviewed public
place and meeting interval, but no private beacon binding. `beacon_id` is therefore
null; selecting a latest beacon or aliasing its revision would invent authority.
The service checks the actual current party meeting revision, current membership
generations, reviewed/open BE08 destination, configured cutoff and database time.

The self-report command takes no target actor, world position or GPS evidence.
`ON_MY_WAY`, `HERE`, `MET`, `NEED_HELP` and `LEFT` are explicit acknowledgements.
The actor comes exclusively from authenticated context. First-report races are
serialized by an exact event/party/actor advisory transaction lock; subsequent
reports compare the self revision. Meeting movement rejects old meeting commands.
Authorized old reports retain their original meeting revision so clients can
render them explicitly stale and use the monotonic self revision to report again.
Removed memberships are filtered immediately, and reapproval cannot restore their
physical report. A new membership generation starts with expected revision zero
while the stored report revision remains monotonic.

Delivery approved the C04 RallyEntry receipt convention: resource and target are
acting `user_id`, revision is the resulting entry revision, exact canonical
`eventId/partyId` path and full body bind the command. Outbox records invalidate
event queries; entry revisions are never compared across parties. FE23 was
informed directly of this convention and stale-report behavior.

Additional source paths: `concert-domain/src/rally/**`,
`concert-application/src/rally/**`, `concert-service/src/meeting/rally.rs`,
`concert-service/tests/meeting_rally.rs` and exact `pub mod rally` exports. Migration
0008 adds only meeting-owned report persistence, not party/membership writes.

Direct SQL verification completed before native Cargo validation:

- `python3 backend/scripts/mcr/meeting_smoke.py --sql-only --report
  /tmp/mcr-meeting-sql-20261104-a.json`: exit 0; PostgreSQL 16.14, TTL/revision/
  immutable scope/terminal revoke guards PASS, restart bytes PASS, cleanup PASS.
- Same command with report `/tmp/mcr-meeting-sql-20261104-b.json` after adding the
  rally table/triggers: exit 0, migration application/SQL guards/restart/cleanup PASS.

These checks did not execute Rust adapters or establish migration-ledger,
production authorization, public HTTP/RPC or IN02 acceptance. Native Rust tests
are authored and await the serialized build lease.
