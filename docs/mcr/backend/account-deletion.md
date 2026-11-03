# BE42 — canonical Auth deletion and Concert cleanup

Parent BE42 status: **IN_PROGRESS**. The assigned canonical Auth orchestration,
typed owner adapters and native qualification are delivered and verified. Real
Concert owner composition and whole-task acceptance remain unproven.

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

The canonical eight-target contract is published in `d3b7c382`, with the
explicit decision in `docs/mcr/account-deletion-contract.md`. That exact commit
is imported into this isolated checkout with provenance; its generator and four
JSON bundles retain existing statuses/shapes. Main owns the separately assigned
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

Rust 1.88 compilation passes under the explicit BACKEND reservation. Frozen
Cargo-JSON-selected copies pass 15 Auth domain tests, 23 Auth application tests
and all 124 Auth unit tests (all exit 0). Python runner safety tests: 2/2 pass.
The new five adapter tests and owned native upgrade/retry/restart checks pass
against PG 16.14 with all 15 migration checksums, including unchanged 0014.
All five inherited durability tests pass after the narrowly granted fixture
correction in `389060b9`: the two-saga initial claim limit is now
`2 * UserDeletionTarget::ALL.len()` so both profiles are actually claimed. All
original atomicity, idempotency, operator-resolution, generation and lease
assertions remain. The first failed run is retained separately. Owned cleanup
passes after both runs.

The native runner accepts only a copied absolute regular executable outside
`target/` matching an explicit SHA256. It creates a fresh Unix-socket-only PG16
cluster with the existing marker/inode-owned `Cluster` helper, injects only its
generated URL and system identity, validates the exact test inventory and
migration checksum ledger, restarts its actual postmaster, checks persisted
failed/successful steps, then stops and removes only its owned root. It never
attaches a configured/default application database or starts Cargo.

Implemented executable checks cover five pure adapter tests plus native migration
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

Source checkpoint: repository safeguards `e040cb1a`, owned implementation and
qualification `e593ad79`. Main owns integration/push; no master push is performed
by this worker. Source-bound final native qualification ran on clean
`389060b90703be7bc1e8e9645c4b0bd926fb0ed9`; only this results document changes
after that run.

First native evidence: `/private/tmp/mcr-be42-qualified-8rznj9aq/` contains
`binaries.json`, `pure-regression.json`, and the preserved
`native-report-inherited-batch-failure.json`. The failed inherited fixture
claims only ten of sixteen steps across two canonical sagas; its later PROFILE
claim was missing. The corrected new source-bound frozen executable passes all
five inherited cases; no production assertion was relaxed.


## Committed handoff and reproducible evidence

The resume commit chain is `67f99768` (root exports), `a16802f5` (canonical
inventory/default fail-closed dispatch), `e040cb1a` (repository completion guards),
`e593ad79` (typed ports/dispatcher/migration/qualification), `57e23860` (exact
import of main's contract `d3b7c382`), `711d5215`, `2bedae5f`, `370076be`,
`c8c22ebf`, `562c3bb2`, `a616f540`, and `389060b9`. Main should integrate the
worker commits serially while omitting `57e23860` if its original contract commit
is already present. Review conflict handling against current root exports and
contracts; do not copy entire older files over newer owners.

Routine inherited test alignment is limited to two inventory counts, two
one-failed-target success counts and the corresponding eight-target test name in
`tests/unit/user_deletion_saga.rs`, plus the single two-saga claim limit in
`tests/user_deletion_saga_durable.rs`. These paths were explicitly granted.

All Cargo commands used Rust 1.88.0 with `--locked --offline -j 2`,
`CARGO_TARGET_DIR=/Users/lhs/Desktop/mcr-build/backend`, dev/test debug 0 and
incremental 0. No manifest, lockfile, dependency install or third target changed.

| Command / gate | Exit | Result |
|---|---:|---|
| `cargo +1.88.0 test --manifest-path backend/Cargo.toml --locked --offline -j 2 -p auth-service --test concert_deletion --test unit --test user_deletion_saga_durable --no-run --message-format=json` | 0 | Focused build; final log 2.80 s. |
| Same no-run command for `-p auth-domain -p auth-application --lib` | 0 | Domain/application test build, 1.18 s. |
| Same no-run command for `-p auth-service --test user_deletion_saga_durable` after fixture fix | 0 | Source-bound affected binary, 2.85 s. |
| Copied `auth_domain`, `auth_application`, `unit` with `--test-threads=2` | 0 each | 15 + 23 + 124 pass. |
| `account_deletion_smoke.py` with explicit copied test paths, SHA256s, PG16 path and report path | 0 | 5 adapter + native upgrade/retry case + postrestart case + 5 inherited native cases pass. |
| `python3 -m unittest discover -s backend/scripts/mcr -p test_account_deletion_smoke.py` | 0 | 2 runner safety tests pass. |
| `python3 scripts/mcr/check-plan.py` | 0 | Planning provenance and exporter checks pass; no application acceptance inferred. |
| Rust 1.88 `rustfmt --check`, staged whitespace and assigned-path audit | 0 | Exact scope includes only granted paths plus the imported contract patch; published Auth 0001–0014 and `docs/source/` unchanged. |

Evidence directory: `/private/tmp/mcr-be42-qualified-8rznj9aq/`.
`binaries-final.json` records exact Cargo-JSON-selected copied executable paths,
SHA256s and sizes. The other four binaries are unchanged from their earlier
successful build; only the inherited durability executable required rebuilding.
`pure-regression.json` and individual stdout/stderr files retain those results.
`native-report-final.json` is PASS at clean source `389060b9`, SHA256
`626f1e2b25c69d8ea1c56b227e98f39e8582028c7b6f1b560632e947caffd23a`.
The ledger contains exactly versions 1–15 with source-matching SHA384 receipts.

The actual owned postmaster changed from PID 70890 to 70925. Both are absent
and `/private/tmp/mcr-pg-imvmdhmz` is absent after cleanup.
`final-release.json` records unconditional BACKEND reservation release; no
further Cargo or PostgreSQL work runs after release. The MEDIA reservation
remains outside this worker's ownership.

Remaining integration belongs to the real Concert ticket/session/sharing/private
cleanup owners and main's production composition. The default production
branch deliberately retains required Concert steps as failed/retriable rather
than reporting cleanup success. FE36's separately owned eight-target consumer,
real reconnect/reveal denial, retained-field erasure, browser flow and deployment
must be qualified on a combined candidate before BE42/IN05 acceptance.

Auth0015 SHA256: `6b5ad5b41e145dab76b41569f225d0f38f3db304e9b9c8e7e649af87a465f751`.
