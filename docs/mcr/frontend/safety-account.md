# FE36 safety and account controls

Status: IN_PROGRESS. Route-ready frontend code is isolated from root composition.
No live endpoint, multiuser, provider, device, or IN03 acceptance is claimed.

Base: `892ca4de7580eb36c82b54efe44f03941ca68c41` on `codex/mcr-fe-FE36`.
Ownership: `frontend/src/features/safety/**`, `frontend/src/features/account/**`, this document.

## Safety slice

New UI and headless controllers use C04 operations `createConcertReport`,
`listOwnConcertReports`, `listConcertBlocks`, `blockConcertUser`,
`unblockConcertUser`, `getConcertCommandStatus`. All requests use the injected
canonical SessionController, its memory credentials, QueryClient and privacy coordinator.
No feature-owned fetch, router, token cache, backend import or moderator detail exists.

`ReportDialog` accepts the exact report query target type and ID. Its injected
`ReportAuthorizationPort` is a local owner seam, not a new HTTP endpoint. The owner
must obtain fresh target access or qualified recent evidence and actor-owned READY
REPORT media authority. An absent port closes reporting. Local checkboxes and
participant/bootstrap visibility never authorize submission. The server rechecks all
invariants. The closed ordinary Report schema rejects privileged extensions.

Unknown writes preserve the original actor/event/operation/key/target binding in
memory and permit manual status reads only. Success receipts require a current
protected own-report/block reread. Missing resources, partial pages, 404, expired or
malformed receipts never authorize replay or a replacement key. Block/unblock
success invalidates canonical private caches, drafts and observers before navigation
callbacks. Unblock never revalidates consent. Root owners must recreate controllers
under the new privacy scope after a restrictive action.

Native dialogs provide modal focus containment and restoration, Escape/cancel,
explicit destructive confirmations and symmetric borders without left accent lines.
Evidence choices are already-sanitized local owner projections, not raw media or
moderator metadata. Root must dispose the controller when its owner lifetime ends;
back/navigation does not replay any mutation. No persistent browser storage is used.

## Evidence so far

- Focused safety/controller suite: 32 tests passed, exit 0 (initial slice).
- Typecheck: passed before tests; the test fixture type correction and subsequent
  account work require another combined check before final handoff.
- Native widths 360/390/430: NOT_RUN yet.
- Live providers/root/IN03: NOT_RUN.

## Account composition gap

Frozen inherited Auth defines `deleteAccount` (DELETE `/api/auth/account`, 202) and
`getAccountDeletionStatus` (GET `/api/auth/account/deletion`, 200). Domain C04 has no
registered replacement deletion operation. SessionController.request prohibits
`/api/auth/` paths. Delivery was informed; Operations confirms BE42 has no assigned
qualified producer/adapter. Account UI must use an injected canonical Auth-owner
port or show deletion unavailable. No alternate fetch/session or invented endpoint
is permitted. Account implementation/verification is still in progress.
