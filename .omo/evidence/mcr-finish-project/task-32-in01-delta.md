# Task 32 - IN01 delta re-run for changed features

Branch `codex/mcr-exec-w5-in01`. Sections 1-8 are the earlier record on candidate
`937ab6699` (status there: PARTIAL). Section 9 records the product fixes and the final
candidate gate; section 10 records the 2026-11-06 correction pass, including the
integrator-authorized scope extension and the boundary-check explanation.

Current gate status: the in-tree stack/driver reports `result: PARTIAL` because its browser
stage is `NOT_RUN` (the browser gate runs as its own driver), with `world_two_client: PASS`
and `cleanup: PASS`. The separate browser report is `RUNTIME_PASS_PENDING_ART_REVIEW` with
all ten checks PASS. The split does not upgrade any delta: D2 remains `NOT_RUN` and D3
content remains `NOT_RUN`.

## Earlier record on candidate `937ab6699`

Status after repair and re-run: **PARTIAL** on candidate
`937ab6699167735995661d91329a238797096ea5` (branch `codex/mcr-exec-w5-in01`, base master
`937ab6699`).

- D1 admission: **PASS** on the real candidate stack (two real accounts, two tickets, two
  distinct streams in one room, mutual membership, authoritative movement, disconnect
  removal, tickets consumed, memberships released, no prepared transactions).
- D2 policy frames: **NOT_RUN**. The candidate's World transport is not composed with the
  Social policy baseline in this gate configuration (`CONCERT_SOCIAL_MODE` is not set), and
  the browser world never opens (see the surviving blocker).
- D3 feed, D4 theme, D5 panels, reconnect: **NOT_RUN**. The shipped shell requires
  `GET /api/concert/events/{eventId}/service-policy` before opening the World; the candidate
  `api-gateway` does not mount that operation (404), so no authorized world, panel, theme or
  socket observation can be produced.
- The two blockers from the previous record were resolved: the shared readiness validator
  now requires the owner-authority check (`ae2092705`), and the repository's isolated
  operator publication path published a policy into the owned Auth database, enabling real
  enrollment consent and admission for both accounts.

## 1. Delta table

| # | Delta | Commits since `1bb911c3c` | Gate assertion | Re-run result | Status |
| --- | --- | --- | --- | --- | --- |
| D1 | Admission: world tickets, membership and canonical enrollment admission | 18 world commits (`51af7f7a8`, `4143e9494`, `c2544b69d`, `8364b25cc`, `b9244f869`, `930380dee`, ...) and 4 enrollment commits (`c093d8a5c`, `c988365ca`, `2f7992d43`, `a3d547f64`) | Two authenticated accounts obtain tickets, join one room on distinct streams with mutual membership, move authoritatively, disconnect cleanly | `world_two_client: PASS`; `world_probe`: mutual membership, movement, peer removal; `world_database`: tickets 2 issued/2 consumed, memberships 0, prepared transactions 0 | PASS |
| D2 | Policy frames: explicit Social policy baseline and invalidation | 2 (`930380dee`, `b9244f869`) | `policy.baseline` follows the world snapshot on the same socket; invalidation withdraws the private view; no second socket | Social transport not composed in this configuration; no world socket in the browser attempt | NOT_RUN |
| D3 | Feed: producer, read composition and Feed/Thread presentation | 26 server + 17 UI commits (`10b1afe68`, `827877828`, `8fea3b43d`, `7eac13ce3`, ...) | Feed panel opens with authorized state, same host/canvas, no socket, movement suppressed while focused | App entry gate stops at the service-policy read; world host never mounts | NOT_RUN |
| D4 | Theme: bounded era palettes in the production shell | 4 (`3321b3e19`, `eacbaab50`, `99b5bbf93`, `8e5cc757f`) | Era change alters the rendered skin, keeps room/map/avatar/position and socket invariants, per-identity persistence | Same entry gate; shell controls unreachable | NOT_RUN |
| D5 | Panels: shell navigation, host lifetime and panel chrome | 17 (`cb44bb77c`, `09be648b8`, `56a0cb7f0`, `78886ffa9`, `0a27b70d7`, ...) | Six panels per client with identical host/canvas, unchanged socket count, peak one | Same entry gate; no world host for either client | NOT_RUN |
| R1 | Reconnect after socket kill | (same shell/transport delta set) | Real transport cut, old socket closed, Reconnect world issues a fresh snapshot with both actors and one socket | No world socket was opened | NOT_RUN |

