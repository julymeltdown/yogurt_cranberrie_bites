# BE04 Gateway Profile client to Profile owner integration gate

Status: **IN_PROGRESS** pending integrator review. The bounded actual Gateway→guarded Profile→native PostgreSQL gate passes all **nine distinct tests**, including cancellation, original deadlines, same-pool reuse, permission denial, outage and restart. Two earlier red runs exposed the production query-lifetime defect and remain preserved. Whole BE04, Gateway HTTP, TLS, Concert admission and event acceptance remain open.

Assignment base: `af8b45893b8e979371bc5fccaff071770dfd98a4` (contains the guarded Profile self RPC and owner read); client prerequisite: `c6404c33e2d284b776652b416d0b96ed45856a28`; final validation and patch base: `953bddffb2de7dd6c41869b923f71855a7d19f50` (reviewed production correction `9ff053a`, parent-integrated as `600582d`); branch `codex/mcr-be-profile-client-integration`; isolated worktree `be-profile-client-integration`. Only the integrator publishes; there is no worker push.

## Scope and provenance

New test and harness code over the retained `julymeltdown/port-rust` foundation revision `704f42149d5e8e35e1d578676d8ae2dac4f0f9c2` (prefix `boilerplate-grpc-rust/`, destination `backend/`, per the import manifest). The read-only JWT key and context helpers follow the Gateway Profile client transport test at client source revision `e132899776cae2644ddd0283cd31c18d5f6c5601` (retained here as `c6404c3`); the owned-cluster checks follow the Profile self-RPC fixture at `4b9d054bafeff170470fc06ab0844427d8106d96` (retained as `af8b458`). Both are re-authored here and neither is imported wholesale; the Profile test helper is not used because Gateway has no direct `profile_application` dependency. No existing application, handler, proto, platform, config, manifest, lockfile, frontend, art or `docs/source/` path is edited and no dependency is added; `api-gateway` already has the `profile-service` dev dependency.

Exact writable paths:

1. `backend/services/api-gateway/tests/concert_profile_owner_integration.rs`
2. `backend/services/api-gateway/tests/support/concert_profile_owner_fixture.rs`
3. `backend/services/api-gateway/tests/support/concert_profile_owner_native.py`
4. This document.

The client under test is `GrpcProfileClient::read_concert_self_profile` with its observation and error types, frozen by the parent. This is an exact test and harness scope, not broad Gateway ownership.

## What the integration exercises

The path is one real chain with no controlled owner and no hand-written reply:

1. `JwtVerifier::verify_concert_access` over a freshly generated RSA key produces the borrowed `ConcertAccessContext`. There is no forged context constructor.
2. The configured `GrpcProfileClient` (service identity, tracing, actor signing and lazy channel) calls the actual generated `ProfileGrpcServer`.
3. That server runs behind production `grpc_method_policy::guard` with the real Profile method policy, caller registry, actor verifier and `CallDeadlineLayer`.
4. It reads the real Profile `AppState` from `build_state`, built with explicit test configuration (seeding and workers disabled, unused database URL) over a pool to the owned fixture, ending at the minimal SQL owner.

A counting layer in front of the guard records requests that reach the server, so refusals before dispatch are observed rather than inferred.

## Behavior covered

- A context that expired locally sends nothing to the server.
- A wrong Gateway credential is denied by the production guard.
- Native PostgreSQL 16, read-only role limited to `profiles(user_id,nickname,bio,deleted_at)` and `profile_user_deletion_tombstones(user_id)`: a signed self read returns the exact four-byte 40 and 160 character maxima under the 1024 byte decode cap, with statement observation inside the call window and private canaries absent from diagnostics. A second JWT subject receives only its own content because no request field selects a subject. Broad private columns and writes are refused for the read role.
- Fresh edits are read without caching and empty bio normalizes to `None`. Absent, soft-deleted, tombstoned and malformed or oversized (including 2 MiB) content all return fixed `NotFound`. Uncommitted edits stay invisible until commit.
- Permission failure and a stopped owned server return fixed unavailable.
- With the table locked, an original budget longer than the retained Gateway Profile ceiling ends near that ceiling with a fixed fail-closed error, and a shorter original budget expires the context. Per the integrator's explicit error-contract decision, a millisecond PostgreSQL timeout just before the exact authority stop may return fixed `Unavailable`; at/after the stop the RPC/client deadline fences retain `DeadlineExceeded` or `AccessExpired`. The test accepts only those exact variants, refuses success, and requires the long call to consume at least ceiling minus 10 ms and the 150 ms call to consume at least 140 ms. No typed error API is expanded for rounding. After each deadline, while the table lock is still held, the owner wait must clear and the same single-connection pool must execute a probe within 350 ms. The short-budget cleanup must finish before a fresh local ceiling could expire. After rollback the same client reads again.
- A read is cancelled only after the test has observed a Gateway edge permit held and a SQL statement waiting on the lock. Capacity returns to full; while the table lock is still held, the owner lock wait clears and the single-connection pool becomes reusable. The test then unlocks the table and reads through the same client. This verifies bounded cleanup without claiming immediate PostgreSQL cancellation.
- The same reader connection retains `statement_timeout=0` and read-only mode after success, dependency error, deadline and cancellation; the production transaction-local timeout does not leak into the pool.
- After the exact owned server is restarted, a fresh read succeeds.

