# Task 46 — W04 policy and extension IA owner map

- Plan row: 46
- Worktree: `/Users/lhs/Desktop/mcr-worktrees/exec-w8-w04`
- Branch: `codex/mcr-exec-w8-w04`
- Base: `fce130fc8` (`master`)
- Date: 2026-11-06
- Scope: documentation and route mapping only. No product behaviour, route, contract, generated artifact or test change.
- Result: W04 policy recorded; **10/10 extension IA states mapped, 0 UNMAPPED, no URL added**. VENU-IN01 reference captures remain **NOT_RUN**.

## W04 policy record

Provenance: **user decision, 2026-11-06**, recorded in the plan row 46 note and carried into
`docs/mcr/functional-delivery-plan.md` (new dated `ROW46_W04_20261106` block) and
`docs/mcr/venue-feature-amendment-2026-11-05.md` ("W04 party-invitation policy decision — 2026-11-06").

| Policy fact | Decision |
| --- | --- |
| Invitee consent | The invited friend must accept or decline; an invitation is not admission. |
| Inviter grants | Exactly one inviter grant per party. |
| Capacity | Party capacity is 8. |
| Expiry | 24 hours from the server-issued invitation time. |
| Revocation | The host can revoke before the server deadline. |

This is product policy for the original **W04 virtual party-room invitation** only. It adds no route,
producer, capability, migration or invitation authority, does not reopen FE18's closed chooser/membership
criteria, and leaves W04 implementation `NOT_STARTED`.

**W04 party invitations remain distinct from VENU-BE-M01 venue meeting invitations, MEET HERE and
contextual MESSAGE.** A venue meeting invitation is a fixed-landmark meeting request between a selected
pair; it is not party membership, virtual-room admission or a W04 invitation grant. Conversely, a W04
party invitation is not venue authority and cannot authorize location, meeting or message access.

Dated-input caveat: `docs/mcr/contracts/paradise-city-v1/policy-freeze.md` still contains the 2026-11-05
W04 `PENDING / do not implement` section. It remains a dated record; its owner must reconcile it with this
decision before any W04 implementation is authorized. This task records the product decision, not an
implementation grant or a contract change.

## Source of the ten extension IA states

`docs/mcr/venue-feature-amendment-2026-11-05.md:50` enumerates the ten states:

1. venue overview;
2. Friends list/clusters;
3. location consent/status controls;
4. selected friend/request;
5. fixed-place invitation composer;
6. invitation decisions/status;
7. meeting conversation;
8. notes map/list;
9. note composer/audience/expiry;
10. note/replies/Meet Here.

`generated/IA_EN.json` contains **64 original screen rows** (`screens` only; no extension-state section).
It does not contain the ten extension rows. The ten-state list therefore comes from the amendment, and the
machine count `extension_ia: 10` is in `docs/mcr/task-progress-2026-11-04.json`. The separate
`docs/mcr/design/venue-redesign-2026-11-05/reference-state-map.json` supplies 48 comparison cases, not the
ten IA states. This is a planning-reference mismatch, not a product gap; no generated file was edited.

## Mapping table

All routes below are already granted. `/world/map` is the original W02 route; the two invitation routes are
the grants recorded by `41d90738e`. The ten states are panel/view states, not new URLs.

