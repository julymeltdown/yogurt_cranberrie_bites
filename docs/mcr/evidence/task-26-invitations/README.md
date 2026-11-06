# Task 26 — invitation and conversation client

- Base: `cb44bb77cb35fd0efc0760033e45432579e29d71` (row 24 shell ports)
- Feature commit: `c6185025a3db3fd31296b2270c90f98303ea74df`
- Fix and route-grant commit: `2cf67de0977f319967817b9894d15897433b681d`
- Branch: `codex/mcr-exec-w4-inv`
- Status: implementation and developer-owned validation complete on the task branch; not merged or pushed.

## Delivered surface

- `GET /api/concert/events/{eventId}/venue-meeting/invitations` is consumed through the frozen
  `meetingRequests.listInvitations` descriptor for incoming and outgoing pages.
- `/meet/invitations` renders both directions with producer-supplied Pending, Accepted, Declined,
  Cancelled and Expired statuses. Accept/Decline/Cancel appear only when the producer's
  `allowedInvitationActions` grants them.
- Expiry applies only to a Pending invitation past its producer deadline. Accepted, Declined and
  Cancelled invitations always render the producer status and keep producer-granted terminal
  actions even when the original deadline has passed. No terminal state is relabeled Expired.
- Row actions first read the current invitation detail (GET only), then execute the exact
  revision/key/selection through the frozen client. A refused action keeps the producer status and
  surfaces the sanitized denial reason instead of inventing a transition.
- `/meet/invitations/:invitationId` and `/meet/invitations/:invitationId/conversation` remain the
  detail and invitation-linked conversation handles. The authorized map-friend MESSAGE path opens
  an event-scoped conversation with no invitation or landmark requirement. No bare-user DM route
  exists. Messaging capability comes only from the server's `canSend`; an invitation id is never
  permission.
- The approved W04 policy is surfaced as copy: invitee consent, one inviter grant per party,
  capacity 8, 24-hour expiry, host revoke. The client does not enforce it locally.

## Route grant

`docs/mcr/venue/community/outcome.md#route-grant-provenance` records the invitation leaf route
grant under plan row 26 and the venue amendment's extension IA state
`invitation decisions/status`. The machine-readable grant is `routeGrants` in
`docs/mcr/venue/community/source-handoff.json`:

- `/meet/invitations`
- `/meet/invitations/:invitationId`
- `/meet/invitations/:invitationId/conversation`

`frontend/src/app/__tests__/routes.test.ts` reads that record and asserts its authority/path list;
it no longer keeps a private allowlist. `/meet/invitations` is a panel view state, not a bare-user
DM entry and not a new wire protocol.

## Verification (Node 24.22.2, pnpm 11.7.0)

| Command | Exit | Result |
| --- | --- | --- |
| `corepack pnpm@11.7.0 install --frozen-lockfile --offline` | 0 | 199 packages reused from the shared store |
| `corepack pnpm@11.7.0 run typecheck` | 0 | `tsc -b` clean |
| `corepack pnpm@11.7.0 exec vitest run` (10 targeted files) | 0 | 10 files, 239 tests passed |
| `corepack pnpm@11.7.0 run test` (full suite) | 1 | 5715 passed, 14 skipped. Three pre-existing failures are named below; this run reproduced C07 and the two sharing-safety tests passed in isolation. |
| `corepack pnpm@11.7.0 run lint` | 0 | no warnings in changed files; pre-existing warnings remain |
| `corepack pnpm@11.7.0 run build` | 0 | Vite build complete |
| `node e2e/venue-community/inbox-native.mjs` | 0 | PASS at 360/390/430; `sourceCommit` `2cf67de0977f319967817b9894d15897433b681d` |

### Pre-existing full-suite failures (out of scope)

These three failures are not caused by this task and were not fixed or suppressed:

1. `src/shared/api/__tests__/c07-social-client/compatibility.test.ts` —
   "pins the exact public cohort without claiming all indexed fixtures are qualified" (C07
   manifest pin; `frontend/src/shared/api/domain-operations.generated.ts` hash differs from the
   retained pin).
2. `src/app/social-composition/__tests__/sharing-safety-default/entry.test.tsx` —
   "mounts actual USER report and blocks routes, rechecks target on submit, and retains the
   original report key across navigation" (composition: duplicate "Blocked users" headings).
3. `src/app/social-composition/__tests__/sharing-safety-default/entry.test.tsx` —
   "requires Profile block confirmation, clears old privacy views, lists and confirms unblock
   without restoring consent" (timing under the full-suite run).

The final full run here reproduced (1); (2) and (3) passed in isolation and in repeated full runs
on this host, so they remain documented as pre-existing composition/timing failures from the
supplied review rather than claimed as fixed. No social-composition code, C07 pin or generated
contract was modified.

The new tests prove:

- Opening the inbox, invitation detail and conversation issues only GET reads (`writes === 0`).
- Accept, decline and cancel use current producer actions/revisions.
- A terminal Accepted/Declined/Cancelled invitation past its deadline keeps the producer status
  (`data-display-status`) and granted actions; only a Pending past-deadline row renders Expired.
- A denied host revoke surfaces `denied` and leaves the accepted producer status unchanged.
- A denied inbox read fails closed with no private rows.
- An accepted invitation id with `canSend: false` renders no composer and issues no write.
- The sole authorized-friend MESSAGE path opens with no invitation and no write.

## Captures

`inbox-native.json` records the controlled fixture clock, browser version, exact component bounds,
zero write counts on open, per-action write counts, overflow, 44px target assertions and the real
source commit `2cf67de0977f319967817b9894d15897433b681d`. Captures are at 360, 390 and 430 CSS
pixels, DPR 1:

- `list-{360,390,430}.png` — includes Accepted, Declined and Cancelled rows whose original
  `expiresAt` is in the past; they retain their producer status and show "Expiry no longer applies."
- `accept-{360,390,430}.png`
- `decline-{360,390,430}.png`
- `expired-{360,390,430}.png`
- `full-{360,390,430}.png`
- `denied-{360,390,430}.png`

The developer inspected the list, accept, decline, expired, full and denied states. The selected
tab, terminal producer states, pending expiry, full banner and denied host revoke are legible at all
widths. No decorative left accent is used.

## Limitations

- The M01 producer is present, but its production bilateral friend authority is still uncomposed;
  the live service returns 503 until that owner exists. These captures are controlled-component
  evidence, not production-shell, HTTP, two-principal or device acceptance.
- VENU-BE-M01 has no party capacity field. The client renders a full state only when the adapter
  supplies capacity; the production adapter returns `null` (no invention). The full capture uses a
  controlled fixture and does not claim live capacity enforcement.
- The W04 policy is distinct from M01. It is surfaced as policy copy only; no W04 route, producer or
  capability was added.
- The frozen client refuses an inbox page containing an expired PENDING invitation. The expired
  capture uses the producer's EXPIRED state; the client also derives an expired display from a
  producer deadline only for a Pending row.
- The new `/meet/invitations` route is recorded by the route grant above; it is not a bare-user DM
  entry and adds no wire protocol.
