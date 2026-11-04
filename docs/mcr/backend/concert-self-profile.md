# Canonical Concert self Profile producer

Status: IN_PROGRESS. Base: `8f89d1905a86143d34ff77d2951fc60651ab2497`.
BE-PROFILE-SELF owns only newly assigned Profile SELF files and the explicitly
allocated new migration `0008_concert_self_profile.sql`. Public Profile retains
all four existing registration files; ParticipantLive owns client/orchestration;
canonical authority owns actual Auth/Friend guard and settlement contracts.

The canonical stored content remains `profiles.nickname` and `profiles.bio`.
Display names are 1..40 Unicode scalars, nonblank and control-free; bio is at most
160 scalars and control-free. Omitted bio preserves it; explicit null clears it;
empty stored bio projects as absent. Username, avatar, counts and privacy do not
belong to this producer. Private self content is not a public profile DTO.

The dedicated `profile.self.v1.ConcertSelfProfileService` takes both actual owner
GuardView capabilities and exact GuardBinding. No bounded Auth observation is
converted into mutation authority. PrepareUpdate consumes exact signed PATCH
bytes and derives selected content fields. `operation_id` is the existing request
UUID; no browser idempotency header is added. Profile epoch/revision is internal
and distinct from the C03 event participant `expected_revision`.

Cross-store writes require durable preparation, participant CAS and an actual
coordinator decision. A proposed value is not committed content. Unknown native
transaction outcomes retain exclusion and prepared effects; deadline expiry is
not permission to release them or guess rollback. ReadOperation and Reconcile
cannot originate another mutation. Full coordinator/guard interface and native
qualification are in progress; nothing here certifies a live PATCH flow yet.

Source authored for this task; no rendering or dependency upgrade. Full RPC
registration and native multi-store checks: NOT_RUN.

## Guarded RPC source checkpoint

The new handler checks the dedicated Concert workload and verified signed actor,
the original verified deadline, exact SELF subject, both actual owner views,
Auth live session/active actor/positive security revision, and Friend active actor
without either malformed self-block flag. Profile privacy remains valid for SELF.
The binding digest delegates to the shared published `owner_binding` helper.
Both actual owner Track calls finish before any protected content read or write.

Positive reads retain a separate durable read reference and actual SQL identity,
then roll back before returning content. They create no dummy write revision or
receipt. PrepareUpdate records an immutable operation identity before Track,
derives only selected fields from the exact body, checks the internal Profile
version, and prepares content, receipt and audit in the same actual transaction.

Reconcile queries the configured Concert decision service using the dedicated
Profile decision signing key. The response must echo the exact stored binding,
GID and effect digest. Unknown replies retain prepared effects. Serialized
settlement rechecks native PG state; committed status additionally requires the
exact effect/command receipt. The background recovery method discovers existing
unresolved operations with fair scheduling hints; those hints never prove a
transaction outcome. Auth/Friend settlement queries use the same native proof.
Profile ResolveScope always reports unfinished because Concert owns HTTP delivery.

Runtime configuration is additive: missing `CONCERT_PROFILE_SELF_VERIFYING_KEYS`
disables SELF registration. If present, it must be valid and requires explicit
`PROFILE_SELF_AUTH_GRPC_ENDPOINT`, `PROFILE_SELF_FRIEND_GRPC_ENDPOINT`,
`PROFILE_SELF_CONCERT_GRPC_ENDPOINT`, and
`PROFILE_PARTICIPANT_DECISION_SIGNING_SEED`. No endpoint or decision seed is
guessed. The shared `concert_self_profile.rs` key helper was adapted from the
audited platform `concert_read.rs` at base `30be9c9c`, with fixed Profile audience,
Concert issuer and dedicated base64 key variables. It rejects Gateway trust and
Gateway/Concert/Profile parity keys. Its exact new-file ownership was transferred
by canonical authority; the parent module export and method policy remain theirs.

The new RPC modules now compile; their actual external RPC qualification is
NOT_RUN. Amended 0008 has SQL and current-binary qualification below. Neither
storage qualification certifies a real owner/coordinator RPC flow.

## Amended schema SQL qualification

`concert_self_profile_schema.py` applies the exact current migration SQL to its
own marker/inode-checked PostgreSQL 16.14 instance. The first attempt applied all
DDL then refused a missing exact test match caused by a doubled prefix; its
cleanup passed. After fixing that runner filter, `schema-report-2.json` passed
three historical frozen storage tests plus a direct native read-reference test.
The read connection's actual full XID is live while an independent reference
commits; positive content is read afterward, rollback proves ABORTED, the control
reference remains and no write receipt exists. Prepared write effects survive a
controlled restart; exact native commit/abort and receipt behavior pass. Cleanup
passed. Amended migration 0008 SHA-384:
`302533481e533b98362d2452db7f91f85eb6d08fe13a8d307877de05de57588e6358547f577f3d910418d48f7f61afa3`.

This is source SQL qualification using the private historical storage test binary
SHA-256 `1564b5f50ab484ba14dfd36c4af9aba798e460570904aa942e415ef2497bdf0a`,
selected through its exact Cargo JSON/frozen-artifact receipt. New Rust RPCs and
the amended binary migration CLI remain NOT_RUN. No build target was accessed.
Receipt: `/private/tmp/mcr-profile-self-8f89d190-20261104/schema-report-2.json`.

## Qualified storage prerequisite

Source checkpoints `30b9b71a716ab952c3edac394f613f50914facba`,
`0fdcc27fee15125d0d0e0b7a104e59030198dd49`, and
`c43345f5` add the dedicated draft wire, content/parser, native identity,
version/operation/receipt schema and storage primitives. These do not enable a
runtime SELF service. Only the exact granted one-line `sha2.workspace = true`
and matching Profile lock dependency reference were added; retained sha2 0.10.9
resolution/checksum stayed unchanged.

