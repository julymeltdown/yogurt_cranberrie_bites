# Task 9 - BE03 application leaves and OAuth boundary

Date: 2026-11-06
Worktree: /Users/lhs/Desktop/mcr-worktrees/exec-w1-auth (branch codex/mcr-exec-w1-auth)
Base: 501710d29 (parent of delivery commit)

## Provenance

- Candidate commit: 11be1d4395354b02c94211ed33f7992105750e42
- Candidate base: 09c9b6b471bd53e6e739a58d00244133de63e418 (merge-base with 501710d29)
- Candidate tree: df28cd9a1e1559e723010255dd5dafd82cf71135
- Packet: /private/tmp/mcr-be03-oauth-12cc/source01/candidate-postimages-11be1d439535.json
- Master drift on all 16 packet paths between 09c9b6b4 and 501710d29: none
- Applied 15 application/shared leaves byte-exact with git checkout 11be1d439 -- <paths>.
  - Verified blob OIDs against the packet: config/mod.rs 11f25d91e5142c3d894e06a47b8284cf7c3b2a7b,
    oauth_routes.rs 1fa8c9bdb1ae3019c320aaefed371af020c2e194.
- The 15 leaves: oauth_routes{,/form_post}.rs, browser_auth.rs, mod.rs, server.rs,
  config/mod.rs, config/hardening_tests.rs, tests/{be03_oauth_boundary,oauth_form_post_browser,
  support/mod,unit/config,unit/http_support}.rs, scripts/mcr/be03_oauth_boundary.py,
  scripts/testing/oauth-form-post-browser.cjs, docs/mcr/core-contract.md.

## Integrated behavior

- OAuthExposurePolicy: public providers always false (no operator bypass); controlled
  transport only in Parity with a 32-4096 byte independent HMAC key, exact browser
  origin, Secure cookie and CORS-origin agreement. Absent settings keep all providers off.
- Signed host-only Secure HttpOnly SameSite=Lax state cookie binds provider, issue time,
  Auth state and one of /world /feed /me /meet/people. Callback returnTo never overrides it.
- Cookie-only 303 to /auth/callback with exactly status in success|denied|error and returnTo.
  Denied/error clears state without Auth exchange; no tombstone write.
- Public OAuth OFF observed live: GET /auth/oauth/google/authorize -> 404.

## Commands

- cargo +1.88.0 check --locked -p api-gateway --lib --tests -> exit 0
  (CARGO_TARGET_DIR=/Users/lhs/Desktop/mcr-build/exec-w1-auth, CARGO_BUILD_JOBS=4)
- Private PG16 cluster: /Users/lhs/Desktop/mcr-build/exec-w1-auth-pg, port 55432,
  max_prepared_transactions=256, system identifier 7704864603555847116.
- Test fixture adaptation (environment-only): test_support::postgres gained
  start_postgres_with_url_or_native() using MCR_TEST_POSTGRES_DATABASE_URL when the host has
  no container runtime; Docker remains the default. service_pool and the gateway quota test
  use it. No assertions were weakened.

## Test results

(see final session results appended below)

## Real HTTP run (isolated PG database mcr_auth_e2e, auth-service parity)

- POST /auth/email/signup -> 200 {"status":"PENDING_VERIFICATION","userId":...}
- verification code read from the isolated email_verifications row (test-only, not a seed)
- POST /auth/email/verify -> 200 {"status":"VERIFIED"}
- POST /auth/login -> 200 access/refresh tokens
- POST /auth/refresh -> 200 rotated tokens
- POST /auth/logout -> 204
- POST /auth/login wrong password -> 400 {"code":"BAD_REQUEST","message":"Invalid credentials"}
  (generic; no account existence detail). After 5 wrong-password audits, further attempts are
  recorded as lockout_refusal (login_audit success=f, credential_attempt=f); the response
  remains the same generic rejection, so throttling does not enumerate accounts.

## Gateway QA (real auth-service gRPC + real PG; http://127.0.0.1:18080)