Complete per-path commit lists: `task-32-in01-delta/delta-commits.txt`.

## 2. Readiness validator fix

Commit `ae2092705` `fix(devstack): require the owner-authority readiness check`.

- `backend/scripts/mcr/runtime_smoke.py`: `readiness_value` now expects the required
  `concert-owner-authority` check for `auth-service` and `friend-service`, matching
  `browser_refresh.current_readiness` and the services' actual `/readyz` schema.
- `backend/scripts/mcr/test_dev_stack.py`: two regressions. The current schema with the
  green required check is accepted; the retained schema without the check, a check marked
  optional, and a check down while the report claims ready are all rejected.
- Proof the regression fails on the old validator: running only the new tests with
  `git show HEAD~1:backend/scripts/mcr/runtime_smoke.py` on the path yields
  `AssertionError: SmokeError not raised` and 5 subtest failures; the current suite is
  `Ran 19 tests ... OK`.
- The unmodified documented launcher now reaches READY with every service validated:
  `dev_stack.py start --duration-seconds 5` exits 0, `result: PASS`, `cleanup: PASS`, and
  `friend-service`/`auth-service` readiness includes `concert-owner-authority: up`
  (`task-32-in01-delta/devstack-fixed-report.json`, `devstack-fixed.log`).

## 3. Enrollment policy publication

Path used: the repository's own isolated operator publication, documented in
`docs/mcr/backend/enrollment-policy-operator-2000.md` and wrapped by
`backend/scripts/mcr/session_deletion_runtime/enrollment.py::owned_auth_adapters`.

- Publisher: retained operator executable
  `/private/tmp/mcr-policy207-operator-o25tayv6/enrollment_policy_browser-frozen`,
  SHA-256 `1c3b16f00dd69ee0305191ed8a310cee3b9f6d5a94f555ec41d7da6713ec113a`, source
  `dee64fd1ac868f4dffbea49c0e75ca8e98727f3a`, ignored case
  `publish_isolated_policy_into_owned_auth`.
- It publishes only into this invocation's owned `mcr_auth` (the candidate has exactly 19
  Auth migrations, matching the operator contract) and records
  `productionApproval: false`, `databaseCreated: false`, `authMigrations: 19`. This is a
  synthetic isolated policy, not legal approval and not a production publication.
- Published policy revision `43bbae7b-18f5-433a-8da3-a925d2e9e9cd`, one document. The real
  public enrollment flow then ran for both accounts: policy 200, bound attempt 201, signup
  202, private HTTP mail capture with the bound verification link, verify 200, gateway login
  200, consent ADMITTED with `acceptedRevision == policyRevision`, admission
  `ADMITTED`.
- No consent text was fabricated in the application, no fixture acknowledgements were
  injected, and admission was not bypassed: both accounts earned `ADMITTED` through the
  public enrollment/consent endpoints.

## 4. Attempt A - in-tree two-principal gate

Command (same in-tree driver as the accepted baseline, against the candidate build):

```
python3 backend/scripts/mcr/world_in01_live.py --bin-dir .../exec-w5-in01-target/debug \
  --concert-bin .../debug/concert-service --report /tmp/mcr-in01-delta/world-gate-report.json \
  --browser-origin http://127.0.0.1:5173 --world --hold-seconds 1200
```

Observed, in order:

