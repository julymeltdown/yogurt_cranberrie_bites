# Task 26 — invitation and conversation client

- Base: `cb44bb77cb35fd0efc0760033e45432579e29d71` (row 24 shell ports)
- Branch: `codex/mcr-exec-w4-inv`
- Status: implementation and developer-owned validation complete on the task branch; not merged or pushed.

## Delivered surface

- `GET /api/concert/events/{eventId}/venue-meeting/invitations` is consumed through the frozen
  `meetingRequests.listInvitations` descriptor for incoming and outgoing pages.
- `/meet/invitations` renders both directions with producer-supplied Pending, Accepted, Declined,
  Cancelled and Expired statuses. Accept/Decline/Cancel appear only when the producer's
  `allowedInvitationActions` grants them.
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
- Expiry uses the producer's `expiresAt` and the qualified clock. The client does not fabricate a
  timestamp. The frozen client refuses an expired PENDING inbox page; an EXPIRED producer row is
  rendered as `Expired after 24 hours`.

## Verification (Node 24.22.2, pnpm 11.7.0)

| Command | Exit | Result |
| --- | --- | --- |
| `corepack pnpm@11.7.0 install --frozen-lockfile --offline` | 0 | 199 packages reused from the shared store |
| `corepack pnpm@11.7.0 run typecheck` | 0 | `tsc -b` clean |
| `corepack pnpm@11.7.0 exec vitest run` (10 targeted files) | 0 | 235 passed |
| `corepack pnpm@11.7.0 run test` (full suite) | 1 | 5711 passed, 14 skipped, 1 pre-existing failure in `c07-social-client/compatibility.test.ts` (pinned hash of untouched `domain-operations.generated.ts`) |
| `corepack pnpm@11.7.0 run lint` | 0 | no warnings in changed files; pre-existing warnings remain |
| `corepack pnpm@11.7.0 run build` | 0 | Vite build complete |
| `node e2e/venue-community/inbox-native.mjs` | 0 | 3 widths, 0 page errors, open writes 0 |

The new tests prove:

- Opening the inbox, invitation detail and conversation issues only GET reads (`writes === 0`).
- Accept, decline and cancel use current producer actions/revisions.
- A denied host revoke surfaces `denied` and leaves the accepted producer status unchanged.
- A denied inbox read fails closed with no private rows.
- Expired actions are not issued.
- An accepted invitation id with `canSend: false` renders no composer and issues no write.
- The sole authorized-friend MESSAGE path opens with no invitation and no write.

## Captures

`inbox-native.json` records the controlled fixture clock, browser version, exact component bounds,
zero write counts on open, per-action write counts, overflow and 44px target assertions. Captures are
at 360, 390 and 430 CSS pixels, DPR 1:

- `list-{360,390,430}.png`
- `accept-{360,390,430}.png`
- `decline-{360,390,430}.png`
- `expired-{360,390,430}.png`
- `full-{360,390,430}.png`
- `denied-{360,390,430}.png`

The developer inspected the list, accept, decline, expired, full and denied states. The accepted
incoming row loses Cancel; the accepted outgoing row keeps the producer-granted Cancel; the denied
revoke keeps `Accepted`; the full banner is informational and does not hide producer actions.

## Limitations

- The M01 producer is present, but its production bilateral friend authority is still uncomposed;
  the live service returns 503 until that owner exists. These captures are controlled-component
  evidence, not production-shell, HTTP, two-principal or device acceptance.
- VENU-BE-M01 has no party capacity field. The client renders a full state only when the adapter
  supplies capacity; the production adapter returns `null` (no invention). The full capture uses a
  controlled fixture and does not claim live capacity enforcement.
- The W04 policy is distinct from M01. It is surfaced as policy copy only; no W04 route, producer or
  capability was added.
- The new `/meet/invitations` route is an authorized extension-IA panel addition and is covered by
  the route composition test. No new wire protocol was added.
