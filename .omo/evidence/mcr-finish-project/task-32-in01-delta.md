[object Promise]

## 9. Product fixes and final gate (2026-11-06)

### Fixes committed on this branch

- `fbdd67823 fix(product): mount the service-policy read`: gateway read route
  `/api/concert/events/{eventId}/service-policy` in `read_routes` with `protects`, the
  closed `product_rpc::operation` classification and query validation, a revisioned
  `service_policy` producer with migration `0077_event_service_policy.sql`, and a
  policy-only product composition under `CONCERT_SERVICE_POLICY_MODE=read`. The gateway
  test `service_policy_read_is_mounted_authenticated_and_reaches_the_concert_boundary`
  proves the unauthenticated 401 envelope and the authenticated request reaching the
  Concert boundary.
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

### Observed test counts

- concert-service lib `service_policy`: 3 passed.
- api-gateway lib `read_routes`: 1 passed.
- api-gateway `concert_product_http` service-policy test: 1 passed.
- `scripts/mcr/contracts/test_route_parity.py`: 5 passed. `route-parity.py`: exit 0
  (registry=69 contract_commands=63 templates=230 missing=0).
- `check-boundaries.py --lane backend` and `--lane frontend`: exit 1 with lane-mixing
  findings only (this task intentionally spans backend composition and the frontend gate
  driver); no import or dependency violation was reported.

### Final gate run

Candidate binaries rebuilt from this branch into the retained target. Stack command:
`python3 /tmp/mcr-in01-delta/gate_run.py --bin-dir .../debug --concert-bin
.../debug/concert-service --world --hold-seconds 420`, plus the repository's isolated
operator enrollment publication and an isolated operator INSERT of one NORMAL service
policy (revision 1, `0.0.0`, 40 minute expiry) in the owned concert database. The stack
exited by hold expiry with `stack: PASS`, `cleanup: PASS`.

Driver report (`gate-final2/driver-report.json`):

- `pre_world`: enrollment policy 200 (1 document), admission 200 `ADMITTED` with
  `acceptedRevision == policyRevision`.
- `world_two_client: PASS`.
- `world_database`: `issued_tickets_2 PASS`, `consumed_tickets_2 PASS`,
  `released_memberships_0 PASS`, `no_prepared_transactions PASS`.
- `public_c03: PASS` for both principals (avatar PUT, preferences PATCH, profile PATCH,
  stale CAS 409, unauthenticated 401, cross-principal isolation).
- `service_policy_publication`: `isolated_operator_insert` returned `INSERT 0 1`.

Browser report (`gate-final2/browser-report.json`, status
`RUNTIME_PASS_PENDING_ART_REVIEW`), all ten checks PASS:

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
- Customization is flaky across identical runs: one run recorded
  `RUNTIME_ASSERTION` and the final run recorded a full PASS; both are retained above.
- `GET .../mural/tools` returns 403 for both principals throughout the run; the Wall
  tool surface is outside these five deltas and remains a separate defect.
- The service policy was published with an isolated operator INSERT, not the unimplemented
  admin `PUT /admin/service-policy` command; the read path, authorization and decision
  logic are real, but the operator write flow remains unbuilt.
- The generated browser evidence directory was copied into `gate-final2/` and removed
  from the tracked tree; the owned cluster and listeners were removed with the driver's
  own cleanup.
