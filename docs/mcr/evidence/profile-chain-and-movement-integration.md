# Profile chain and movement integration checkpoint

Reviewed 2026-11-03. Profile owner transport is INTEGRATED within BE04; FE15 is
MOCK_READY. Accepted counts remain 11/112 required and 11/118 total. The new
frontend chat advances FE06 independently from real email account journeys.

## Delivered changes and ancestry

| Change | Worker | Main integration |
|---|---|---|
| Profile wire, policy and composition prerequisite | d63fb2b | ffdcf81 |
| Guarded minimal Profile RPC | 4b9d054 | af8b458 |
| Configured bounded Gateway Profile client | e132899 | c6404c3 |
| PostgreSQL query lifetime correction | 9ff053a | 600582d |
| Actual Gateway/Profile/PG integration gate | 1244ef2 | 3d6a518 |
| FE15 movement/camera and boundary correction | 0511aea / 80a171d | d91ee02 / aec5d17 |
| Independent FE15 review | 7cee66a | b381d03 |

Parent reviewed exact ownership, production diffs, caller/actor/deadline guards,
whole-foot routing and resource lifecycle. Source, dependency and log hashes were
checked against the executed candidates. The integration receipt's pre-result
document differs from the final result-only document; its three Rust/controller
inputs and remaining production inputs are unchanged and separately verified.

## Results

- Initial Profile RPC checks: 77 distinct passes plus one recovery repeat. The
  full Profile library attempt failed on the retained Docker-dependent schema
  damage fixture because the socket was unavailable. Its exact bounded rerun
  excludes that fixture; this is not a full-library PASS.
- Gateway client: 268 top-level tests and ten compile-fail doctests pass, with
  library check and binary build. Capacity subprocess cases are not extra coverage.
- Real-chain tests exposed abandoned SQL after Gateway cancellation: all 256
  edge permits returned, but one lock waiter held the sole database connection
  beyond three seconds. Both failing attempts are preserved.
- The correction scopes the original RPC stop into the repository and installs
  a transaction-local PostgreSQL statement timeout before the content SELECT.
  Nine actual-chain cases now pass. With the lock still held, cancellation leaves
  zero waiters and a reusable single-connection pool after 734 ms. Local and
  original-short refusals occurred at about 702 ms and 152 ms. Settings restore
  after success/error/cancellation; permission, outage and restart checks pass.
  Cleanup is bounded by the deadline, not instantaneous wire cancellation. A DB
  cancellation just before expiry may return fixed Unavailable; elapsed authority
  remains DeadlineExceeded/AccessExpired. Private diagnostics remain excluded.
- FE15: worker 958 tests/22 files, including 87 movement tests, build/lint and
  four native browser widths plus short-height views pass. Independent QA closes
  stale same-owner visibility and rejected valid bottom-edge routes. No original
  route hang was reproduced. Parent ran the affected movement/runtime suites:
  221 tests pass, plus build/lint; mobile and short screenshots were inspected.
- Parent built all six retained services coherently at 600582d with Rust 1.88,
  locked/offline dependencies, two jobs and debug disabled: exit 0. All 1,108
  backend build-input hashes and six executable hashes match. The real email
  browser chat receives these immutable binaries; build success alone is not its
  runtime acceptance. Local reset links now target the configured browser origin;
  the launcher regression suite passes 17 tests.

The [artifact index](profile-chain-and-movement/index.json) preserves 157
hash-verified evidence references, including historical failures, final logs,
receipts and browser images. Historical logs retain their original bytes and
whitespace. Node 24.22.2, pnpm 11.7.0, Chrome 156 and native PostgreSQL 16 were used.

## Remaining scope

No HTTP-to-Concert admission, event eligibility, production TLS, complete BE04,
actual multiplayer, selected production art, devices or release gate is accepted
by this checkpoint. PostgreSQL tests use isolated owned fixtures and direct
migration SQL, distinct from service startup ledger qualification. The frontend
email journey and FE06 theme work have separate active goals. Model/reasoning
choices are user-controlled; orchestration no longer overrides them.