- Enrollment publication and two real admitted accounts as in section 3.
- Public C03: `PASS` (avatar PUT 200, preferences PATCH 200, canonical profile PATCH 200,
  stale CAS 409, unauthenticated 401, cross-principal isolation).
- C05 two-principal probe: `PASS` for mutual membership, authoritative movement
  (`consumed_input_ack: 2`), peer-observed removal, two streams in one room.
- Database observations: `issued_tickets_2 PASS`, `consumed_tickets_2 PASS`,
  `released_memberships_0 PASS`, `no_prepared_transactions PASS`.
- Immediately after the probe closed both sockets, the `concert-service` child exited:
  `World authority failure stage=maintenance-publication-activation error=Unavailable`,
  `World supervised failure before cleanup: World authority maintenance failed`,
  `exit_code: 1`, `unexpected_exit: true`. The production owner publication path
  (`backend/services/concert-service/src/world/production_owners.rs::activate`, reached from
  `world_runtime.rs` maintenance) aborts the supervised world when a publication window
  closes during activation instead of deferring the tick.

Raw: `task-32-in01-delta/attempt-a-world-gate-report.json`. World socket frames were
captured by the probe (two streams, 4 frames each); no browser context was opened.

## 5. Attempt B - browser gate

Because attempt A's probe teardown aborts the candidate world before the browser driver can
use the descriptor, attempt B ran the same in-tree stack driver with the probe step omitted
(a copy of `world_in01_live.py` in `/tmp`; the probe result above is not re-claimed). All
other steps are the in-tree helpers: operator policy, provisioning, profiles, concert seed,
concert child, public bootstrap, descriptor, hold.

- Stack: `result: PARTIAL`, descriptor written, held
  (`private_inputs` `/private/tmp/mcr-pg-1yizu5er/in01-live-inputs.json`); logs
  `attempt-b-stack.log`.
- Browser driver `frontend/e2e/real/world/in01/run.mjs` with Node 24:
  `independentAccounts PASS` (two contexts, two fresh logins), then `authorizedWorld FAIL`.
- App state after login: path `/world`, body text
  `EVENT AVAILABILITY COULD NOT BE CONFIRMED ... We could not obtain a current service
  decision. No World has been opened.` Only `/api/auth/capabilities`,
  `/api/auth/login` and `/api/auth/enrollment/admission` were requested; no world socket was
  opened (`observers`: active 0, peak 0, opens 0).
- Direct probe with the admitted principal's bearer on the held stack:
  `GET /api/concert/events/{event}/service-policy` returns **404** (with and without the
  closed query), while `GET /api/concert/events/{event}/bootstrap` returns 200.
  `frontend/src/shared/api/domain-operations.generated.ts` registers
  `getConcertServicePolicy` and `contracts/generated/mcr-domain.openapi.json` documents it,
  but `backend/services/api-gateway/src/adapters/in/http/concert_product/read_routes.rs`
  mounts no such route, so the shell's pre-World policy read cannot succeed at this
  candidate. The browser driver also does not define `VITE_MCR_APPLICATION_RELEASE`, which
  the same shell requires to build the policy binding; both conditions are recorded.
  Raw: `service-policy-404.txt`, `attempt-b-browser.log`, `attempt-b-inspect.log`,
  `attempt-b-driver-report.json`, `attempt-b-inspect-report.json`.

## 6. Surviving blocker and its scope

The candidate's shipped World entry requires the service-policy read, and the candidate
gateway does not serve it. Every D3/D4/D5/reconnect check happens inside the World shell, so
those deltas cannot run until the route is implemented (producer plus gateway read) and the
browser driver supplies the application release metadata. Separately, the
`concert-service` world authority maintenance abort after member teardown would kill the
world during the gate's own probe and reconnect steps; it needs product-owner resolution.

D2 additionally needs the Social policy transport configured (`CONCERT_SOCIAL_MODE=live`
with its key/claim settings); it is not composed in this stack.