First compile refused the missing lock bookkeeping under `--locked`; its receipt
is preserved. After the precise shared-entry grant, both commands exited 0 using
Rust 1.88.0, locked/offline, one job, dev/test debug 0 and incremental disabled:

- `cargo +1.88.0 test --locked --offline -j1 -p profile-service --test concert_self_profile --no-run --message-format=json`
- `cargo +1.88.0 build --locked --offline -j1 -p profile-service --bin profile-service --message-format=json`

The frozen test binary passed six model/parser/fingerprint tests. The isolated
`concert_self_profile_native.py` controller passed three actual PostgreSQL 16.14
cases: legacy content writes advance the immutable epoch/revision and tombstones
refuse late writes; prepared writes remain invisible and exclude racing legacy
writes; prepared effects survive a controlled restart and fixture commit/abort
produce exactly the corresponding native status and committed receipt.
All eight migration checksums, empty-store refusal, migrate/verify/reapply and
post-restart verification passed. The marker/inode-owned fixture was removed.
These tests intentionally use synthetic local scope metadata to qualify SQL
mechanics. They do not claim actual Auth/Friend guard admission or an authoritative
cross-store decision. Those production RPC checks remain NOT_RUN.

Evidence: `/private/tmp/mcr-profile-self-8f89d190-20261104/` includes original
Cargo JSON/commands, private frozen binaries and receipts, source hashes,
`deterministic-1.log`, `native-report-1.json` and `target-release-2.json`.
MEDIA was released after executable/source freeze and before native execution,
under the explicit slice grant; PostgreSQL ran only from private binary copies.

The schema-only prerequisite was published independently to `master` as
`cc97f510703ca360b167ab448cf19a7848814dec`; fetched remote ancestry was verified.
It contains only 0008, the two owned native/schema controllers and their scope
report. Continuing RPC implementation is preserved on this development branch;
publishing the prerequisite did not complete BE-PROFILE-SELF.

## Current qualification inputs and remaining gates

The developer checkout has consumed the published SDK/key/policy checkpoint
`888cb6bd30846d891286b92a9a66e921e8d1ce78` and the published decision IDL
`43cb9e9f`. Public Profile's exact three-file registration patch `d88751fd` was
cherry-picked as `e2ca87e09924065a1adc3be9893c639cde5a6705`, without transferring
or manually editing its existing-file ownership. It adds the generated contracts,
optional dedicated SELF and settlement services, and supervised bounded recovery.
The public child protocol uses its own Complete method and does not register a
second ProducerSettlementService. Missing SELF keys preserve that separate path.

Read admission permits only exact C03 triples: getConcertParticipant/GET/me,
updateConcertParticipant/PATCH/me, and setConcertAvatar/PUT/me/avatar. Avatar
admission is read-only here; only update/PATCH/me can prepare Profile writes.
Both signatures and actual owner capabilities remain necessary. The new receipt
INSERT also checks the actual registered full XID, cluster/database, PID/start,
instance, command/effect digest and current Profile version on the same database
connection. A different connection must not forge the receipt. The expanded
native test covers that refusal; it is NOT_RUN pending a new allocated build.

Remaining BE-PROFILE-SELF gates belong to this developer: qualify the actual
Auth/Friend Check/Track and Concert decision/settlement RPC path with their real
owners. Owner16e1 supplies qualified Auth/Friend stores and artifacts; owner209
supplies the actual participant coordinator/decision endpoint. These are direct
implementation interfaces, not king/SL review gates. The original goal stays
active through that validation, documentation and non-force publication.

## Current compiled component

Source `5c921e28872ce3d24f7d930f63fdce9572b66ab3` compiled with Rust 1.88,
locked/offline, one job, debug 0 and incremental disabled. The final Profile test
target and production binary builds exited 0 with no warnings. Eight deterministic
tests passed. Three current native PostgreSQL 16.14 tests passed, including the
new durable read-reference rollback, a foreign connection refusing a receipt
even when its visible Profile version matches, actual prepared commit/abort,
legacy-write exclusion, restart persistence and cleanup. The current binary's
empty-store refusal, migrate, verify, reapply and post-restart verify all passed
against every exact source migration checksum.

The first attempt reused stale generated outputs. Only the Profile/contracts-build
package caches were refreshed (75 files, 72.1 MiB); other packages were retained.
Subsequent compiler findings fixed Prost's generated `profile.self_.v1.rs`
filename and signed-channel/interceptor composition. The wire package/service is
still `profile.self.v1`. Production registration remained the exact owner205
patch. Test-only isolated module embedding omits startup entry points by design.

Evidence: `/private/tmp/mcr-profile-self-906e69ab-20261104/` contains failed and
successful commands/Cargo JSON, 1490 compile-input hashes, source-bound frozen
binaries, eight-test log, `native-report-2.json` and `target-release-3.json`.
The allocated BACKEND target was released before native PG execution; no later
target access occurred. Frozen test SHA-256:
`86505e9073e14c08f65f95bf84f58158c692ebe03f62366996cea383a16992d7`;
production Profile SHA-256:
`e7483ca02811ee835a0c63cc83c2dca343a812f67dab937f5cc9951cc7004ec8`.

The new external actual-owner read/Track/abort controller and ignored native RPC
probe compile but remain NOT_RUN until exact Auth/Friend artifact prerequisites
arrive. The actual Concert decision endpoint is supplied separately by owner209.
ReadOperation/Reconcile may inspect only their exact existing persisted operation
under a fresh signed query; original binding expiry is not renewed into mutation
authority, and no fresh owner-view grant or caller-selected decision is invented.
