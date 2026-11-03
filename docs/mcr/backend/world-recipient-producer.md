# World recipient read producer

Status: `CONTRACT_READY` for the committed, natively qualified bounded recipient-read producer. This is a BE13/BE14 prerequisite, not acceptance of the complete tasks. Runtime composition, publication fencing, live multiuser IN01 and IN05 remain `NOT_RUN` here.

## Ownership and provenance

- Assigned checkout: `be-world-recipient-producer` (isolated assigned worktree).
- Assigned branch: `codex/mcr-be-world-recipient-producer`.
- Base: `1b7caeccdd10856b3c5ebaa0e9b0286ef8a2ced0`.
- New producer: `backend/services/concert-service/src/world/recipient_owners.rs` and `recipient_owners/local.rs`.
- New native evidence source: `backend/services/concert-service/tests/world_recipient_owners/mod.rs` and `native_smoke.py`.
- Registration-only grant: one module export in `backend/services/concert-service/src/world/mod.rs`.
- No manifest, lockfile, proto, runtime, infrastructure, frontend, or original source archive changes.

The producer is newly authored. It calls the existing signed `GrpcRelationshipAuthority` and `relationship::lock_state` local lock protocol at the base revision. It does not copy Bigyeting rendering or query Auth/Friend databases directly. Native fixture support is adapted from `tests/relationship_authority.rs`, `src/adapters/out/relationship_authority/world_auth/tests.rs`, and `tests/world_auth_read/native_smoke.py` at that revision. It retains the marker/inode/private-directory/socket/database checks and guarded owning-service reads. Dependencies are the retained workspace versions of sqlx, tokio, tonic, chrono, uuid, platform, concert-application/domain, and the existing Auth/Friend test service dependencies; no dependency changes are required.

The separately owned sealed-context source dependency is `353e7f8ebe96bac3c90134e6ca60e627589decb5`, cherry-picked with explicit Main authorization as `4a73d53`. Its qualification is separately owned; its source commit alone is not publication acceptance. The declared interface is `relationship_authority::world_projection::WorldPresenceProjection`, constructed through a `WorldProjectionParent` retained from a checked `WorldOwnerRequest` and `WorldAuthObservation`. The producer is type-checked and natively qualified against that committed dependency. No uncommitted other checkout was used.

## Behavior

`WorldRecipientReader::new(PgPool, GrpcRelationshipAuthority)` has no permissive authority fallback. Its asynchronous `read(&WorldPresenceProjection)` accepts one server-scoped candidate. A caller cannot construct an observation from browser identity or a raw `WorldSubject`.

Before any I/O, the reader validates the sealed context, converts it to the exact Presence `RequestBinding`, and fallibly reserves both Auth and Friend attempts. Nested attempt guards and an outer timeout retain the original monotonic stop, immutable credential/source expiry, and shared cancellation. Same-target derivation cannot mint new attempts; distinct room targets remain bounded by the parent context's fanout and original stop. There are no retries or newly minted dependency budgets.

Remote I/O completes before local database locks. Local reads reuse the event/pair/participant locking protocol and independently check:

- Presence-only actor/event/subject scope; same room, process owner/epoch and complete resource pin.
- Event phase, participation, active actor admission, subject suspension and both event-local block directions.
- Current exact session, connection, membership owner/epoch, live owner lease, membership and immutable access expiry, disconnect state, room availability, and current room map/avatar pins.

Missing authority, expired leases, owner drift, malformed state, pool/SQL failures and elapsed operation stops fail closed. A disallowed actor fails the read. A blocked, deleted, suspended or otherwise disallowed subject produces `visible=false` with independent observed revisions where the local rows still exist. No identity/profile/private handle data is returned by this producer.

Local SQL reserves a data transaction and a cancellation-control transaction before taking policy locks. An owned per-reader semaphore permit travels with the SQL task through both rollbacks, capping these two-connection operations at half the pool maximum; other pool users can still cause fail-closed acquisition expiry. An owning task retains the data connection until cancellation and rollback drain. Dropping the outer request signals that task, which cancels the exact captured PostgreSQL backend through the reserved control connection and rolls back before recycling it. Transaction-local statement and lock limits are clipped to the remaining original budget with a 100ms safety ceiling. The control query has a 250ms cleanup limit and grants no new read or publication authority. These limits disappear at rollback and do not alter serving defaults. Reads slower than the conservative SQL ceiling fail closed.