## 7. Cleanup

- Both candidate stacks reported `cleanup: PASS`; the stopped browser attempt's owned
  PostgreSQL cluster was stopped and removed after its marker uid was verified.
- No candidate binary, `world_in01_live`, `dev_stack.py` or listener remains; port 5173 is
  free. The three `/private/tmp/mcr-pg-*` clusters from earlier jobs were not touched.
- Generated browser evidence under `docs/mcr/evidence/in01-world-browser/task32-*` was
  copied into this evidence directory and removed from the tracked tree.
- No consent producer, policy text or product code was fabricated. The only source change
  is the readiness validator and its regressions (`ae2092705`).

## 8. Earlier record (first attempt, before the fix and publication)

Kept for provenance: the first attempt on this candidate without the readiness fix or a
published policy recorded `dev_stack.py` failing readiness at `friend-service` and browser
signup refused with 422 `ACKNOWLEDGEMENTS_REQUIRED` / policy and rooms `503
POLICY_UNAVAILABLE`. Captures: `devstack-report.json`, `readyz-probe.json`,
`world-live-report.json`, `world-direct-report.json`, `direct-run.log`, `build-verify.log`.
Those blockers are now resolved or superseded as described above.

## 9. Product fixes and final gate (2026-11-06)

### Fixes committed on this branch

- `fbdd67823 fix(product): mount the service-policy read`: gateway read route
  `/api/concert/events/{eventId}/service-policy` in `read_routes` with `protects`, the
  closed `product_rpc::operation` classification and query validation, a revisioned
  `service_policy` producer with migration `0077_event_service_policy.sql`, and a
  policy-only product composition under `CONCERT_SERVICE_POLICY_MODE=read`.
- `cf574eadf fix(world): defer a closing publication window`: a publication window that
  closes during a maintenance tick now rolls back and defers that tick instead of
  aborting the supervised world. Reproduced as
  `maintenance-publication-activation / Unavailable` after probe teardown; an identical
  rerun passed, confirming the time-of-check race. In the final run the concert child
  stopped cleanly (`exit_code: 0`), not by abort.
- `83eb8c965 test(in01): define the application release for the browser gate`: the
  in-tree browser driver defines `VITE_MCR_APPLICATION_RELEASE` from the descriptor
  source revision and the frontend package version, which the post-acceptance pre-World
  policy requires.
- `761b53444 fix(product): compose the service-policy read with Intro/Social`: the
  correction pass. The policy surface now composes as a wrapper over whatever product
  owner the shared participant runtime already composed (Intro, Social, Report, Closure)
  instead of being skipped because `product_owner` was `Some`.
  `service_policy::compose` has unit coverage for an existing Intro/Social base, the
  opt-out, and standalone composition. The activation pool-timeout arm was extracted into
  `within_start` and is exercised by a deferral test and a genuine pool-error test. The
  `api-gateway` service-policy transport test is renamed to state that it stops at the
  unconfigured product-client guard.
- `2bb9e5e9e Merge branch 'master' into codex/mcr-exec-w5-in01`: merge of master
  `fce130fc8` (the branch was 27 behind; no rebase). The only conflict was the
  `concert-service/src/main.rs` import block; it was resolved by keeping both sides -
  master's `Future`/`Pin`/`AtomicBool`/`Ordering`/`Context`/`Poll`/`tonic`/`tower`
  imports plus the single `std::sync::Arc` the policy composition needs.
- `02d22cc5e Merge branch 'master' into codex/mcr-exec-w5-in01`: merge of the current
  master `2f2f11b61` (the row 31 and row 46 merges plus the W04 policy row). No
  conflicts; the merge brings the archive/closure transport files and their evidence.
