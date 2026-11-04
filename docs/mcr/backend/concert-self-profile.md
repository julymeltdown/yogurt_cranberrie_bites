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

These new RPC modules and the added read/recovery tables in unpublished 0008 are
source-only, NOT_RUN. The historical storage qualification below precedes these
changes and cannot certify their amended schema or runtime behavior.

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
