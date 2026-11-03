# BE04 guarded Profile self RPC

Status: **IN_PROGRESS** for BE04. This bounded internal self-RPC implementation passes the signed transport, native PostgreSQL and retained checks below; committed handoff and integration review are separate obligations. Broader BE04 identity/admission, Gateway consumption and product acceptance remain open.

Assignment base: `d63fb2b95d3d8e61406ef71a12ed86a3b0bf15fd`; branch `codex/mcr-be-profile-self-rpc`; isolated worktree `be-profile-self-rpc`. The former owner worktree remains at its clean `81665a04b579d8709fa6cbe018c46b01db4edce0` handoff. Only the integrator publishes reviewed commits; there is no worker push.

## Source, contract and scope

This is newly authored code over the retained `julymeltdown/port-rust` foundation revision `704f42149d5e8e35e1d578676d8ae2dac4f0f9c2`, original prefix `boilerplate-grpc-rust/`, destination `backend/`, recorded by the existing import manifest. The upstream object was not independently fetched. The owner application/repository bytes from `81665a04` are retained at this base through integrator commit `b3262e2`. No Bigyeting executable code, legacy art or broad Profile DTO is copied.

`docs/mcr/backend/profile-rpc-prerequisite.md` freezes the wire, policy, AppState injection and server delegation at the assignment base. Read-only implementation inputs include the generated Profile service from `backend/proto/profile/v1/profile.proto`, complete `PROFILE_METHOD_POLICY`, retained caller registry and actor guard, private `VerifiedReadAuthority`, `AuthorityRpcDeadline`, original call deadline layer and GatewayProfile edge budget. The specification, project assessment and execution plan were read; analysis does not change product acceptance.

Exact writable paths:

1. `backend/services/profile-service/src/adapters/in/grpc/concert_identity.rs` (replace unavailable stub).
2. `backend/services/profile-service/tests/concert_identity_rpc.rs`.
3. `backend/services/profile-service/tests/support/concert_identity_fixture.rs`.
4. `backend/services/profile-service/tests/support/concert_identity_native.py`.
5. This document, `docs/mcr/backend/profile-self-rpc.md`.

All owner application/repository code and tests, proto/policy/server/module/config composition, manifests/lockfile/platform/other services/frontend/art and `docs/source/` remain read-only. Existing Post author authority remains unchanged. No new dependencies or migration are required.

## Handler behavior

The actual generated `/profile.v1.ProfileService/ReadConcertSelfProfile` handler requires `VerifiedCaller(Gateway)`, `VerifiedActor` and the private `VerifiedReadAuthority` installed only by the successful retained signed guard. Bare metadata or externally constructed caller/actor extensions cannot replace that private context. The request subject must be the exact canonical lowercase hyphenated nonnil UUID and match both actor identities. The production guard also binds method, body, caller, audience and actor and consumes replay state before dispatch.

The handler uses `AppState.concert_self_profile`; the configured minimal owner repository supplies current user ID, display name, optional bio and statement observation only. Private self content is an internal owner projection, not authorization to disclose another user's profile and not a C03 PublicProfile. The successful wire requires `profile`, preserves exact validated content, maps only empty bio to absent and supplies `cache-control: no-store`. Observation time is not a revision, TTL, session/provider/event fact or admission proof.

One captured deadline covers the entire owner await. Its ceiling is the retained `GatewayProfile` budget minus `LOCAL_TIMEOUT_MARGIN` via `grpc_deadline::local_deadline`, currently 800 ms minus 100 ms. The private context clamps that ceiling to time already spent buffering/verifying/dispatching, the original caller's monotonic stop, captured proof monotonic stop and strict signed wall expiry. The handler checks before owner dispatch, immediately after await even on errors or late noncooperative completion, and after response construction before return. It wraps the whole owner await in `timeout_at` without creating a new budget, retry or cache. Dropping a future does not establish instant database cancellation.

| Condition | Fixed handler result |
| --- | --- |
| Missing verified caller, actor or private current context | UNAUTHENTICATED |
| Verified sibling caller or subject mismatch | PERMISSION_DENIED |
| Noncanonical/nil/invalid subject | INVALID_ARGUMENT `Invalid Profile subject` |
| Missing/malformed original deadline | Retained INVALID_ARGUMENT `Original caller deadline required` |
| Elapsed original/proof/local authority | DEADLINE_EXCEEDED; retained fixed budget message or `Profile read deadline exceeded` |
| Absent, deleted, tombstoned, malformed or misbound owner content | NOT_FOUND `Profile content unavailable` |
| Owner/database/decode failure | UNAVAILABLE `Profile owner unavailable` |