- POST /api/auth/login (fresh isolated account) -> 200 + Set-Cookie: refresh_token=<redacted>
- POST /api/auth/login wrong password -> 401 {"code":"GATEWAY_401","message":"Invalid credentials"}
- POST /api/auth/refresh with cookie -> 200
- POST /api/auth/logout with cookie -> 204
- GET /auth/oauth/google/authorize -> 404 (public OAuth OFF)

## Cleanup

- Auth/gateway e2e processes stopped; private e2e databases dropped.
- Private PG cluster and test fixture databases are disposable and listed for teardown.

## Final results (this session)

- cargo +1.88.0 check --locked -p api-gateway --lib --tests -> exit 0
- cargo +1.88.0 test -p auth-service --lib -> 52 passed; 0 failed
- cargo +1.88.0 test -p api-gateway --lib -> 147 passed; 0 failed
- Full cargo +1.88.0 test -p auth-service -> exit 101 at test concert_admission_rpc
  (11 passed, 5 failed: "owned fixture root required"). Those tests require the dedicated
  AUTH_ADMISSION_RPC_TEST_OWNER_ROOT / _OWNER_IDENTITY / _OWNER_TOKEN / _DATABASE_URL
  allocation and are not satisfiable in this environment. No assertion in them was changed.
- Full cargo +1.88.0 test -p api-gateway -> exit 101 at test target architecture
  (3 passed, 1 failed): pre-existing layering::incoming_adapters_cannot_import_sql_drivers_or_outgoing_implementations
  flags src/adapters/in/http/concert_access.rs, which this task did not modify
  (git diff 501710d29 HEAD -- that file is empty). Not caused by the BE03 leaves.

## Cleanup receipts

- auth-service and api-gateway e2e processes: stopped (both sessions exited 1 on signal).
- mcr_auth_e2e, mcr_gateway_e2e and all mcr_fixture_* databases: dropped.
- Private PG16 cluster /Users/lhs/Desktop/mcr-build/exec-w1-auth-pg: pg_ctl stop exit 0;
  127.0.0.1:55432 no response.
- Worktree clean at commit 2c1ed8fb2.


## Attempt 2 (2026-11-06): blockers investigated

### auth-service owned fixture

- Ran backend/scripts/mcr/auth_admission_rpc_smoke.py with the compiled auth-service binary and
  concert_admission_rpc / concert_identity_owner test binaries against a fresh owned PG16 cluster.
  Result: FAIL, failure="exact-auth-migration-set", cleanup=PASS, report
  /tmp/be03-admission-rpc-report.json.
  Cause: the script requires the Auth migration set to equal versions 1..13, while the current
  service owns migrations 1..19. The script is stale relative to the repository and cannot
  materialize its fixture as written.
- Materialized the equivalent fixture independently (migration_smoke.Cluster, owner mcr_owner,
  mode-700 marker root, Unix socket, port 5432, migrations 1..19) with databases
  mcr_auth_admission_rpc, mcr_auth_identity, mcr_auth_identity_exhaustion, mcr_account_deletion.
- Running concert_admission_rpc against it: 12 passed, 4 failed. Three fail with gRPC
  POLICY_UNAVAILABLE (no enrollment-policy publication is seeded by the fixture), one fails with
  user_identities_provider_subject_idx on a reused database. The suite's own hosting needs a
  policy publication that neither the script nor the migration set supplies.
- Full cargo test -p auth-service therefore remains red at this target; all other targets in that
  run passed (lib 52, and the preceding targets listed in the log).

### api-gateway layering

- Moved ConcertAccessContext / ConcertAccessRejection / MAX_CONCERT_ACCESS_TOKEN_BYTES to
  src/concert_access.rs (neutral, outside adapters). The JWT adapter keeps the verifier and
  re-exports the types; the HTTP extractor imports the neutral module. Behavior unchanged.
