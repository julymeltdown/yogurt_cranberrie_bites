# BE04 Profile owner content read

Status: **IN_PROGRESS** for BE04; this bounded Profile application/repository increment has deterministic and real PostgreSQL evidence and awaits integration review. Authenticated Profile RPC, Gateway/Concert composition, multiuser participation, provider binding and broader BE04 acceptance remain **NOT_RUN** here. No task or IA entry is accepted by this document.

Base: `3fdb977d6a64798d76ea137099026879a68a67f9`; branch `codex/mcr-be-profile-owner`; isolated worktree `be-profile-owner`. Only the integrator publishes reviewed commits. Delivered ancestry is supplied in the handoff and external receipt rather than embedding this document's own commit hash.

## Provenance and dependencies

The four Rust additions are newly authored. They use the retained Profile foundation from `julymeltdown/port-rust`, source revision `704f42149d5e8e35e1d578676d8ae2dac4f0f9c2`, source prefix `boilerplate-grpc-rust/`, destination `backend/`, as recorded in `docs/mcr/backend/import-manifest.json`. No upstream object was independently fetched for this increment. No Bigyeting executable code was copied; `docs/source/` is unchanged.

The implementation reads the specification BE04 requirements, C03 content contract, and the existing `BasicProfile::new` checks in `backend/crates/concert-application/src/identity/mod.rs`. Profile owns current content and its local deletion lifecycle. Auth owns canonical identity/session state; this read does not attest account activation, suspension, provider ownership, event eligibility, roles or admission.

Only existing dependencies are used: `async-trait`, `chrono`, `uuid` and the standard library in application code; SQLx and the existing application crate in persistence. Tests use retained Tokio, SQLx and serde_json dependencies. No Cargo manifest, dependency resolution, lockfile, migration, proto, shared contract, service registration or router changes are included. The external `source-pins.json` pins the specification, assessment/execution documents, C03 inputs, consumer validation, manifests/lockfile, existing repository, all seven migrations and migration fixture helper to their inspected bytes at the base above.

Selected SHA-256 pins:

| Input | SHA-256 |
| --- | --- |
| Concert `src/identity/mod.rs` | `0d10b78c8308a5557dcec26f7ed3c93c8d22a8587aff1f159133ebb5375a5bd8` |
| `backend/Cargo.lock` | `6f4a6388b0aff3b1f835e6f7f0f545f4c9c99a1bc51857a86cbee3daea4a7cd7` |
| Profile `0007_user_deletion_fences.sql` | `050a2239f3efbf14016b36b41d0e50ed22ba408284165418d1a347059b3f2930` |

## Implemented contract

`ConcertProfileQuery` accepts only a nonnil UUID. String construction additionally requires exact canonical lowercase hyphenated representation. It selects a subject; it does not authenticate a caller. A future guarded self-only RPC must bind the authenticated subject and enforce its workload/actor/deadline policy before invoking this read. A caller can currently construct a query for any UUID in process; the unit is not an authorization endpoint.

`ReadConcertSelfProfile` dispatches to the owner port anew on every call. The result contains only canonical user UUID, current nickname as display name, optional bio and a database statement observation time. It has no broad Profile DTO conversion, serde implementation or legacy avatar fallback. Query, raw record and validated result redact Debug output. Errors contain fixed messages with no SQL, credentials, subject or private content.

The use case rejects a mismatched/nil returned subject, an observation outside calendar years 1–9999, blank display names, control characters, names over 40 Unicode scalar values and bios over 160 scalar values. These match the existing Concert consumer, including scalar rather than grapheme counting. It preserves spaces and exact content; only an exactly empty bio becomes `None`. It never trims, truncates or sanitizes invalid content into validity. The observation is neither a revision nor a TTL, clock-freshness guarantee, portable authority proof or authorization validity interval.

`PgConcertProfileRepository` owns an injected `PgPool` and issues one uncached parameterized SELECT with `$1` bound as UUID. Its projection is exactly `user_id`, `nickname`, `bio`, `statement_timestamp() AS observed_at`. The same statement requires:

- the selected `user_id` and `deleted_at IS NULL`;
- no row for that user in `profile_user_deletion_tombstones`;
- nickname at most 40 characters and 160 bytes, bio at most 160 characters and 640 bytes.

The byte bounds cap transferred content before SQLx decodes it; four-byte Unicode scalars at both legal maxima remain valid. Character bounds also reject excessive ASCII that fits within the byte cap. Invalid sizes produce no row, without substring operations or mutation. PostgreSQL may still inspect/decompress stored values to evaluate predicates; this is a service transfer/allocation bound, not a database CPU/IO bound. Application validation remains in place for malformed content and alternate/defective ports.

The actual owning fence table is `profile_user_deletion_tombstones` from migration `0007_user_deletion_fences.sql`. The retained triggers prevent active writes/reactivation after a committed tombstone while allowing soft-delete cleanup. The new read respects that tombstone even while the profile row still has `deleted_at = NULL`. Existing broad Profile queries were inspected but are unchanged; their synthetic `lifecycle_status = ACTIVE` is not reused as authority. Private self content remains readable by this internal producer; it is not public disclosure or a C03 PublicProfile, which separately requires authorized Concert avatar composition.

Absent rows, soft-deleted rows, committed fences and malformed content all map to `ContentUnavailable` with `Profile content is unavailable`. Query/connection/decode failures map to `DependencyUnavailable` with `Profile owner is unavailable`. Invalid selectors fail with `InvalidSubject`. No diagnostic variant discloses which unavailable-content condition occurred.

A statement snapshot gives coherent content and fence visibility. An uncommitted content edit remains invisible; its commit becomes visible on the next read. An uncommitted deletion fence does not retroactively invalidate an earlier snapshot; its commit excludes the next read. No read lock, global instant revocation, cache, portable proof or transaction spanning Auth and Profile is claimed.

## Exact assigned paths

The patch contains only:

1. `backend/crates/profile-application/src/concert_identity.rs`
2. `backend/crates/profile-application/tests/concert_identity.rs`
3. `backend/services/profile-service/src/adapters/out/persistence/concert_identity.rs`
4. `backend/services/profile-service/tests/concert_identity_owner.rs`
5. This document, `docs/mcr/backend/profile-identity-owner.md`
6. `backend/crates/profile-application/src/lib.rs`: append only `pub mod concert_identity;`.
7. `backend/services/profile-service/src/adapters/out/persistence/mod.rs`: append only the same module export.

Application imports remain free of transport, persistence and frontend dependencies. The persistence adapter depends only on its owner application contract and SQLx. No other module or service is wired by this patch.

## Validation and evidence

External evidence root: `/Users/lhs/Desktop/mcr-build/profile-identity-owner/`. Exclusive retained Rust target: `/Users/lhs/Desktop/mcr-build/backend`. Final builds use Rust `+1.88.0`, jobs 2, both `CARGO_PROFILE_DEV_DEBUG=0` and `CARGO_PROFILE_TEST_DEBUG=0`, `--locked --offline`. Artifact metadata verifies test debug information is zero. Earlier runs that only set the dev setting retained test debug information; their evidence is preserved separately and does not establish the final build configuration.

All listed commands exited 0. `debug0-checks.json` and `bounded-build.json` contain exact argv; logs retain the test output:

| Command (from worktree root) | Evidence and result |
| --- | --- |
| `cargo +1.88.0 test --manifest-path backend/Cargo.toml --locked --offline -p profile-application --lib --test concert_identity` | `application-debug0.log`: 6 existing library tests and 10 new application tests passed |
| `cargo +1.88.0 test --manifest-path backend/Cargo.toml --locked --offline -p profile-application --doc` | `doc-debug0.log`: 1 compile-fail nonserialization test passed |
| `cargo +1.88.0 test --manifest-path backend/Cargo.toml --locked --offline -p profile-service --test architecture` | `architecture-debug0.log`: 1 existing layering test passed |
| `cargo +1.88.0 test --manifest-path backend/Cargo.toml --locked --offline -p profile-service --test concert_identity_owner --no-run --message-format=json` | `build-bounded.jsonl`, `build-bounded-stderr.log`: corrected repository/test binary compiled, debug level zero |
| `python3 /Users/lhs/Desktop/mcr-build/profile-identity-owner/verify_owner.py` | `native.json`, `owner-pg.log`, `owner-outage.log`, `owner-recovered.log`: 7 distinct real-PG tests plus 1 repeated recovery invocation passed |
| `rustfmt +1.88.0 --edition 2021 --check` on the four new Rust files | `final-format.log`: passed |
| `python3 scripts/mcr/check-plan.py` | `final-plan.log`: planning consistency only; passed |
| `git diff --check` and exact path/export/import audit | `final-diff.log`, `scope-audit.json`: passed |