- `6cf2c2685 fix(concert): accept the archive method in the dispatch gate expectation`:
  master `1bc74e3b8` added `rpc GetArchive` to `product.proto`, which made the dispatch
  gate test's RPC list stale. The expectation now lists all three RPCs and asserts the
  archive method's gate: `ProductArchiveService/GetArchive` is allowed for the Gateway
  workload only and is not reachable through the ProductService dispatch policy.

### Observed test counts (final tree after merging master `2f2f11b61`, `cargo +1.88.0`,
`CARGO_BUILD_JOBS=3`)

- `cargo test -p concert-service --lib service_policy`: 5 passed, 0 failed. The three
  original tests plus `the_policy_read_composes_over_an_intro_social_surface` and
  `the_policy_read_stands_alone_and_respects_the_opt_out`.
- `cargo test -p concert-service --lib visibility_pipeline_tests`: 6 passed, 0 failed,
  including `a_pool_wait_past_the_window_defers_the_tick` and
  `a_pool_failure_before_the_window_still_aborts`, plus the retained
  `a_closing_publication_window_defers_instead_of_failing_the_service`.
- `cargo test -p api-gateway --lib read_routes`: 1 passed, 0 failed.
- `cargo test -p api-gateway --test concert_product_http`: 8 passed, 0 failed (7 before
  the merge; master's `public_archive_is_unauthenticated_and_closed_without_a_producer`
  merged in).
- `cargo test -p concert-service --test product_runtime`: 6 passed, 0 failed. The
  dispatch-gate test now expects `["Dispatch", "GetArchive", "CompleteResponse"]` and
  asserts the archive method is Gateway-only under `ARCHIVE_POLICY` and unreachable
  through `POLICY`; `1bc74e3b8`'s new RPC had left the old two-name list stale and the
  suite failing.
- `cargo check -p concert-service --bin concert-service`: exit 0 after the merge
  resolution.
- `python3 scripts/mcr/contracts/test_route_parity.py`: `Ran 5 tests ... OK`.
  `python3 scripts/mcr/contracts/route-parity.py`: exit 0,
  `registry=69 contract_commands=63 templates=231 missing=0`.

### World deferral mutation

Mutating the defer arm in `within_start` from `Err(_) => Ok(None)` to
`Err(_) => Err(WorldError::Unavailable)` makes the suite fail as required:

```
test world::production_owners::visibility_pipeline_tests::a_pool_wait_past_the_window_defers_the_tick ... FAILED
test result: FAILED. 5 passed; 1 failed; 0 ignored; 0 measured; 182 filtered out
```

The file was restored from a byte copy and the baseline rerun reports
`test result: ok. 6 passed; 0 failed`. The genuine-failure assertion
(`a_pool_failure_before_the_window_still_aborts`) stays green under the mutation, so the
test distinguishes a closing window from a pool error rather than accepting both.

### Final gate run

Candidate binaries rebuilt from this branch into the retained target. Stack command:
`python3 /tmp/mcr-in01-delta/gate_run.py --bin-dir .../debug --concert-bin
.../debug/concert-service --world --hold-seconds 420`, plus the repository's isolated
operator enrollment publication and an isolated operator INSERT of one NORMAL service
policy (revision 1, `0.0.0`, 40 minute expiry) in the owned concert database.

The stack exited by hold expiry with its own `result: PARTIAL` and `cleanup: PASS`
(`gate-final2/stack.log`). `PARTIAL` is the driver's own top-level result, not an abort:
the in-tree driver does not run the browser stage (`browser_in01: NOT_RUN`) because the
browser gate is a separate driver. The driver report records `world_two_client: PASS` and
`owned_stack.result: PASS`.

Driver report (`gate-final2/driver-report.json`):

- `pre_world`: enrollment policy 200 (1 document), admission 200 `ADMITTED` with
  `acceptedRevision == policyRevision`.
- `world_two_client: PASS`.
- `world_database`: `issued_tickets_2 PASS`, `consumed_tickets_2 PASS`,
  `released_memberships_0 PASS`, `no_prepared_transactions PASS`.
