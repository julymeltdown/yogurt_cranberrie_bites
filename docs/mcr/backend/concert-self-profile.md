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

Source authored for this task; no rendering or dependency upgrade. Production registration compiles and actual owner read RPC checks pass below.
The native multi-store Prepare/decision flow remains NOT_RUN.

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

The new RPC modules compile; actual Auth/Friend read/Track/abort RPC
qualification passes below. Concert Prepare/decision qualification is NOT_RUN. Amended 0008 has SQL and current-binary qualification below. Neither
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
native test covers that refusal and passed in the current qualification below.

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

The external actual-owner read/Track/abort probe passed as recorded below.
The actual Concert decision endpoint is supplied separately by owner209.
ReadOperation/Reconcile may inspect only their exact existing persisted operation
under a fresh signed query; original binding expiry is not renewed into mutation
authority, and no fresh owner-view grant or caller-selected decision is invented.

## Actual owner read RPC qualification

`rpc-report-2.json` is PASS using the current frozen Profile binaries and actual
Auth/Friend production binaries from owner publication
`dc70d62049d8e8430c88b9ebfc6f071740020538` (qualified source `d4c604f0`).
The owner metadata and exact source/binary hashes were checked before launch.
All three production mains used a new isolated PostgreSQL 16.14 fixture, fresh
private mTLS identities, explicit endpoints and separate standard-Base64 signing
keys. All three migrate/verify pairs exited 0. The exact external read test
exited 0 with one passing test: actual signed owner Acquire, both owner Check/Track,
Profile protected read, persisted native read identity, and actual Auth workload
querying Profile settlement with native ABORTED proof. Wrong subject and wrong
Friend credential were refused without another read reference. Exactly one read
reference and no write receipt remained.

The unavailable private Concert root endpoint was intentional for this bounded
read component. After original expiry, both actual owner guards remained prepared;
expiry did not fabricate root completion or grant release. Root scope release,
actual Concert decision and Gateway HTTP remain NOT_RUN. All three service mains
shut down normally with exit 0, no forced stops; owned PG cleanup passed.
The first runner failed before service launch because system LibreSSL could not
derive an Ed25519 public key. The controller now uses installed OpenSSL 3 for that
fixture step, with no package install. Both attempts and cleanup are preserved.

Evidence: `/private/tmp/mcr-profile-self-906e69ab-20261104/rpc-report-2.json`,
controller SHA-256 `9ce31ae9c2fcbad5298013af9a50a5a1243c2b820c2b8715d78da13c81954a4b`,
owner metadata SHA-256 `dadd2b3ddab4c19e2eb165def6c2e8e8d9cc079ee3c8fa14b7bdc3aa9beb4b8b`.
No mutable Cargo target was used for this RPC run. The full assigned goal remains
active through actual Prepare/Concert decision/reconcile validation and delivery.

## Published SELF contract prerequisite

The dedicated `backend/proto/profile/v1/concert_self_profile.proto` is published
independently to unblock actual Concert client generation. The exact contract
compiled in Profile source `5c921e28872ce3d24f7d930f63fdce9572b66ab3`, with the
production main and test target exiting 0 under Rust 1.88 locked/offline.
Its guarded Read endpoint also passed the actual Auth/Friend/Profile production
mTLS read/Track/native-abort probe. Receipt:
`/private/tmp/mcr-profile-self-906e69ab-20261104/rpc-report-2.json`.
The protobuf package is `profile.self.v1`; Prost generates `profile.self_.v1.rs`
because `self` is a Rust keyword.

This two-path prerequisite contains the IDL and this report only. It does not
enable the SELF runtime, publish the continuing implementation, or certify
Prepare/Concert decision/reconcile and HTTP delivery. Those remain IN_PROGRESS
in the assigned developer chat. The earlier schema prerequisite is `cc97f510`.

## Qualified runtime source publication boundary

The continuing guarded producer is published as a qualified component to unblock
the participant developer's actual joined harness. Status remains IN_PROGRESS;
Prepare against real owners, actual Concert Journal commit/abort reconciliation,
and Gateway HTTP delivery are still open. ParticipantLive owner209 owns the joined
driver using its actual ParticipantProfileJournal, BoundParticipantAuthority and
typed Profile client. A decision-reader fixture alone cannot accept that flow.

Publication base is fetched master `b222805c`; the preserved schema prerequisite
is `cc97f510`. The new delta owns the `src/concert_self_profile/` tree, its exact
test entry, native/RPC controllers and this report. The only shared edits are the
explicitly granted Profile `sha2` manifest/lock entry and the unchanged owner205
three-file registration handoff `d88751fd` (local cherry-pick `e2ca87e0`). The new
IDL was published independently in `b222805c`. No application implementation was
imported across services or from the frontend, and no source archive was changed.
The exact changed-path list and compiled input comparison are retained in
`/private/tmp/mcr-profile-self-906e69ab-20261104/prepublish-1.json`.

After incorporating peer history, every Profile source/manifest/lock input and
its generated Profile/authority/participant contract bytes still matched the
qualified build. The shared platform delta changes two test fixture operation
labels only; the unrelated product-delivery IDL is not generated by Profile.
No mutable target was used or claimed for another build. The planning check exited
0 with original archive checksums intact; its first sparse-checkout invocation
refused missing planning files, then passed after materializing those tracked
inputs. The planning result certifies no application runtime gate.
