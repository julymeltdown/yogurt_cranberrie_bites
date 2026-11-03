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

Source authored for this task; no rendering or dependency upgrade. Validation,
registration, migrations and native multi-store checks: NOT_RUN.

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
