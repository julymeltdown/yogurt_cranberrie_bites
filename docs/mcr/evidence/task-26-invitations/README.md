# Task 26 — invitation and conversation client

- Base: `cb44bb77cb35fd0efc0760033e45432579e29d71` (row 24 shell ports)
- Feature commit: `c6185025a3db3fd31296b2270c90f98303ea74df`
- Branch: `codex/mcr-exec-w4-inv`
- Status: implementation and developer-owned validation complete on the task branch; not merged or pushed.

## Delivered surface

- `GET /api/concert/events/{eventId}/venue-meeting/invitations` is consumed through the frozen
  `meetingRequests.listInvitations` descriptor for incoming and outgoing pages.
- The inbox is an **in-panel state of the granted detail route**
  `/meet/invitations/:invitationId`. The "All invitations" control opens it; "Back to invitation"
  returns to the detail. No parent list URL exists.
- The only route additions remain the two granted routes from `41d90738e`:
  `/meet/invitations/:invitationId` and `/meet/invitations/:invitationId/conversation`. The route
  test asserts exactly those two and rejects `/meet/invitations`.
- Accept/Decline/Cancel appear only when the producer's `allowedInvitationActions` grants them.
  Row actions first read the current invitation detail (GET only), then execute the exact
  revision/key/selection through the frozen client. A refused action keeps the producer status and
  surfaces the sanitized denial reason.
- Expiry applies only to a Pending invitation past its producer deadline. Accepted, Declined and
  Cancelled invitations always render the producer status and keep producer-granted terminal
  actions even when the original deadline has passed.
- The authorized map-friend MESSAGE path opens an event-scoped conversation with no invitation or
  landmark requirement. No bare-user DM route exists. Messaging capability comes only from the
  server's `canSend`; an invitation id is never permission.
- The approved W04 policy is surfaced as copy: invitee consent, one inviter grant per party,
  capacity 8, 24-hour expiry, host revoke. The client does not enforce it locally.

## Verification (Node 24.22.2, pnpm 11.7.0)

| Command | Exit | Result |
| --- | --- | --- |
| `corepack pnpm@11.7.0 install --frozen-lockfile --offline` | 0 | 199 packages reused from the shared store |
| `corepack pnpm@11.7.0 run typecheck` | 0 | `tsc -b` clean |
| `corepack pnpm@11.7.0 exec vitest run` (10 targeted files) | 0 | 10 files, 239 tests passed |
| `corepack pnpm@11.7.0 run test` (full suite) | 1 | 5715 passed, 14 skipped; the pre-existing failures are named below |
| `corepack pnpm@11.7.0 run lint` | 0 | no warnings in changed files; pre-existing warnings remain |
| `corepack pnpm@11.7.0 run build` | 0 | Vite build complete |
| `node e2e/venue-community/inbox-native.mjs` | 0 | PASS at 360/390/430; the fix source commit is recorded in `inbox-native.json` |

### Pre-existing full-suite failures (out of scope)

The scratch-master failure record at `6cb31ca70` names exactly these three; none is caused by this
task and none was fixed or suppressed:

1. `src/shared/api/__tests__/c07-social-client/compatibility.test.ts` —
   "pins the exact public cohort without claiming all indexed fixtures are qualified" (C07
   manifest pin; `frontend/src/shared/api/domain-operations.generated.ts` hash differs from the
   retained pin).
2. `src/app/operations-composition/__tests__/composition.test.tsx` —
   "aborts pending settings and ignores its late completion after canonical policy replacement".
3. `src/app/operations-composition/__tests__/reports-production/composition.test.tsx` —
   "aborts a pending list read and rejects its late completion across policy change" (the `list`
   parameterized case).

The two sharing-safety tests previously named in this README are **not** in the failure record.
They pass on scratch master; the full sharing-safety file is 9/9 on this checkout.

Scratch-master commands used to check the record:

```
git worktree add --detach /tmp/mcr-scratch-6cb31ca70 6cb31ca70
ln -s /Users/lhs/Desktop/mcr-worktrees/exec-w4-inv/frontend/node_modules \
  /tmp/mcr-scratch-6cb31ca70/frontend/node_modules
cd /tmp/mcr-scratch-6cb31ca70/frontend
node /Users/lhs/Desktop/mcr-worktrees/exec-w4-inv/frontend/node_modules/vitest/vitest.mjs run
node /Users/lhs/Desktop/mcr-worktrees/exec-w4-inv/frontend/node_modules/vitest/vitest.mjs run \
  src/app/operations-composition/__tests__/composition.test.tsx \
  src/app/operations-composition/__tests__/reports-production/composition.test.tsx \
  src/app/social-composition/__tests__/sharing-safety-default/entry.test.tsx
```

Observed on this host: the scratch-master full suite was 5713 passed, 14 skipped, 1 failed (C07);
the combined operations/social file run was 54/54, and the sharing-safety file alone was 9/9. The
operations-composition failures are therefore recorded from the supplied scratch-master full-suite
review as order/timing-dependent; they are named, reproduced in the record context, and kept out of
scope. No operations-composition, social-composition, C07 pin or generated contract was modified.

The new tests prove:

- Opening the granted detail route and then the in-panel inbox issues only GET reads
  (`writes === 0`).
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
source commit. Captures are at 360, 390 and 430 CSS pixels, DPR 1:

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
  capture uses the producer's EXPIRED state; the client derives an expired display from a producer
  deadline only for a Pending row.
- The inbox is an in-panel state of the granted detail route. If a parent list URL becomes a
  product requirement, it is an ownership exception for the Root route-grant owner; this task does
  not self-grant it.
