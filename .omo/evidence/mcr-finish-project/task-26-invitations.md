# Task 26 — complete the invitation and conversation client

- Plan row: 26 (`frontend/src/features/venue-invitations/`,
  `frontend/src/features/meeting-conversation/`, `frontend/src/app/venue-community-entry/`)
- Worktree: `/Users/lhs/Desktop/mcr-worktrees/exec-w4-inv`
- Branch: `codex/mcr-exec-w4-inv`
- Base: `cb44bb77cb35fd0efc0760033e45432579e29d71` (row 24 shell ports)
- Feature commit: `c6185025a3db3fd31296b2270c90f98303ea74df`
- Fix commit: `c30978adcdcbe0fc1ca48c1f578c6867f0f4be17`
  (`fix(venue): keep the inbox on a granted route and correct the failure record`)
- Evidence commit: `ee8bb3021`
- Scope: frontend only; no backend, wire protocol, lockfile, CI or unrelated composition change.
  No new route grant and no `routeGrants` record were introduced.

## Route authority

The only route additions are the two granted by `41d90738e` and recorded in
`docs/mcr/venue/community/outcome.md`:

- `/meet/invitations/:invitationId`
- `/meet/invitations/:invitationId/conversation`

The incoming/outgoing invitation inbox is an **in-panel state of the granted detail route**. The
detail header's "All invitations" control opens it; "Back to invitation" returns to the detail. There
is no `/meet/invitations` parent URL. `docs/mcr/venue/community/source-handoff.json` remains without
`routeGrants`. `frontend/src/app/__tests__/routes.test.ts` asserts exactly the two granted paths and
that `/meet/invitations` is absent. `frontend/src/app/router.tsx` and `routes.ts` are identical to
the base.

## Terminal producer states

`invitationExpired` applies only to Pending. Accepted, Declined and Cancelled invitations past their
original `expiresAt` render the producer status and keep producer-granted terminal actions.
`adapter.viewInvitation`, `owner.armExpiry`, `owner.decideInbox` and `owner.execute` apply expiry only
to Pending rows. Adversarial tests that failed on the old behavior:

- `e2e/venue-community/inbox.test.tsx` — terminal Accepted/Declined/Cancelled past deadline keep
  `data-display-status`; only Pending renders Expired.
- `e2e/venue-community/adapter.test.ts` — terminal Accepted past deadline keeps `status` and
  producer-granted `allowedActions`.
- `e2e/venue-community/owner.test.ts` — terminal Accepted past deadline still executes a
  producer-granted Cancel.

## Corrected pre-existing failure record

The scratch-master failure record at `6cb31ca70` names exactly three, all pre-existing and out of
scope:

1. `src/shared/api/__tests__/c07-social-client/compatibility.test.ts` —
   "pins the exact public cohort without claiming all indexed fixtures are qualified" (C07
   manifest pin; untouched `frontend/src/shared/api/domain-operations.generated.ts`).
2. `src/app/operations-composition/__tests__/composition.test.tsx` —
   "aborts pending settings and ignores its late completion after canonical policy replacement".
3. `src/app/operations-composition/__tests__/reports-production/composition.test.tsx` —
   "aborts a pending list read and rejects its late completion across policy change" (the `list`
   parameterized case).

The two sharing-safety tests previously misrecorded in this evidence are **not** failures. They pass
on scratch master; the full sharing-safety file is 9/9 on this checkout.

### Scratch-master commands

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

Observed on this host:

- Scratch-master full suite: 5713 passed, 14 skipped, 1 failed (C07).
- Combined operations/social files: 54/54 passed.
- Sharing-safety file alone: 9/9 passed.
- The operations-composition two are therefore recorded from the supplied scratch-master
  full-suite review as order/timing-dependent; they remain the named pre-existing failures and are
  out of scope. No C07 pin, operations-composition, social-composition or generated contract was
  modified.

## Implemented surface