Auth account, session and event eligibility are not asserted or invented from Profile content.

## Native controller

`concert_profile_owner_native.py` runs the compiled test binary: first the cases that need no database, then a new owned PostgreSQL 16 cluster (UTF-8, private socket, scram credentials, database `mcr_gateway_profile_owner`) with the seven retained Profile migrations applied directly in order with recorded hashes, then the online cases, the outage case with the server stopped, and the restarted case. It uses one test thread because the Gateway Profile edge limiter is process-global.

All locations are arguments, with no hardcoded user-home paths: `--binary`, `--build-metadata`, `--target-dir`, `--output` and `--pg-bin`. It refuses a binary outside the declared target directory, metadata that does not describe that exact binary, a nonzero debug level, a migration count other than seven, and any foreign or replaced cluster root. It binds source, binary and log hashes into a per-attempt receipt, redacts credentials, keeps failed attempts separately and removes only its exact owned root. Migrations are applied by direct SQL, which is not SQLx ledger or Profile startup qualification, and transport is plaintext loopback, which is not TLS qualification.

## Executed validation and resource boundary

Rust 1.88, `--locked --offline`, jobs 2, both `CARGO_PROFILE_DEV_DEBUG=0` and `CARGO_PROFILE_TEST_DEBUG=0`, shared target explicitly leased for each run. All three builds exit zero. The final command is:

```sh
cargo +1.88.0 test --manifest-path backend/Cargo.toml --locked --offline \
  -p api-gateway --test concert_profile_owner_integration --no-run --message-format=json
python3 backend/services/api-gateway/tests/support/concert_profile_owner_native.py \
  --binary "$BINARY" --build-metadata "$BUILD_METADATA" --target-dir "$TARGET" \
  --output "$EVIDENCE/native" --pg-bin "$PG_BIN"
```

`BINARY` is the exact Cargo JSON executable; `BUILD_METADATA` is the saved JSON output. The external handoff records the concrete paths and environment. The native controller supplies one test thread and `--nocapture`.

Evidence root: `mcr-build/gateway-profile-integration/`. Final build: `build-attempt-3/`, exit 0. Final native receipt: `native/attempt-1793707928112387000-398de256/receipt.json`, SHA-256 `a1319c0aa6473a2614b8a016ee852e531b65fe536181b5873058ccb4eeefcf5d`, controller exit 0. PostgreSQL **16.14**. Every phase exits zero: two database-free guards, five online cases, one stopped-database case and one restarted-database case. All nine distinct tests pass, none ignored; phase filters select disjoint subsets of that same nine-test binary.

Observed final cancellation cleanup: Gateway capacity **256/256**, **zero** owner lock waiters, and the same single connection reusable while the table lock remained held, at **734 ms**. Long-budget result: `DeadlineExceeded`, **701995 microseconds**. Short original 150 ms result: `AccessExpired`, **152072 microseconds**. Both deadline cleanup probes pass while the lock remains held; the shorter one finishes before a fresh 700 ms local ceiling could expire. Setting restoration, subsequent reads, fixed permission/outage errors and a fresh post-restart read all pass. These results use the production transaction-local timeout; the fixture installs none.

`build-attempt-1/` and `build-attempt-2/` retain successful compilation of the earlier candidates. Native attempts `attempt-1793707087959036000-7a565b98` and `attempt-1793707174758436000-f017b49b` retain receipts and logs. Both controller exits are 1: each guard phase passes two tests and each online phase exits 101 with four passes and one cancellation failure. Their outage/restart phases were **NOT_RUN**. The second red run recorded full Gateway capacity but one PostgreSQL lock waiter after 3155 ms and an unavailable single connection while the lock stayed held. Unlocking first would have hidden it. The reviewed producer correction installs a transaction-local `statement_timeout` before SELECT and scopes the exact RPC authority stop around owner execution. No production file is authored in this four-path patch.

All three owned roots are absent and their postmasters stopped. `release-confirmation-final.json` records a fresh quiescent process audit and explicit shared-target release. No more Cargo or PostgreSQL is run after release. Final Rustfmt, Python AST, planning, backend boundary and staged whitespace/scope checks are recorded in the handoff with exits. The planning check covers all 13 original archives; these static checks do not substitute for the actual-chain run above. The native receipt pins the document as it existed at execution; the handoff pins the final results-only document separately. Executed Rust/harness code and production input hashes remain unchanged.


## Limits

Gateway HTTP entry, TLS, Concert admission composition, event eligibility, multiuser flows and browser behavior are not tested here. The goal for the broader BE04 increment remains active.