The guard retains legacy clock skew for older RPCs. A proof already past its real signed expiry receives no strict private read context, even if the legacy verifier tolerates that timestamp, so this RPC returns UNAUTHENTICATED without owner dispatch. A context that expires during this RPC fails with DEADLINE_EXCEEDED. No platform or older RPC semantics are amended here.

## Validation design and resource boundary

The generated-service fixture serves the actual `ProfileGrpcServer` behind production `grpc_method_policy::guard`, Profile method policy, real caller registry and actor verifier, and `CallDeadlineLayer`. The actual generated client sends framed protobuf bodies signed by the retained Gateway signer. Controlled mode replaces only the owner repository and uses a deliberately unusable lazy pool, with seeding disabled. This is plaintext, private loopback transport; it does not qualify TLS, deployed credentials, Gateway HTTP or a real browser.

Signed cases cover valid self; missing/wrong/sibling caller; missing/bad/forged signature; wrong audience/method/body/actor; forged metadata and directly constructed caller/actor extensions without private authority; replay; malformed IDs; missing/invalid/zero original budget; actual proof expiry including legacy skew; fixed missing/malformed/misbound/dependency failures with no cached fallback; whole-await timeout; late blocking owner completion; retained local ceiling and proof expiry shortening longer original deadlines. Dispatch counters establish refusal before owner work. Completion/drop counters distinguish cancellation of pending work from a late completed poll.

The owned native controller invokes this exact test binary first for signed deterministic transport, then creates one fresh marker/token/device/inode-owned PostgreSQL 16 cluster with UTF-8, scram credentials, a private Unix socket and TCP disabled. Database: `mcr_profile_self_rpc`. It applies all seven exact retained Profile SQL migrations in order directly, with SHA-256 pins and recorded exits. This is not SQLx migration-ledger or service-startup qualification. No Docker, imported default service or live database is used.

The native real-owner RPC runs with a read-only role permitted only `profiles(user_id,nickname,bio,deleted_at)` and `profile_user_deletion_tombstones(user_id)`. Explicit broad private-column/read-write refusals accompany success. Real RPC coverage includes exact four-byte Unicode 40/160 maxima, under-1024-byte minimal encoded response, private-field canary exclusion, fresh edits/empty bio, absent/deleted/fenced/malformed/overlong/2 MiB content, statement/commit visibility, actual table-lock timeout and recovery of the same single-connection reader pool, denied SQL privileges, stopped-server dependency failure and restart recovery. The controller stops/removes only its exact owned cluster and preserves failed attempts separately.

Resource coordination is mandatory: the Auth-client worker held `/Users/lhs/Desktop/mcr-build/backend` when this source was prepared. No Cargo, rustc, test binaries or PG start occurs before explicit transfer. Subsequent validation uses that retained target only, Rust `+1.88.0`, `--locked --offline`, jobs 2 and both dev/test debug settings zero. Each attempt receipt binds code/input hashes, binary SHA-256 and artifact profile to actual test commands/exits and logs; final handoff binds committed ancestry to those same source bytes.

## Executed evidence

Evidence root: `/Users/lhs/Desktop/mcr-build/profile-self-rpc/`. Final native/transport receipt: `attempt-1793705860403297000-c6e61a0d/receipt.json`. Its code/input hashes match this candidate. Actual RPC test binary SHA-256: `e4905e4bfd24ca6087fda9ecc7945406456ac59ce3fe2835eaf62115387129ec`. `build-attempt-2.jsonl` and its receipt bind the executable and debug-zero artifact profile to the actual code hashes and build command. All commands below use the assigned worktree root, Rust `+1.88.0`, the retained shared target, `--locked --offline`, jobs 2 and both dev/test debug settings zero.