| # | Extension IA state | Implementing screen / component | Granted route (reachability) | State owner | Tests that exercise the state | Capture status |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | Venue overview | `ParadiseCityMap` MAP tab and `MapCanvas` siteplan (`frontend/src/features/paradise-city-map/ui/ParadiseCityMap.tsx:73`, `ui/MapCanvas.tsx:62`). Mounted by `ParadiseCityMapPanel` (`frontend/src/app/venue-entry/ParadiseCityMapPanel.tsx`). | `/world/map` (`frontend/src/app/routes.ts:25`; `frontend/src/app/router.tsx:41`). | `frontend/src/features/paradise-city-map` (`model/controller.ts`, `spatial/viewport.ts`, `ui/*`). | `consumer/view.test.tsx` "implements keyboard local tabs and preserves one map without a mutation"; `spatial/viewport.test.ts` pan/zoom/fit cases; `app/__tests__/venue-host-integration/mounting.test.tsx` "loads the verified illustrated Map through the injected venue ports and offers local tabs" and "retains one public image and local Map tab state across real Outlet navigation and back"; `app/__tests__/venue-shell-ports/production.test.ts` "projects real Location and reviewed-landmark responses through the production ports". | Map shell appears in task-25 captures at 360/390/430 (`SUBSTITUTED_FIXTURE`); no dedicated overview case. VENU-IN01 comparison **NOT_RUN**. |
| 2 | Friends list/clusters | FRIENDS tab and cluster markers (`ParadiseCityMap.tsx:111-123`; `MapCanvas.tsx:86-88`). | `/world/map` → FRIENDS. | `frontend/src/features/paradise-city-map` (`controller.ts` roster/selection; `model/location-adapter.ts`). | `view.test.tsx` "clusters real checked-in friends and lists exactly those actors", "shows a failed friend gate as unavailable instead of an empty friends list"; `consumer/controller.test.ts` "never labels an aged snapshot as current and keeps only its last-known markers", "refetches on demand, recovers to a fresh snapshot and never duplicates actors"; `location-adapter.test.ts` "keeps a stale offline snapshot only as last-known and recovers without duplicate actors"; `production.test.ts` "names friends only from the actor snapshot and an authorized public profile", "mounts a truthful unavailable panel when the friend gate refuses with 503". | Task-25 `clustered-friends-{360,390,430}.png`, `unavailable-503-*`; report `PASS / SUBSTITUTED_FIXTURE`. VENU-IN01 comparison **NOT_RUN**. |
| 3 | Location consent/status controls | Sharing footer, consent sheet, revoke confirmation, permission/stale states (`ParadiseCityMap.tsx:135-145`). | `/world/map` (shell footer). | `frontend/src/features/paradise-city-map` (`controller.ts` sharing methods; `venue-entry/browser-location.ts`). | `view.test.tsx` "never starts phone location from render or an unchecked sharing prompt", "reports device permission denial without starting another watch and allows a reviewed retry", "requires explicit confirmation before revoke and then removes sharing", "labels a stale snapshot as last-known and offers a refetch"; `controller.test.ts` consent/pause/background/refusal/revoke cases; `browser-location.test.ts` six cases; `location-adapter.test.ts` "requires explicit current viewer consent and never defaults to all friends", "sends contract-valid empty audiences for PAUSED and REVOKED and re-reads authority". | Task-25 `permission-denied-*`, `paused-*`, `revoked-*`, `stale-*`, `stale-recovered-*` at 360/390/430; `PASS / SUBSTITUTED_FIXTURE`. VENU-IN01 comparison **NOT_RUN**. |
| 4 | Selected friend/request | Friend sheet with `ASK LOCATION` and the Location requests section (`ParadiseCityMap.tsx:96-102`, `:118-122`). | `/world/map` → FRIENDS / map marker. | `frontend/src/features/paradise-city-map` (`controller.select`, `requestLocation`, `decideLocationRequest`, `model/controller.ts:229,282-289`). | `view.test.tsx` "opens current friend details, sends only a contextual open and restores focus on Escape", "scrubs selected private DOM in the same withdrawal notification", "enables MESSAGE without location or a landmark while MEET HERE stays disabled"; `consumer/location-requests.test.tsx` all five cases (incoming approval, denial/cancel, expiry/departed, unknown key, privacy withdrawal); `controller.test.ts` "requesting a friend location never grants sharing consent"; `location-adapter.test.ts` "decides only current pending incoming/outgoing requests with the authoritative revision". | No dedicated selected-friend/request capture in task-25. VENU-IN01 comparison **NOT_RUN**. |
| 5 | Fixed-place invitation composer | Friend/landmark/note `MEET HERE` entry plus the invitation surface (`ParadiseCityMap.tsx:46,64,99,107-109`; `features/venue-invitations/Invitation.tsx:21-48`). | Composer panel on `/world/map`; the opened invitation is also served on `/meet/invitations/:invitationId` (`routes.ts:38`; `router.tsx:38`). | `frontend/src/features/paradise-city-map` for the entry/selection; `frontend/src/features/venue-invitations` + `frontend/src/app/venue-community-entry` for the invitation surface and command. | `controller.test.ts` "requires a reviewed fixed landmark for INVITE", "associates a reviewed landmark with MESSAGE without creating or sending anything", "rejects a bare-user or unauthorized MESSAGE even with a reviewed landmark"; `view.test.tsx` "enables MESSAGE without location or a landmark while MEET HERE stays disabled"; `e2e/venue-community/owner.test.ts` "retains safe draft across Map close/reopen and invite/conversation navigation" and "uses current server actions for invitation commands and original versions" (create path); `routes.test.ts` grant assertions. | Task-27 `meet-here-{360,390,430}.png` shows the opened invitation from a note; no dedicated composer capture. VENU-IN01 comparison **NOT_RUN**. |
| 6 | Invitation decisions/status | `Invitation` detail and `InvitationInbox` (`Invitation.tsx:31-46`; `Inbox.tsx:38-121`). | `/meet/invitations/:invitationId` (`routes.ts:38`; `router.tsx:38`). The inbox is an in-panel state of that route; there is no `/meet/invitations` URL (`routes.test.ts:20-26`). | `frontend/src/features/venue-invitations` (`model.ts`, `Invitation.tsx`, `Inbox.tsx`) + `frontend/src/app/venue-community-entry` (`owner.ts`, `adapter.ts`, `routes.tsx`). | `e2e/venue-community/inbox.test.tsx:44-147` (list, terminal status, accept/decline, full, denial, unavailable, fail-closed, detail, conversation separation); `owner.test.ts:251-329` (inbox read, accept, decline/cancel, expired, denial, host revoke, unknown, privacy scrub); `adapter.test.ts:103-147` (revision/CAS, terminal status, capacity, directions); `ui.test.tsx:33` (explicit accept updates the view). | Task-26 `docs/mcr/evidence/task-26-invitations/` list/accept/decline/expired/full/denied at 360/390/430; `inbox-native` `PASS / SUBSTITUTED_FIXTURE`. VENU-IN01 comparison **NOT_RUN**. |
| 7 | Meeting conversation | `MeetingConversation` (`frontend/src/features/meeting-conversation/Conversation.tsx:13-53`). | `/meet/invitations/:invitationId/conversation` (`routes.ts:39`; `router.tsx:39`); the MESSAGE panel is also reachable on `/world/map` via the friend sheet. | `frontend/src/features/meeting-conversation` + `frontend/src/app/venue-community-entry` (`owner.ts` surface/commands). | `e2e/venue-community/ui.test.tsx:15-68` (composer, no fake landmark, unknown send, privacy scrub, provider absence); `inbox.test.tsx:131-150` (conversation with no invitation; invitation id is not permission); `owner.test.ts:14-80,110-167,331-341` (read-only entry, no-landmark MESSAGE, server capabilities, no send inference); `adapter.test.ts:50-99` (normalized context, paging, gaps, dedupe, command identity). | Native27 / `docs/mcr/evidence/venue-community/visual-qualified-27.json` conversation at 360/390/430 (`PASS`, controlled changed regions); task-26 component captures. VENU-IN01 comparison **NOT_RUN**. |
| 8 | Notes map/list | NOTES tab, note clusters and note rows (`ParadiseCityMap.tsx:123-134`; `MapCanvas.tsx:94-96`). | `/world/map` → NOTES. | `frontend/src/features/paradise-city-map` (`controller.ts:136,148`; `ui/*`) + `frontend/src/app/venue-notes-adapter` (`index.ts:78,254`) + `frontend/src/shared/api/paradise-city-v1/notes.ts`. | `view.test.tsx` "renders note markers and cluster rows from server counts even when no page rows are loaded", "loads the selected place when its cluster marker is chosen", "shows the server visible count rather than the loaded page length"; `controller.test.ts` "loads server note counts only when the Notes tab is explicitly opened", "loads a selected place through the real notes read port", "re-reads the place when its disclosure lease ends instead of reporting no notes"; `venue-notes-adapter/__tests__/adapter.test.ts` server-count clusters, empty authorized page, place total vs page length, denied place omission. | Task-27 `clusters-*` and `selected-*` at 360/390/430; `PASS / SUBSTITUTED_FIXTURE`. VENU-IN01 comparison **NOT_RUN**. |
| 9 | Note composer/audience/expiry | `pc-composer`: place, text, `Visible to`, `Expires after`, submit (`ParadiseCityMap.tsx:88-95`). | `/world/map` → NOTES. | `frontend/src/features/paradise-city-map` (`controller.compose/edit/setAudience/setExpiry/submit`, `controller.ts:233`) + `frontend/src/app/venue-notes-adapter` (`postNote`/`reply`, `index.ts:300-322`). | `view.test.tsx` "authors a place note with black audience/expiry selectors and sends one explicit intent", "limits composer audience and expiry to current server rights"; `controller.test.ts` "keeps an unknown note outcome and original key locked until status-only recovery", "reply uses only its parent ID and body, without an independent audience or expiry", "removes a reply draft when the parent expires or permission is withdrawn", "reloads the producer page and summary after a note is applied"; `adapter.test.ts` "rejects an audience or expiry outside the contract before any write", "keeps one-hour independently available when server day-end policy is unavailable", "requires an explicit original UUID and never resends a retained uncertain command". | Task-27 `composer-{360,390,430}.png` and `created-*`; `PASS / SUBSTITUTED_FIXTURE`. VENU-IN01 comparison **NOT_RUN**. |
| 10 | Note/replies/Meet Here | `noteCard`: reply author rows, `REPLY`, `MEET HERE`, `WITHDRAW`, `READ REPLIES` (`ParadiseCityMap.tsx:56-67`). | `/world/map` → NOTES (note card). `MEET HERE` opens the meeting panel on the same route. | `frontend/src/features/paradise-city-map` (note card/reply/withdraw/meeting entry) + `frontend/src/app/venue-notes-adapter` (thread/withdrawal) + `frontend/src/app/venue-community-entry` (`openMeeting`/`viewOnMap`). | `view.test.tsx` "offers withdrawal only on the current author note and sends its exact revision", "renders reply author identity from the producer reply row", "withdraws an expired note from the rendered list at the server clock boundary"; `controller.test.ts` "withdraws only an author-authorized note with its exact server revision", "reloads the exact reply thread after a reply is applied", "renders a truthful unavailable notice when the note read is denied"; `adapter.test.ts` "binds replies to the actual parent revision and sends no audience/expiry override", "projects reply authors from producer rows rather than the root author", "binds withdrawal to the exact server revision and idempotency key"; `owner.test.ts` "uses the sole scoped focus port before closing and Root-owned navigation" (Meet Here focus path). | Task-27 `reply-thread-*`, `meet-here-*`, `withdrawn-*`, `expired-*`, `denied-*`, `unreviewed-*` at 360/390/430; `PASS / SUBSTITUTED_FIXTURE`. VENU-IN01 comparison **NOT_RUN**. |