- api-gateway --lib: 147 passed, 0 failed.
- architecture target now passes the concert_access file and fails on the next pre-existing
  incoming->outgoing import. Remaining exact cascade:
  - src/adapters/in/http/concert_bootstrap.rs: crate::adapters::out::concert_client::BootstrapError
  - src/adapters/in/http/concert_world.rs: out::concert_world::WorldClientError,
    out::concert_identity::ConcertIdentityReadError, out::auth_client::concert_admission::AdmissionReadError
  - src/adapters/in/http/concert_product/mod.rs: out::concert_product::{ProductHttpRequest, MAX_BODY}
  - src/adapters/in/http/concert_participant/mod.rs: out::concert_participant::{failure_status,
    GrpcConcertParticipantClient, Operation, ParticipantHttpRequest, MAX_BODY}; the router takes the
    concrete gRPC client and proto response types, so closing this needs an application port, not a
    type move.
- Commit d12cfc994 contains the neutral concert_access move.

### Cleanup

- Owned fixture root removed by the driver's SIGTERM cleanup; descriptor removed.
- Private TCP PG16 cluster (port 55432) stopped; no listener.


## Attempt 3 (2026-11-06): fixture and layering verified; full suites blocked by container runtime

Worktree /Users/lhs/Desktop/mcr-worktrees/exec-w1-auth, branch codex/mcr-exec-w1-auth,
HEAD d19b04455 fix(gateway/test): derive the retained Profile migration set (clean tree).

### 1. Auth owned-root fixture (PASS)

Command (fresh 54k-entry report, new owned marker/inode root):
  cd backend/scripts/mcr && python3 auth_admission_rpc_smoke.py \
    --binary .../debug/auth-service \
    --rpc-test .../deps/concert_admission_rpc-7299f5dc854fb1f5 \
    --owner-test .../deps/concert_identity_owner-318352f6efb6d148 \
    --pg-bin /opt/homebrew/opt/postgresql@16/bin --report /tmp/be03-admission-rpc-report3.json
-> exit 0; {"result":"PASS","cleanup":"PASS","real_postgres_rpc_tests":5,"restart_persistence":"PASS"}
- PG16.14 owned cluster; Auth migration set derived from migration.migration_manifest:
  19 migrations, ledger 19; the previous hardcoded 13 is gone
  (auth_identity_smoke.py and auth_admission_rpc_smoke.py both use len(manifest)/max(manifest)).
- concert_admission_rpc: 5 passed, 0 failed. concert_identity_owner retained regressions:
  22 passed, 0 failed.
- Enrollment policy: both test files publish an explicit synthetic revision and consent
  through EnrollmentStore (commit 4f5d504c5), so POLICY_UNAVAILABLE no longer occurs.
- Report: /tmp/be03-admission-rpc-report3.json; log .../build/exec-w1-auth/be03-smoke3.log.

### 2. Gateway layering (PASS)

- Error enums live in src/domain/concert_identity.rs (commit a3d547f64); inbound
  concert_world.rs imports crate::domain::concert_identity::{ConcertIdentityReadError,
  AdmissionReadError} and the outbound adapters re-export the same types. Diff of
  concert_identity_error mapping against 501710d29 is import-path only; GatewayError
  arms are byte-identical (gateway_timeout / ConsentRequired / PolicyUnavailable /
  forbidden / ServiceBusy). No allowlist or assertion change.
- cargo +1.88.0 test -p api-gateway --test architecture -> exit 0, 4 passed, 0 failed.

### 3. Full suites (NOT GREEN; environment and pre-existing test debt)

- cargo +1.88.0 test -p auth-service --no-fail-fast -> exit 101
  totals across 37 targets: 234 passed, 108 failed, 23 ignored.
  80 of the 108 failures are "container runtime unavailable: ... /var/run/docker.sock";
  27 are the env-owned admission/identity fixtures (proven green by the fixture run above);
  1 is pre-existing. The 80 Docker-gated and the 1 pre-existing failure are recorded gaps,
  not scope absorbed by BE03.