| Command after `cargo +1.88.0` (unless stated otherwise) | Exit and evidence |
| --- | --- |
| `test --manifest-path backend/Cargo.toml --locked --offline -p profile-service --test concert_identity_rpc --no-run --message-format=json` | 0; `build-attempt-2.jsonl`, `.stderr.log`, `.receipt.json` |
| `python3 backend/services/profile-service/tests/support/concert_identity_native.py --binary /Users/lhs/Desktop/mcr-build/backend/debug/deps/concert_identity_rpc-d100d9d54a59fa06 --build-metadata /Users/lhs/Desktop/mcr-build/profile-self-rpc/build-attempt-2.jsonl` | 0; `controller-attempt-1.log` and the attempt receipt/logs: 12 signed controlled-owner transport tests, 5 native online tests, 1 stopped-server test, 1 repeated recovery invocation |
| `test --manifest-path backend/Cargo.toml --locked --offline -p profile-service --lib -- --skip config::tests::external_migration_mode_rejects_schema_damage_without_owner_privileges` | 0; `profile-lib-db-free.log`: 21 passed, exactly 1 filtered out |
| `test --manifest-path backend/Cargo.toml --locked --offline -p profile-service --test architecture` | 0; `profile-architecture.log`: 1 passed |
| `test --manifest-path backend/Cargo.toml --locked --offline -p profile-service --test contract profile_author_authority_contract` | 0; `post-author-contract.log`: 4 passed |
| `test --manifest-path backend/Cargo.toml --locked --offline -p profile-service --test unit config::` | 0; `profile-config.log`: 13 passed |
| `test --manifest-path backend/Cargo.toml --locked --offline -p platform --lib actor_delegation::read_authority` | 0; `private-read-context.log`: 6 passed |
| `test --manifest-path backend/Cargo.toml --locked --offline -p platform --lib actor_delegation::authority_deadline` | 0; `authority-deadline.log`: 3 passed |
| `test --manifest-path backend/Cargo.toml --locked --offline -p platform --lib grpc_method_policy` | 0; `retained-method-policy.log`: 10 passed |
| `test --manifest-path backend/Cargo.toml --locked --offline -p api-gateway --test read_retry_transport` | 0; `gateway-generated-fixture.log`: 1 passed; existing generated service fixture is compatible |
| `test --manifest-path backend/Cargo.toml --locked --offline -p friend-service --test contract --no-run` | 0; `friend-generated-fixtures.log`: retained Profile generated-service fixture compiled; no Friend database contract runtime acceptance claimed |
| `check --manifest-path backend/Cargo.toml --locked --offline -p profile-service --lib --bins` | 0; `profile-bin-check.log`: library and configured binary compile |

`retained-checks.json` records all ten bounded retained command argv, exits and logs. Total distinct passing tests: **77** (18 new RPC tests and 59 retained checks), plus **one repeated recovery invocation**. The earlier full-library attempt repeated 21 of these tests and does not add distinct coverage. The previous owner-only suite was not rerun; the actual owner path is exercised through the new generated RPC instead.

Native environment: PostgreSQL **16.14 (Homebrew)**. All seven actual Profile migrations applied by direct SQL with exit 0 and recorded hashes. The final owned cluster root was stopped and removed; exact ownership and cleanup are recorded in the native receipt. `release-confirmation.json` records the later process/root recheck and release of `/Users/lhs/Desktop/mcr-build/backend`. No Cargo, test binary or PostgreSQL is started after release without a new coordinated transfer. Final static checks and exact five-path/unchanged-input checks are recorded in `static-checks.json` and `scope-audit.json`; final committed ancestry and source/log hashes are bound in `handoff.json`.

## Preserved failures and explicitly unrun retained behavior

The first RPC compilation exited 101 because the two new SQLx `after_connect` closures took one argument instead of the retained two-argument API. `build-attempt-1.jsonl`, `.stderr.log` and `.receipt.json` preserve that failed source/diagnostic evidence. The fixture closures were corrected; build attempt 2 and the actual signed/native run then passed. No central dependency change was needed.

The initial broad `cargo ... -p profile-service --lib` invocation exited **101: 21 passed, 1 failed**. The retained `config::tests::external_migration_mode_rejects_schema_damage_without_owner_privileges` could not acquire its default Testcontainers fixture because `/var/run/docker.sock` was unavailable. The raw log is preserved as `profile-lib-with-unconfigured-db-failure.log` (also `profile-lib.log`), with receipt `retained-failed-docker-dependent-lib.json`. This is a failed whole-library attempt, not full `--lib` acceptance. The schema-damage/database behavior of that unrelated retained case is **NOT_RUN** here; the explicit bounded rerun excludes only its exact name. No Docker/container service was started or remediated, and no fixture or global database cleanup was attempted. The new RPC's native owned database tests are separate passing evidence and do not stand in for that retained startup/schema-damage case.

A preparatory worker-boundary invocation used `--doc` instead of `--allow-doc` and exited 2 (usage error). It is recorded as a failed invocation, not evidence of compliance. The corrected command explicitly names the five-path assignment document. Preparatory and final Rustfmt, Python AST/root, planning and exact boundary checks are distinct from runtime proof.

## Remaining acceptance and goal

No Auth/session/provider/event eligibility assertion, avatar authorization, Gateway consumer, multiuser product flow, TLS deployment, load or release acceptance is delivered by these five files. The broader BE04 persistent goal is aligned and unfinished. It previously reported BLOCKED after the earlier shared-dependency stop; implementation resumed under explicit authorization without claiming a false API status. The user subsequently resumed the goal, and a fresh `get_goal` now confirms **ACTIVE** with the same objective and no token budget. This bounded handoff does not complete broader BE04. Concrete integration-review fixes remain required.