## Route and reachability proof

- `frontend/src/app/routes.ts:25` builds `/world/map` as an original IA-derived panel route; lines 38-39
  register only the two granted invitation routes.
- `frontend/src/app/router.tsx:41` binds `/world/map` to `ParadiseCityMapPanel`; lines 38-39 bind the two
  invitation paths to `VenueInvitationPanel` / `VenueConversationPanel`.
- `frontend/src/app/routes.ts:20-25` and `routes.test.ts:20-26` assert the exact route set: required IA
  paths plus exactly the two granted invitation routes, and no `/meet/invitations` parent URL. The route
  test also rejects optional IA paths. No route was added or redefined by this task.
- `frontend/src/app/WorldShell.tsx:227` mounts `VenueRootFeatures` around the shell outlet;
  `venue-root/VenueRootFeatures.tsx` provides `VenueCommunityFeatures` and `ParadiseCityVenueProvider`.
  That is the single shell composition for the map panels and the invitation leaf.
- The invitee/decision panel is an in-panel state of `/meet/invitations/:invitationId`; its "All
  invitations" control opens the inbox and "Back to invitation" returns. No list URL exists.
- The map states are tab/sheet states inside `/world/map`; none needs a new path.

**What is not proven here.** There is no VENU-IN01 extension-IA capture or verdict in this worktree:
`.omo/evidence/mcr-finish-project/` has no task-28 file, and the current plan records **10 extension IA
and 48 comparisons NOT_RUN**. The browser evidence that exists is scoped component evidence from rows
25–27 (`SUBSTITUTED_FIXTURE`, actual production components/adapters, no live backend, device GPS or
physical venue). The route-level integration tests are jsdom tests, not default-app 360/390/430 browser
captures. This evidence therefore proves code registration, reachable panel states and test coverage, not
default-app browser reachability or visual acceptance.