- `public_c03: PASS` for both principals (avatar PUT, preferences PATCH, profile PATCH,
  stale CAS 409, unauthenticated 401, cross-principal isolation).
- `service_policy_publication`: `isolated_operator_insert` returned `INSERT 0 1`.
- `concert_stop`: `exit_code: 0`, `forced: false`.

Browser report (`gate-final2/browser-report.json`, status
`RUNTIME_PASS_PENDING_ART_REVIEW`, source `83eb8c965`): all ten checks PASS.

- independentAccounts, authorizedWorld (one socket each, one snapshot each, players 2,
  `invalid: 0`; `GET service-policy` 200 for both principals), bidirectionalMovementAndAck
  (7 inputs, acks 9/8, frames 20/16), persistentPanels (6 panels per context, same host
  and canvas, no new socket, covered movement suppressed), themeRuntimeAndLocalPersistence
  (PARADE, canvas sha changed, local identity/event scope, other context isolated,
  room/position/avatar unchanged, socket unchanged), customization (real save, peer
  avatar changed, 3 choices, real server read after reentry, clean draft),
  sustainedCurrentAuthWorld (91.6 s, 6 cycles, 2 identities, same room, one socket per
  context, 12 profile navigations, no auth injection), realBrowserReconnect (real proxy
  TCP cut, peer withdrawal, new authorised stream, fresh snapshot, both players restored,
  same canvas, peak 1 socket), singleActualPhaserGame (one live game and one owned canvas
  per context), finalWorldInvariants (2 identities, 2 live streams, 2 players per stream,
  peak 1 socket per context, 0 invalid frames, 0 private fields, 0 late errors).

Captures retained: `world-final-a.png`, `world-final-b.png`, `theme-parade.png`,
`customization-actual.png`, `customization-saved-persisted.png`.

### Delta status

| # | Delta | Status | Evidence |
| --- | --- | --- | --- |
| D1 | Admission (tickets, membership, enrollment consent) | PASS | driver probe + `world_database` rows above |
| D2 | Policy frames (`policy.baseline` / `policy.invalidate`) | NOT_RUN | `CONCERT_SOCIAL_MODE` is unset, so `social_settings` is `None` and the World transport is not composed with the Social policy baseline; no policy frame can reach any socket. Enabling it needs the social cursor/handle/command keys and claim issuer/revision, plus a probe decoder extension for the new frames. |
| D3 | Feed presentation | PASS (panel), NOT_RUN (content) | The Feed panel mounts inside the 6-panel check with the same host/canvas, no new socket and movement suppression. No `/feed` request is issued because `CONCERT_PUBLIC_FEED_MODE=off`, so authorised feed content is not asserted. |
| D4 | Theme | PASS | theme check above (canvas sha before/after, isolation, socket unchanged) |
| D5 | Panels | PASS | persistentPanels check above |
| R1 | Reconnect after socket kill | PASS | realBrowserReconnect check above |

### Limits and residual observations

- `refreshObservation` in the sustain stage is `NOT_RUN`: the deployment access-token TTL
  (900 s) exceeds the immutable runtime window, and the driver refuses token/clock
  manipulation. This is not one of the five deltas.
- The only `RUNTIME_ASSERTION` on this candidate was attempt B's `authorizedWorld` check
  (`locator('[data-world-host]')` never showed `World connected.`), recorded in
  `attempt-b-driver-report.json` before the service-policy route existed. Attempt B ran
  only `independentAccounts` (PASS) and `authorizedWorld` (FAIL); customization was not
  reached there and is not a flaky check on this candidate - it passed in the final
  browser run with a real save, peer avatar change, 3 choices and a real server read after
  reentry.
- `GET .../mural/tools` returns 403 for both principals throughout the run; the Wall
  tool surface is outside these five deltas and remains a separate defect.