| Requirement | Implementation |
| --- | --- |
| Incoming/outgoing invitation lists | `InvitationInbox`, `Inbox.tsx`; in-panel state of `/meet/invitations/:invitationId`; no new URL. |
| Real frozen invitation client | `createNormalizedMeetingAdapter` exposes `listInvitations` on the `MeetingLeafAdapter` seam, built from `meetingRequests.listInvitations`. |
| Accept/decline/cancel | Row actions render only producer `allowedInvitationActions`. `owner.decideInbox` opens the current detail (GET), re-checks current actions/revision, then executes through the existing CAS/key path. |
| Cancel rights + truthful denial | Outgoing cancel is shown only when the producer grants `CANCEL`. The adapter maps denial/conflict/unavailable results truthfully; the owner keeps the producer status and surfaces the denial. |
| Event-scoped conversation | The authorized map-friend MESSAGE path opens a conversation with no invitation or landmark. No bare-user DM route exists. Messaging comes only from server `canSend`; an invitation id is never permission. |
| No write on open | Opening the detail and then the in-panel inbox issues only GET reads. Tests assert `writes === 0`; the browser report records `openWrites: 0` at all three widths. |
| States | Pending, Accepted, Declined, Cancelled, Expired, full and denied are rendered. Expiry applies only to Pending; terminal states retain the producer status. |
| W04 policy surfaced | Invitee consent, one inviter grant per party, capacity 8, 24-hour expiry and host revoke are rendered as policy copy. The client does not enforce them locally. |
| Capacity 8 full | Rendered only when the adapter supplies an authoritative capacity. VENU-BE-M01 has no party-capacity field, so the production adapter returns `null`; the full capture is a controlled fixture. |

## Files

- `frontend/src/features/venue-invitations/Inbox.tsx`
- `frontend/src/features/venue-invitations/model.ts`
- `frontend/src/features/venue-invitations/Invitation.tsx`
- `frontend/src/features/venue-invitations/meeting.css`
- `frontend/src/features/meeting-conversation/Conversation.tsx`
- `frontend/src/app/venue-community-entry/{owner,adapter,ports,routes,index,view}.ts(x)`
- `frontend/src/app/__tests__/routes.test.ts`
- `frontend/e2e/venue-community/{fixture,owner.test,adapter.test,ui.test,inbox.test,inbox-preview,inbox-native}`
- `docs/mcr/venue/community/{outcome.md,source-handoff.json}`
- `docs/mcr/evidence/task-26-invitations/` (README, captures, report)

## Verification (Node 24.22.2 / pnpm 11.7.0)

| Command | Exit | Result |
| --- | --- | --- |
| `corepack pnpm@11.7.0 install --frozen-lockfile --offline` | 0 | 199 packages reused |
| `corepack pnpm@11.7.0 run typecheck` | 0 | clean |
| `corepack pnpm@11.7.0 exec vitest run` (10 targeted files) | 0 | 10 files, 239 tests passed |
| `corepack pnpm@11.7.0 run test` (full) | 1 | 5715 passed, 14 skipped, 1 failed (C07) in this run; the scratch-master record names all three pre-existing failures above |
| `corepack pnpm@11.7.0 run lint` | 0 | no changed-file warnings |
| `corepack pnpm@11.7.0 run build` | 0 | compile + Vite build |
| `node e2e/venue-community/inbox-native.mjs` | 0 | PASS at 360/390/430; `sourceCommit c30978adc` |

## Captures

`docs/mcr/evidence/task-26-invitations/inbox-native.json` records the controlled fixture clock,
Chrome version, component bounds, zero open writes, action write counts, no overflow, 44px control
assertions and `sourceCommit c30978adcdcbe0fc1ca48c1f578c6867f0f4be17`. Captures at 360/390/430
CSS pixels, DPR 1:

| State | Captures |
| --- | --- |
| List | `list-{360,390,430}.png` (includes terminal past-deadline Accepted/Declined/Cancelled) |
| Accept | `accept-{360,390,430}.png` |
| Decline | `decline-{360,390,430}.png` |
| Expired | `expired-{360,390,430}.png` |
| Full (controlled capacity) | `full-{360,390,430}.png` |
| Denied host revoke | `denied-{360,390,430}.png` |

Developer inspection: terminal Accepted/Declined/Cancelled rows show their producer status and
"Expiry no longer applies."; the Pending past-deadline row shows Expired; the full and denial
banners are legible at all widths. No decorative left accent.

## Limitations and risks

- The M01 producer exists, but its production bilateral friend authority is not composed; the live
  service fails closed with 503. These captures are controlled-component evidence, not production
  shell/HTTP/two-principal/device acceptance.
- M01 carries no party-capacity field. The production adapter reports no capacity rather than
  guessing; the full state is only reachable through an adapter that supplies capacity.
- W04 is distinct from VENU-M01. The policy is surfaced as copy; no W04 route, producer or
  capability was added.
- The frozen client refuses an inbox page containing an expired PENDING invitation. The expired
  capture uses the producer's EXPIRED state; the client derives an expired display from a producer
  deadline only for a Pending row.
- The inbox is an in-panel state of the granted detail route. A parent list URL would be a Root
  route-grant ownership exception; this task does not self-grant one.
- Three pre-existing full-suite failures are named above and out of scope; none was fixed or
  suppressed.

## Cleanup

- Vite and Chromium closed by the harness; no background process remains.
- Install used the shared offline store; `node_modules` is worktree-local.
- No backend, database, lockfile, CI or unrelated composition file was modified.
