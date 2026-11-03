# BE42 — canonical Auth deletion and Concert cleanup

Status: **IN_PROGRESS**. Source work extends the inherited durable Auth saga;
real Concert owner integration and whole-task acceptance remain unproven.

## Source and ownership

Current worktree: `/Users/lhs/Desktop/mcr-worktrees/be-deletion-resume`, branch
`codex/mcr-be-deletion-resume`, clean resume base
`ef9422c4d83f8555ee5b48ef693d37fd5bf87e0c`. Original authoring base was
`edc460943496011873f90f9d60a5931a9570042a` in the stopped
`be-account-deletion` worktree. The original source commits `3243e9d8` and
`4524aee4` were recovered with `cherry-pick -x` as `67f99768` and `a16802f5`.
All ten saved files were verified against the SHA256/byte receipts in
`/private/tmp/mcr-be42-recovery-1793742216341071000/receipt.json` before restoration;
current shared-root exports were preserved. Required behavior comes from
`MCR_IMPLEMENTATION_PLAN_V2_EN.md` BE42 and §§7.2, 8.3, 9; `docs/source/`
is unchanged. The retained port-rust source revision is
`704f42149d5e8e35e1d578676d8ae2dac4f0f9c2`, recorded by
`docs/mcr/backend/import-manifest.json`. Existing inherited source and its imported
history are reused in place; no Bigyeting rendering, second Auth identity, separate deletion database,
or replacement Concert command receipt is introduced.

The reused canonical sources at this base are:

- `backend/crates/auth-domain/src/user_deletion.rs`: target and saga/step states.
- `backend/crates/auth-application/src/dto/deletion_dto.rs` and
  `src/use_cases/delete_user.rs`: internal transfer and deletion request behavior.
- `backend/services/auth-service/src/adapters/out/persistence/user_deletion_saga_repository.rs`:
  transactional account disable, refresh revocation, saga/step persistence,
  leases, retry generations and immutable operator retry resolutions.
- `backend/services/auth-service/src/adapters/in/messaging/user_deletion_worker.rs`:
  bounded batches, exponential retries, dead letters and restart recovery.
- Auth migrations 0005, 0009–0011: existing saga, operator recovery and trace
  context. Published migrations remain byte for byte unchanged.

New `auth-application/src/concert_deletion/` owns framework-free typed commands,
operation identities, completion acknowledgements and four independent owner
ports. New `auth-service/src/concert_deletion/` owns the bounded composite
dispatcher, including explicit timeout uncertainty with a stable owner operation key. Dependencies are retained workspace `async-trait`, `chrono`,
`uuid`, `thiserror`, and service-only `tokio`; no manifest/lockfile change.
Root exports are isolated in commit `3243e9d`. Delivery explicitly granted the
canonical target/DTO and inherited dispatcher edits, isolated in `4524aee`.
The resume assignment explicitly grants the narrow repository changes that
union canonical targets into `begin` and require all eight successful timestamped
steps before completion. Empty/four-target legacy callers are normalized and
duplicate targets deduplicate under the existing primary key. A malformed saga
with missing required steps or completion timestamps cannot complete.

## Durable behavior and owner obligations

Every canonical deletion requires eight targets: `PROFILE`, `FRIEND`, `POST`,
`NOTIFICATION`, `CONCERT_TICKETS`, `CONCERT_SESSIONS`, `CONCERT_SHARING`, and
`CONCERT_PRIVATE_DATA`. Existing HTTP shapes and status vocabularies remain:
`DELETE /api/auth/account` returns 202 with `userId/deleted/sagaId/saga`;
`GET /api/auth/account/deletion` returns the canonical saga. There is no declared
DELETE request body or idempotency header. `deleted=true` means canonical
account disable was accepted, not that the saga completed.

The inherited transaction disables the canonical user and revokes refresh
tokens before success is returned. The inherited unique user/saga and
`(saga_id,target)` keys deduplicate repeated requests. Concert commands preserve
the same `(saga_id,user_id,target)` identity across attempts, expired leases and
operator retry generations; actor and retention deadline also remain bound.
Each owner must atomically persist its operation receipt and effects, retaining
a permanent deleted-user fence. A queued job or transient read is insufficient
to acknowledge completion. A response must match the complete command and have
a nonzero committed fence revision; transport adapters must authenticate its
origin. These typed assertions do not manufacture a real owner or prove its
database/authorization semantics.

Required owner effects cover every event for the canonical user:

| Target | Completion means |
|---|---|
| Tickets | Revoke outstanding tickets and deny future issuance and consumption. |
| Sessions | Remove active sockets, presence, subscriptions and private caches; deny reconnect. |
| Sharing | Revoke requests/grants in both directions, remove encrypted handles and caches; deny reveal and recreation. |
| Private data | Scrub retained personal fields across Concert features using reviewed retention rules; preserve necessary aggregate, stock, command-receipt and audit integrity. |