The local transaction is explicitly rolled back. The shared lock helper can provision a neutral pair when none exists; this provisional row is never committed by the reader. Its initial local revision describes the observed neutral state and is not a durable mutation receipt. No fabricated `FreshAuthority`, `PolicyDecision`, scalar stamp or `FencedRelationshipAuthority` is introduced.

The observation contains the independent Auth and Friend `(epoch, revision)` values plus separate local event, pair, participant and restriction revisions. Its expiry is the minimum of owner observation expiry, actual/captured local lease bounds, original source/credential expiry and original operation deadline. Debug output is redacted.

`check_scope(context)` checks the exact captured request/family, actor and candidate membership snapshots, cancellation and expiry. It does **not** re-observe revocation. Neither comparing a vector with itself nor passing this check proves current distributed authorization.

## Runtime handoff boundary

The runtime owner must retain the original operation context. Initial ticket-backed parents are for self Profile only. Presence requires the authoritative post-admission current membership and checked Auth observation under the same original operation deadline; no reconsume, ticket promotion, raw-subject parent construction or fresh timeout is allowed.

This patch intentionally does not implement the existing `LiveWorldOwners::visible`, whose current inputs lack that sealed parent. Runtime publication/invalidation remains separately owned: issue bounded candidate reads outside the publication fence, compare/install exact owner-scoped results, invalidate on known Auth/Friend/local/connection/owner changes, and deny expired/unavailable views. A read completed before a subsequent remote revocation is an observation, not a distributed fence over later frame enqueue/publication. No real listener, product navigation or two-user flow is certified by this patch.

## Validation and delivered ancestry

Original base: `1b7caeccdd10856b3c5ebaa0e9b0286ef8a2ced0`. Dependency `353e7f8` was cherry-picked as `4a73d53`; the canonical context owner subsequently qualified its complete source separately. Integrate only these recipient commits after the canonical context dependency:

1. `8d95db94b77711312c80afd56a546d63435833c7`: reader, local policy, actual owner native fixture, runner and initial evidence document.
2. `8f645fb0c694ce51da2960978598ffffdf0f4d1e`: cross-target observation rejection and accessible local revision type.
3. `d6c6dbb30a9b99e9868dfa100b21a0fe01b3f1ea`: explicitly drain SQLx cancellation responses before rollback; heap-pin native scenario.
4. `ebd223153c026ded39f70d60f445728e99cdbfa2`: heap-pin every typed read inside the long native fixture so the default test stack suffices.
5. This documentation-only qualification handoff follows those source commits.

Only the six assigned paths listed above differ from the dependency parent. The world module change is one export. No dependency-owned source was edited.

| Check | Exit / result | Boundary |
| --- | --- | --- |
| Rust 1.88.0 source formatting and staged `git diff --check` | 0 | Owned source and whitespace. |
| Python runner compilation and `--help` | 0 | Runner syntax/imports. |
| `python3 scripts/mcr/check-plan.py` | 0 | Planning provenance/export checks; 13 original archive checksums preserved. |
| `cargo +1.88.0 test --locked --offline -p concert-service --lib --no-run --message-format=json` | 0 | Final source `ebd2231`, 10.84 seconds after scoped cache repair. |
| Auth/Friend/Concert migration binary build, locked/offline | 0 | Frozen source `8f645fb`, 6.31 seconds. Only recipient local read/test files changed afterward; migration sources and binaries remained identical. |
| Frozen library Auth tests, excluding native modules | 0; 8 passed, none skipped | Actual final frozen executable. |
| Frozen library sealed projection tests, excluding the database test | 0; 5 passed, none skipped | Dependency revision present in this branch; do not substitute the other owner's larger suite count. |
| Native recipient scenario with default test-thread stack | 0; 1 passed, none skipped | Real guarded Auth/Friend services and three private PostgreSQL 16.14 stores. |
| Migration, verification and reapply | 0 for each owner | Auth 14, Friend 15, Concert 13 migrations; exact ledgers checked. |
| PostgreSQL stop/start, persisted owner/local state and migration re-verification | PASS | All three stores; fresh postmaster required. |
| Artifact/source hashes after native execution | PASS | Exact frozen executable and source binding. |
| Owned cluster cleanup and process/root verification | PASS | Fixture root absent; zero processes referring to it. |