- The service policy was published with an isolated operator INSERT, not the unimplemented
  admin `PUT /admin/service-policy` command; the read path, authorization and decision
  logic are real, but the operator write flow remains unbuilt.
- The generated browser evidence directory was copied into `gate-final2/` and removed
  from the tracked tree; the owned cluster and listeners were removed with the driver's
  own cleanup.
- The heavy browser gate was **not** rerun after `761b53444`. The policy composition
  change only activates when `CONCERT_SERVICE_POLICY_MODE=read` is set together with an
  already-composed product owner; the gate configuration used for the run above composes
  no other product owner (`product_owner` was `None`), so the wrapper is not installed
  then. The `within_start` extraction preserves the exact previous control flow (same
  three outcomes), and the gateway change is test-only. No runtime behaviour used by the
  browser gate changed; rerunning it would reproduce the same stack, not exercise the new
  composition branch.

## 10. Correction pass (2026-11-06)

### Corrections applied to this record

- The file previously began with the literal string `[object Promise]` and contained only
  section 9; sections 1-8 were replaced wholesale by commit `1cabdb2aa`. This revision
  restores sections 1-8 unchanged from `417c3cc3d` and adds this section.
- The gateway claim is corrected. The `api-gateway` test
  `service_policy_read_is_mounted_authenticated_and_fails_closed_without_a_producer`
  proves the route is mounted, that the unauthenticated request gets the 401 envelope, and
  that an authenticated request is admitted past route authentication and then refused
  with a fail-closed 503 at the unconfigured `concert_product_client` guard. It does not
  reach the Concert gRPC boundary; the boundary semantics are covered by the
  concert-service `service_policy` and `product_rpc` tests, not by this transport test.
- The misattributed "customization flaky" statement is removed. The `RUNTIME_ASSERTION`
  in `attempt-b-driver-report.json` belongs to `authorizedWorld` ("World connected." with
  `[data-world-host]` absent), and customization passed in the final browser run.
- The stack line is corrected. `gate-final2/stack.log` and the driver report both record
  `result: PARTIAL` with `world_two_client: PASS` and `cleanup: PASS`; the browser report
  is a separate `RUNTIME_PASS_PENDING_ART_REVIEW`. The earlier `stack: PASS` wording was
  wrong and is not retained.

### Integrator-authorized scope extension

Commit `fbdd67823` carries the service-policy vertical slice. It edits composition paths
the working agreement otherwise keeps with the delivery integrator. The integrator
declared these exact paths as this task's scope extension (task 32, branch
`codex/mcr-exec-w5-in01`), because the shipped shell performs this read before it opens
the World and the slice cannot be split without leaving the shell unable to enter:

| Path | Commit | Why it is part of this task | Ownership category |
| --- | --- | --- | --- |
| `backend/services/api-gateway/src/adapters/in/http/concert_product/read_routes.rs` | `fbdd67823` | Mounts `GET /api/concert/events/:event_id/service-policy` and classifies it in `protects`. | Gateway product route composition |
| `backend/services/concert-service/src/main.rs` | `fbdd67823`, `761b53444` | Mounts the policy producer/owner; the correction composes it over Intro/Social instead of skipping it. | Service composition (root) |
| `backend/services/concert-service/src/lib.rs` | `fbdd67823` | Exposes the new `service_policy` module. | Root composition |
| `backend/services/concert-service/src/product_rpc/mod.rs` | `fbdd67823` | Closes `getConcertServicePolicy` in the operation and query classification, and carries the query into `VerifiedProduct`. | Shared product dispatch |
| `backend/services/concert-service/migrations/0077_event_service_policy.sql` | `fbdd67823` | Adds the revisioned `concert_event_service_policy` table the producer reads. | Migration |
| `backend/services/concert-service/src/service_policy/mod.rs` | `fbdd67823`, `761b53444` | Policy producer, decision precedence, owner wrapper and composition. | Backend service code |
| `backend/services/concert-service/src/config/product_runtime.rs` | `fbdd67823` | `CONCERT_SERVICE_POLICY_MODE` parsing and the `ProductOwners` surface. | Backend config |
| `backend/services/concert-service/src/product_rpc/owner_media.rs` | `fbdd67823` | Accepts the new operation in its closed match. | Backend product dispatch |
| `backend/services/api-gateway/tests/concert_product_http.rs` | `fbdd67823`, `761b53444` | Transport proof for the mounted route. | Backend test |
| `backend/scripts/mcr/runtime_smoke.py`, `backend/scripts/mcr/test_dev_stack.py` | `ae2092705` | Readiness validator and its regressions, delegated earlier in this task. | Backend harness (integrator-owned) |
| `frontend/e2e/real/world/in01/session.mjs` | `83eb8c965` | Defines the application release the pre-World policy binding needs. | Frontend lane driver |

