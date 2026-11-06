# Task 32 - IN01 delta re-run for changed features

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