Build settings were `CARGO_BUILD_JOBS=2`, `CARGO_PROFILE_DEV_DEBUG=0`, `CARGO_PROFILE_TEST_DEBUG=0`, `CARGO_INCREMENTAL=0`, with an 8 GiB free-space floor. Only Main's explicitly allocated existing media/backend targets were used. Cross-worktree stale protoc paths, generated output and local Auth/Friend/Platform metadata caused recorded build failures (101). Package-scoped cache invalidation corrected them; no dependency installation, third target or source workaround was used. Each lease was explicitly released after executable copying/hashing, with zero open target files and no associated Cargo/rustc, before private PostgreSQL execution.

The native test uses actual server-issued/consumed tickets, current membership repository reads, checked signed Auth parents, signed dual-owner Presence reads, and local policy rows. It verifies positive reads; independent Auth/Friend revision changes; repeated-attempt rejection and distinct-target fanout under one parent; target-scope mismatch; no committed neutral pair; inherited reverse and local blocks; local suspension; Auth subject restriction; disconnect, resource-pin and owner-epoch drift; pre-read cancellation; original deadline expiry; and actor family revocation.

For in-flight cancellation, the test first observes the exact backend waiting on an event-row lock. It then cancels the sealed context, requires an explicit PostgreSQL cancel acknowledgement and both successful rollbacks for that same backend, and verifies the backend no longer waits while the blocker remains held. SQL timeout alone cannot satisfy this proof. The owning task retains its semaphore permit through cleanup.

## Native receipts and failure history

Private local evidence root: `/private/tmp/mcr-world-recipient-8f645fb-20261104`.

- Final native report: `ebd2231/native-report.json`, SHA-256 `0ad1f4dd5d74e61aea02d3c547dbf6c82a5d466a21d57c12ee03e5a05666c472`.
- Final freeze receipt: `ebd2231/freeze.json`, SHA-256 `d0037d826d301a5bab937538984b66d116de66a5654c2de559f0cb8d771646a3`.
- Final library test: SHA-256 `d6e7f1746b3d03fdb8eb5f6acf27bfeb9abbff9e8e8251b63b65582e1654a786`, source `ebd2231`.
- Auth migration binary: SHA-256 `e0f5b8efb827c221f5c410c2776c09dd8a00e111caa94bed83a2c4b5a32d84e6`.
- Friend migration binary: SHA-256 `46986a249eefaa5ab43cb75780872fca1be2851ee32ed78b2997c94cc9dc39d8`.
- Concert migration binary: SHA-256 `a738c3ec6f1d7bdaa2e928bd2bbcd3506eac93072c052b6a03cbd810bbf17173`.
- All three migration binaries retain source `8f645fb`; both later source changes affect only recipient read cleanup and its fixture. Their hashes were rechecked at every freeze/run.

The private `run_frozen.py` driver calls the owned runner's artifact/fixture functions after verifying source and copied artifact hashes, preserving release-before-PostgreSQL ordering. Cargo JSON receipts remain unchanged. The owned runner also supports a complete standalone invocation:

```text
python3 -B backend/services/concert-service/tests/world_recipient_owners/native_smoke.py --cargo-json <absolute successful receipt> --pg-bin /opt/homebrew/opt/postgresql@16/bin --artifact-dir <new private directory> --report <new absolute report>
```

Its standalone mode pins executables immediately before starting PostgreSQL; a shared-target operator must instead freeze first, explicitly release the lease, then invoke `exercise` with those verified copies as the recorded private driver does.

Failures are retained, not relabeled as passes. The initial default-stack run overflowed the aggregate native test stack. Its 16 MiB diagnostic rerun reached and failed the strict cancellation/rollback check: SQLx surfaced the pending cancellation error before ReadyForQuery, so rollback could return early. Commit `d6c6dbb` fixes this by draining the expected PostgreSQL cancellation/lock-timeout response before rollback. That source passed the complete scenario with the diagnostic stack, but its default-stack run still overflowed. Commit `ebd2231` heap-pins the individual test read futures; the final run passes with **no `RUST_MIN_STACK` override**. Every failed/diagnostic cluster was cleaned up. The diagnostic pass is not substituted for the final default-stack proof.

The conservative 100ms local SQL safety ceiling and two-connection cancellation reservation have not been load-qualified. No product World listener, real socket publication race, browser flow, complete multiuser session recovery or IN01 is certified here.

## Persistent goal

The existing BE13/BE14 persistent goal remains unfinished. At inspection its tool status was historical `blocked`; the exposed goal tool cannot resume it. This continuation is working on the authorized new producer without falsely completing or replacing that goal. The earlier recovery-fixture correction also has no fresh native rerun in this checkout. No goal completion, real IN01 acceptance or release readiness is claimed.