- cargo +1.88.0 test -p api-gateway --no-fail-fast with private PG16 and isolated fixture
  databases (MCR_TEST_POSTGRES_DATABASE_URL, GATEWAY_TEST_DATABASE_URL,
  GATEWAY_TEST_CREATE_ISOLATED_DATABASES=1, GATEWAY_TEST_INITDB_SYSTEM_IDENTIFIER)
  -> exit 101; verifier-measured totals across 46 targets: 479 passed, 135 failed,
  6 ignored. (An earlier 588/72 line in this file did not reproduce and is superseded.)
  Failure classes (135 total): 110 Docker-gated container runtime, 8 missing fixture env
  (MCR_GATEWAY_PROFILE_OWNER_*, OAUTH_BROWSER_NODE), 2 parity, and 15 assertion failures.
  The 15 assertions include the 11 BE03-attributable unit failures fixed in Attempt 4.
  api-gateway --lib: 147 passed, 0 failed; architecture: 4 passed, 0 failed.
- Docker Desktop was launched (com.docker.backend) but crashed before the VM socket
  existed: "opening tray: starting electron: unmarshaling start request: unexpected EOF".
  No container runtime is available, so testcontainers-backed Postgres/Redis targets
  cannot run on this host.

### 4. Cleanup receipts

- Owned fixture roots removed by the smoke driver (cleanup PASS).
- Fixture databases created during the gateway run dropped from the private PG16 cluster
  on 127.0.0.1:55432; cluster left running for its owner.
- Docker Desktop backend terminated; no docker.sock.
- Worktree clean at d19b04455; no new commit was needed for items 1-2.


## Attempt 4 (2026-11-06): BE03-attributable unit failures fixed

Worktree /Users/lhs/Desktop/mcr-worktrees/exec-w1-auth, branch codex/mcr-exec-w1-auth,
base d19b04455. No register or lockfile edits; no merge or push.

### Root causes

- Eight OAuth unit failures: 2c1ed8fb2 added OAuthExposurePolicy and made oauth_routes
  gate on it, while the unit harness kept the default (empty) policy, so authorize and
  callback returned 404 before reaching the previously exercised behavior.
- Three timeout failures: a3d547f64 raised http_support request_timeout_ms 1000->5000 and
  journal_client_timeout_ms 900->4000 with no test that needed the longer window; the
  three http_concert_access assertions still require a sub-1s remaining deadline and a
  <2s outer timeout. All other gateway configs keep 1000/900.

### Fix

- Restored request_timeout_ms=1000 and journal_client_timeout_ms=900.
- authorize now parses the provider before the transport gate: an unknown provider is a
  400 input error, while every known provider stays 404 unless its controlled transport
  is explicitly enabled. Production composition still omits the OAuth router entirely.
- Added RouteHarness::with_oauth_transport (Parity, controlled providers google,apple,x,
  independent 32+ byte key, loopback browser origin) for the tests that must exercise
  302/303 handoff behavior. Default-policy tests still prove OFF=404.
- OAuth unit tests now use HMAC-bound state cookies issued by the authorize handoff and
  assert the post-BE03 contract (303 to /auth/callback, provider-denial page) instead of
  the retired 200 JSON callback shape. No security assertion was relaxed; one regression
  test was added for default-policy 404.

### Verification

- cargo +1.88.0 test -p api-gateway --test unit -> exit 0, 217 passed, 0 failed
  (base 216/216 plus the new default-policy-404 regression).
- cargo +1.88.0 test -p api-gateway --test architecture -> exit 0, 4 passed, 0 failed.
- backend/scripts/mcr/auth_admission_rpc_smoke.py (fresh owned PG16, report
  /tmp/be03-admission-rpc-report4.json) -> exit 0, result PASS, cleanup PASS,
  19 migrations, 5 RPC tests, 22 retained owner tests, restart_persistence PASS.

### Residual classification

- Auth: 234 passed / 108 failed / 23 ignored = 80 Docker-gated + 27 owned-fixture
  (green under the smoke above) + 1 pre-existing.
- Gateway verifier baseline: 479 passed / 135 failed / 6 ignored = 110 Docker + 8 env +
  2 parity + 15 assertions. This session removed the 11 BE03-attributable assertions;
  the Docker/env/parity classes and the 1 pre-existing auth failure remain recorded gaps,
  not absorbed into BE03 scope.
- Docker/container-runtime coverage was not re-run: no container runtime on this host.

### Cleanup receipts

- Smoke-owned fixture roots and databases removed by the driver (cleanup PASS).
- No auth-service or gateway process left running.