Owners are independent, so BE41 readiness does not require replacing these
ports with a successful stub. Any missing owner, timeout, incomplete/mismatched
acknowledgement or typed owner failure becomes fixed `Dependency unavailable`
in the canonical retry state. Calls have an explicit positive deadline no
longer than five seconds. Timeout is an uncertain result; retry keeps the same
operation identity and requires owner deduplication. No private identifier or
downstream diagnostic is included in new command `Debug` output or errors.

The inherited dispatcher explicitly rejects Concert targets until a production
composition injects real owners. This patch does not register an invented HTTP
or gRPC cleanup endpoint or silently skip an unavailable owner. Reconnect and
reveal denial against actual services remains a required integration gate.

## Migration and forward recovery

Reserved Auth `0015_concert_deletion_steps.sql` extends the existing step CHECK
and inserts missing Concert steps for existing sagas. Previously completed
four-step sagas reopen as `IN_PROGRESS` with no completion time. Successful
legacy steps and retention deadlines remain; existing `DEAD_LETTERED` sagas keep
their operator-required state. Existing worker retry generation and stale-result
fences continue to apply. SQLx receipts prevent normal reapplication.

Stop older Auth writers and workers for this upgrade. Older binaries cannot
parse the new required targets and must never create new four-step sagas after
cutover. Deploy the coordinated target-contract/consumer revision together.
Published Auth `0014_concert_relationship_revision.sql` is present at the resume
base and remains unchanged. The native upgrade explicitly requires its
successful ledger receipt before applying 0015. This is an Auth migration; it
does not allocate or alter a Concert-service migration.

Migration failure must roll back target-check changes, backfill and migration
receipt together. Correct the failing environmental condition and rerun the
unchanged migration. Do not edit a published migration/checksum or roll back to
an older four-target binary. After application, recover individual downstream
failures through the canonical operator retry command and its immutable
resolution receipt. Missing-owner failures remain visible through restart;
never purge a non-completed saga as successful cleanup.

## Contract and frontend coordination

The Delivery-owned input is
`backend/scripts/docs/openapi/schemas.py`, `UserDeletionStep.target.enum`.
Generated core/domain bundles and fixtures must append the four exact values.
`backend/proto/auth/v1/auth.proto` carries `UserDeletionStep.target` as string;
Auth gRPC and Gateway mappings already pass it through unchanged. No protobuf
field numbers, statuses or HTTP shapes change.

The main chat recorded the eight-target contract decision in the resume
assignment and owns generator/artifact publication plus the separately assigned
FE36 decoder adaptation. The original FE36 four-target decoder correctly rejects
unknown/missing/duplicate steps; it must adopt the published revision/hash before
producer wire acceptance. No heuristic unknown-target
acceptance or four-of-eight completion is permitted. Preserve the original
canonical privacy scope (actor/event/generation/policy), issue DELETE once, and
recover uncertain results only through manually requested same-identity GET
under the isolated suspended deletion scope. Keep private owners cleared while
pending. Complete only when the saga is `COMPLETED` and all eight required
steps have successful states and completion timestamps; then canonical logout
clears local identity. Previously seen/copied handles cannot be erased from
another person's memory.

## Validation record

Before Cargo lease: Rust 1.88 formatting passes; the Python runner's two safety
tests pass (exit 0). Cargo compilation, pure Rust tests and runner-owned native
PostgreSQL tests are **NOT_RUN** until the shared target lease is granted.

The native runner accepts only a copied absolute regular executable outside
`target/` matching an explicit SHA256. It creates a fresh Unix-socket-only PG16
cluster with the existing marker/inode-owned `Cluster` helper, injects only its
generated URL and system identity, validates the exact test inventory and
migration checksum ledger, restarts its actual postmaster, checks persisted
failed/successful steps, then stops and removes only its owned root. It never
attaches a configured/default application database or starts Cargo.

Planned executable checks cover five pure adapter tests plus native migration
failure/forward recovery, legacy completed/pending/dead-letter upgrades, repeated
deletion/refresh revocation, seven-success/one-failure visibility, dead letter,
deduplicated operator retry, same-attempt cross-generation stale-result fencing,
expired lease recovery, malformed four-target/missing-timestamp noncompletion,
all-eight completion, absent-owner persistence, checksum-mismatch refusal and
actual PostgreSQL restart.
Typed deterministic owner fixtures exercise orchestration, not actual Concert
cleanup, socket shutdown or reveal authorization. HTTP/browser/two-account
flows, BE41 owners, deployment, retention-policy approval and IN05 remain
**NOT_RUN**. No full BE42 acceptance is claimed.