No central Cargo manifest, lockfile, contract source under `backend/proto/`,
`contracts/` or `generated/`, CI composition or other root router was changed by this
task. The merge commit brings master's own files; they are not claimed as this task's
writes.

### Boundary checks

The checker accepts composition/harness delegations by exact path and documentation only
under `docs/mcr/` or the checked lane, so the assigned `.omo/evidence/` file cannot be
allowed and is reported by design.

Scoped run for the correction commit (`761b53444`) in a detached worktree:

```
$ python3 scripts/mcr/check-boundaries.py --base 1cabdb2aa --lane backend \
    --allow-composition backend/services/concert-service/src/main.rs
BOUNDARIES PASS: backend, 4 changes vs 1cabdb2aa54050b59ac4e651d0660134b7077c4a; manual assignment/import review required
Explicit composition delegation: 'backend/services/concert-service/src/main.rs'; verify against the recorded assignment
```

The four changes are `main.rs`, `service_policy/mod.rs`, `world/production_owners.rs` and
`tests/concert_product_http.rs`. The same commit under `--lane frontend` reports exactly
those four backend paths as `opposite-lane write (backend)`, which is the expected mirror
of a backend task, not a frontend writer's edit.

Scoped run for the only frontend write on this branch (`83eb8c965`):

```
$ python3 scripts/mcr/check-boundaries.py --base 83eb8c965^ --lane frontend
BOUNDARIES PASS: frontend, 1 changes vs cf574eadf175b0283418ba730bf0a64801ceaef1; manual assignment/import review required
```

Full-branch run against the original base `937ab6699` exits 1. That range now also spans
the master merges and every earlier commit on this branch, so the output lists every path
changed since that base and does not isolate this task. Inside it, the paths this task
itself writes are exactly the four in the scoped backend run, the frontend driver change
above, and the assigned evidence:

- `.omo/evidence/mcr-finish-project/task-32-in01-delta.md` and the files under
  `task-32-in01-delta/`: this task's assigned evidence, outside the checker's documentation
  allowlist.
- `backend/services/concert-service/src/main.rs` and `src/lib.rs`: recorded composition
  delegation (rows above), plus the harness pair and the frontend driver, all delegated by
  the recorded scope extension.

Everything else in that range is master's own work between `937ab6699` and the merged
`fce130fc8` (for example `docs/mcr/functional-delivery-plan.md`,
`backend/proto/concert/v1/product.proto`, `backend/scripts/mcr/dev_stack.py`) or the
branch's earlier commits, and is not a write of this task.

The merged master commits (`fce130fc8`, then `2f2f11b61`) add their own files in that
range: for example `docs/mcr/functional-delivery-plan.md`,
`backend/proto/concert/v1/product.proto` and `backend/scripts/mcr/dev_stack.py` from the
first merge, and `.omo/evidence/mcr-finish-project/task-31-in04.md`,
`task-46-w04.md` and `backend/scripts/mcr/in04_wall_b_gate.py` from the second. They are
not writes of this task.