## UNMAPPED

**None.** Each of the ten states has (a) a concrete implementing component, (b) an already-granted route
or a panel state inside one, (c) a named client feature owner, and (d) named tests. Nothing was assigned a
new URL to make the mapping work.

Mapping is not acceptance. The VENU-IN01 extension-state captures and verdicts remain **NOT_RUN**, so the
ten states remain unaccepted for visual/reference purposes even though they are owned and routed.

## Audit notes

- `generated/IA_EN.json` has 64 original screen rows and no extension rows; the ten states live in the
  amendment and the `extension_ia: 10` machine count. Do not infer an IA route from the extension list.
- The existing venue invitation inbox renders "Party invitation policy" copy from
  `frontend/src/features/venue-invitations/model.ts:51` (`W04_INVITATION_POLICY`) in
  `Inbox.tsx:61-68`. That is presentation copy only. It does not implement W04, does not make the
  VENU-BE-M01 meeting invitation a party invitation, and does not grant W04 authority. Product copy
  separation, if desired, is a separate UI change and was not made here.
- `docs/mcr/contracts/paradise-city-v1/policy-freeze.md` still records W04 as pending/unfrozen. Its owner
  must reconcile it with the 2026-11-06 decision before implementation. This task does not edit the
  contract registry.

## Files changed

- `docs/mcr/functional-delivery-plan.md` — new dated `ROW46_W04_20261106` current block.
- `docs/mcr/venue-feature-amendment-2026-11-05.md` — new W04 policy-decision section.
- `.omo/evidence/mcr-finish-project/task-46-w04.md` — this record.

## Checks

| Check | Result |
| --- | --- |
| `python3 scripts/mcr/check-plan.py` | `PASS` (planning provenance/structure only; application build/tests/deployment `NOT_RUN`). |
| `git diff --check` | `PASS`. |
| Targeted frontend tests | Not rerun in this docs-only task; no code changed and `frontend/node_modules` is absent in this worktree. The named cases above are the existing row 25–27 suites and route tests; their prior counts are in the row 25–27 evidence. |
| VENU-IN01 captures | **NOT_RUN**; no extension-IA capture exists to cite. |