Total: **25 distinct tests**, with one additional repeated successful recovery invocation. Application/architecture checks above precede the repository-only size correction; the corrected service target was rebuilt and its full PostgreSQL suite rerun. No application code changed after those checks.

The native harness reuses `backend/scripts/mcr/migration_smoke.py`'s owned-cluster helper at its pinned source hash. It creates a fresh marker/token/device/inode-owned temporary cluster with Homebrew PostgreSQL **16.14**, UTF-8, private Unix socket, TCP disabled, scram credentials and the isolated database `mcr_profile_identity_owner`. It applies all seven actual Profile SQL migration files in order, recording hashes and exit codes. This direct SQL application is not SQLx migration-ledger/startup qualification. No Docker, default database, imported service startup or live-data migration is used.

The read role has only column SELECT on `profiles(user_id,nickname,bio,deleted_at)` and `profile_user_deletion_tombstones(user_id)` and uses read-only transactions. Successful minimal reads and explicit denied private-column/write queries prove the projection boundary. Fixtures include private handles, legacy URL tokens, counts and deletion reasons; output is scanned for canaries and connection/password leakage. The role setup and credentials exist only in the owned fixture, not production composition.

Real-PG coverage includes private active self, foreign-subject selection without cross-row mixing, absent/soft-deleted/committed-fence equivalence, stale-write fence rejection and allowed deletion cleanup, uncached updates, malformed blank/control/oversized values, exact maximum Unicode content, transaction snapshot/commit visibility, real permission failure, a closed pool, stopped-server failure and recovery. The size regression seeds 2 MiB nickname and bio values separately, asserts **at the repository boundary** that no record is returned, then verifies stored byte lengths are unchanged. It also rejects 41/161-character ASCII/four-byte inputs and returns exact 40/160-character four-byte maxima. Application doubles additionally cover misbound/nil returned subjects, malformed observations and safe dependency failure with no cached fallback.

Final native test binary SHA-256: `a8b582bc6f6a57f85932e5180a92557860b588b39b53c831e4d66fac32e77eec`. The harness stopped its exact owned server, removed its exact owned root and recorded cleanup success. The external handoff receipt pins final source, controller, binary, logs and ancestry; it contains no database credentials.

## Corrected verification failures and remaining limits

Preserved evidence records an interrupted controller followed by identity-checked cleanup of its owned cluster (`interruption-recovery.json`), an initial socket/host preflight mistake (`native-failed-fixture-host.json`), and an unsuccessful MetadataExt build followed by a stale-binary preflight run (`service-build-missing-import.jsonl`, `native-failed-stale-binary.json`). Those are failures, not passes; the corrected final build was selected before rerunning the fixture. `before-bounded-read-*` preserves the prior evidence, and the final bounded-read logs/report supersede it for the corrected adapter. No cleanup targeted another service or a global process list.

The local persistent BE04 goal was previously marked BLOCKED for shared dependencies. The integrator explicitly resumed compatible useful work; the available goal API has no resume-to-active operation. Its actual blocked status was reported, not rewritten as active or falsely completed. This content increment does not complete the broader BE04 goal. Concrete integration-review fixes remain part of handoff completion.

Authenticated caller policy, transport serialization/envelope, RPC registration, Gateway/Concert clients, owner deadlines, service credentials, Auth/session and event restrictions, provider binding, avatar authorization, browser flows, load and release acceptance remain outside these seven paths. No production route gains content access from exporting these modules. The integrator owns those separate prerequisites and the final combined checks.
