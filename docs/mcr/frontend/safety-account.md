# FE36 safety and account controls

Status: MOCK_READY. Assigned frontend implementation, deterministic transport/owner
fixtures, privacy regression and native browser validation are delivered. Integration
review/publication belongs to Delivery. Root composition, qualified live Auth/deletion,
BE41 lifecycle, devices, QA04/QA07 and IN03 remain NOT_RUN by this worker. None of these
external checks is a prerequisite for completing this assigned client source scope.

Base: `892ca4de7580eb36c82b54efe44f03941ca68c41`.
Worktree: `/Users/lhs/Desktop/mcr-worktrees/fe-safety-account`.
Branch: `codex/mcr-fe-FE36`. No worker push.
Ownership: `frontend/src/features/safety/**`, `frontend/src/features/account/**`, this document.

## Behavior and composition

| IA / owner action | Export / behavior |
| --- | --- |
| `/report?targetType=:type&targetId=:id` | `ReportDialog` with `createSafetyController`; closed typed targets, reasons, optional text/evidence, explicit availability/authorization and safe receipt status |
| `/me/blocks` | `BlocksPanel`; protected current paginated block list and explicit unblock confirmation |
| Other owner user surfaces | `BlockUserButton`; explicit confirmation, canonical privacy invalidation and affected-panel callback |
| `/me/account` | `AccountPanel` with `createAccountController`; canonical logout, exact deletion confirmation, unavailable/pending/partial/unknown progress |
| `/account/deleted` | Root-owned public pending/completed view using the retained account controller's sanitized progress; `onDeletionRestricted` and `onDeletionComplete` are navigation seams |

These are route-ready exports, not router edits or integrated IA claims. Roots must
provide their one canonical SessionController and preserve its one QueryClient,
Phaser lifetime and world connection. Create private-route safety controllers under
the current lease and dispose them at their actual owner lifetime boundary. Keep an
unresolved command owner alive across dismiss/reopen so the original key remains
available; dismissal scrubs report drafts without minting another key. There is no
browser persistence or automatic request replay.

Root composition must keep the deletion owner alive outside the private WorldShell
when routing to the public pending/deleted view. `onDeletionRestricted` runs only
after canonical cleanup and safe pending/unknown state publication, once. Do not
recreate this owner under an unrelated actor or restamp an old scope as current.
All external identity/event/policy changes scrub account confirmation/progress and
abort its port work. Only the controller's own deliberate restriction may rebind its
minimal deletion recovery lease to the exact next suspended privacy generation.
It never reopens the privacy gate or restores consent.

### Reporting and command reconciliation

Safety requests use `createConcertReport`, `listOwnConcertReports`,
`listConcertBlocks`, `blockConcertUser`, `unblockConcertUser`, and
`getConcertCommandStatus` through canonical protected no-store transport. Reports
support USER/POST/COMMENT/PARTY/GIVEAWAY/MURAL, the five frozen reasons, at most 2000
Unicode characters and four distinct evidence IDs. Closed ordinary Report schemas
reject moderator-only extensions; no ReportDetail is imported or retained.

`ReportAuthorizationPort` is an injected local owner projection, not a new endpoint.
The owner must obtain fresh current target access OR qualified recent target evidence,
and actor-owned event-scoped READY REPORT media authority. Missing/expired/denied
proof closes the target. Submission rechecks evidence immediately before dispatch.
Local checkboxes, UUIDs, participant visibility or bootstrap are never authority.
The server independently enforces all guards, reporter privacy, duplication and quota.
BE36 has confirmed this design but has not qualified a live public route.

`blocksAvailable` must explicitly be true from a qualified owner; absence is unavailable.
A block and an unblock both invalidate all canonical private queries, active observers,
drafts and feature subscriptions before navigation callbacks. Unblock never
revalidates prior consent. The list exposes only contract-permitted target IDs and
block state; it does not invent display-name or profile authority.

Every mutation keeps its original actor/event/operation/idempotency key/typed target
and known resource binding. PENDING 202 is never success. SUCCEEDED must carry the
registered 200 result and a current authorized resource reread with adequate revision.
Created reports reconcile through the actor's own report list, never moderator detail.
Incomplete block pages cannot prove absence. Unknown results, 404, terminal failure,
retirement, expiry, changed bindings and unavailable current resources allow manual
bounded status reads only. They never authorize replay or a replacement key.

### Account deletion and logout

No domain deletion operation is registered by frozen C04. The inherited Auth operations
are `deleteAccount` (DELETE `/api/auth/account`, 202 DeleteAccountResponse) and
`getAccountDeletionStatus` (GET `/api/auth/account/deletion`, 200 UserDeletionSaga).
There is no declared DELETE body or idempotency key. The typed confirmation is a local
interaction guard and is not sent as an invented wire DTO. No domain receipt is substituted.

SessionController.request rejects `/api/auth/`, so `AccountDeletionPort` must be supplied
by Delivery's canonical Auth owner. Until supplied, deletion is disabled/unavailable.
Its exact interface is in `features/account/controller.ts`: `request(scope, signal)`
returns `{scope,status:202,data:unknown}`, and `status(scope, signal)` returns
`{scope,status:200,data:unknown}`. That adapter owns canonical transport, original
actor/generation checks, browser intent, no-store, inherited Auth errors and no DELETE
replay. It must return the captured scope without changing canonical identity/policy
before consumer validation. Status-only recovery must be authorized for that same
identity's deliberately suspended lease; this grants no ordinary private-read access.
Raw response/error objects and credentials never enter UI state.

Request exactly once. An uncertain DELETE may already have executed, so it also
closes private views and permits only manual GET recovery. A verified accepted or
recovered saga suspends canonical privacy and synchronously clears all private owners.
`deleted: true` alone never means cleanup completed. Saga ID/user identity must match,
updatedAt cannot go backwards or into the future, and completion requires the four
frozen required targets each exactly once, SUCCEEDED with completed timestamps, plus
consistent COMPLETED saga metadata. Partial/dead-lettered/unknown remains visible.
Verified completion uses canonical logout to revoke local identity before the public
completion callback. Logout always calls the existing canonical controller; local
revocation is immediate and server confirmation requires its 204 result. Late callbacks
cannot navigate or sign out a new actor.

BE42 is now assigned in its separate checkout. It proposes CONCERT_TICKETS,
CONCERT_SESSIONS, CONCERT_SHARING and CONCERT_PRIVATE_DATA step targets. These are NOT
in this base's frozen inherited enum. Delivery/BE42 were informed directly that a
reviewed canonical contract revision and full required-step inventory are needed;
unapproved extensions deliberately fail as unknown, never false completion. This is
an explicit integration compatibility item, not a feature-owned contract amendment.

## Validation

Toolchain: Node 24.22.2 / pnpm 11.7.0; installed offline with retained frozen lockfile.
Commands below ran from the worktree root unless noted, with the pinned Node directory
on PATH and pnpm invoked using Node on `/Users/lhs/Desktop/mcr-tools/pnpm-11.7.0/package/bin/pnpm.cjs`.

| Command / check | Result / exit |
| --- | --- |
| `pnpm --dir frontend test src/features/safety src/features/account src/features/auth src/shared/query` | 367 tests / 13 files PASS, exit 0; includes 63 new FE36 tests |
| `pnpm --dir frontend build` (TypeScript build then Vite) | PASS, exit 0; existing large-chunk warning remains outside this patch |
| `pnpm --dir frontend lint` | PASS, exit 0 |
| From `frontend/`: `node src/features/safety/__tests__/browser-smoke.mjs` | Native Chrome 360/390/430 PASS, 24 canonical synthetic HTTP requests, no page errors, exit 0 |
| `python3 scripts/mcr/check-plan.py` | PASS, exit 0; source archive checksums preserved, no planning status regeneration |
| `python3 scripts/mcr/check-boundaries.py --base 892ca4de7580eb36c82b54efe44f03941ca68c41 --lane frontend --allow-doc docs/mcr/frontend/safety-account.md` | PASS, exit 0; every changed path manually matches assignment |
| `node scripts/mcr/check-ui-origin.mjs` | PASS, exit 0; no old rendering imports or reviewed extracts added |
| `git diff --check` | PASS, exit 0 |

Native evidence: `/tmp/mcr-fe36-evidence/result.json` and `report-{360,390,430}.png`,
`account-{360,390,430}.png`. The JSON records source hashes, exact synthetic requests,
widths and assertions. These are reproducible temporary artifacts, not committed secrets
or live service receipts. Screenshots at 360 and 390 were manually inspected: readable
wrapping, scrolling, ordinary symmetric borders, no horizontal overflow or left accents.

Native checks cover real modal focus containment (including explicit Tab wrap), initial
safe focus, Escape/focus restoration, report typed text/evidence and pending receipt
reread, denied deep links, block/unblock cleanup, absent deletion adapter, pending and
partial failure, all-step synthetic completion, single deletion request, late identity
fence and unconfirmed logout. Unit tests also exercise event/suspension/revocation,
unknown/expired/foreign/malformed receipt, current-resource absence, no key replacement
after dismissal, account saga validation, privacy-generation mismatch, synchronous
observer revocation before dispatch, and an old controller's inability to log out a new actor.
Fixtures are explicit test-only owner/transport seams. They do not certify server
transactionality, media readiness, authorization, multiuser propagation, real provider
cookies, lifecycle cleanup, account retention policy or release readiness.

## Provenance and delivered paths

All UI and controllers are newly authored. No Bigyeting rendering/headless code,
legacy character art, new visual library or third-party implementation was copied.
Dependencies are retained React/Zod and the existing canonical Auth/privacy APIs.
No manifest, lockfile, source archive, central router, contract registry, backend,
world runtime or artwork changed. The inherited Auth schema provenance remains
upstream `704f42149d5e8e35e1d578676d8ae2dac4f0f9c2`, as recorded in inherited-map.

Contract/owner SHA256 pins at the base:

| Source | SHA256 |
| --- | --- |
| `contracts/domain/v1/operations.json` | `42ed48ee0cad846b94f9f7a5d39a38beeeb5f5b41e0129f157e51ff31c635223` |
| `contracts/domain/v1/policy.json` | `508d22af964b718cdcbd5a25b32b9a1e3f7b651a5b8c25c87d56f1f6b68d0ebe` |
| `contracts/domain/v1/common.schema.json` | `303f3019611c636679282d6c29106deab83d7c0c9717bd8b9ba526d2ff865a35` |
| `contracts/domain/v1/admin.schema.json` | `b8a4f8b91440d561bbcf23dc68bc196919a8893cf744ca09ac05ecaabff72cb8` |
| `contracts/domain/v1/feed.schema.json` | `9c8d991df0bb707d7a7a2d43d4b132e7fadb878a52633e1c1f338afc9407f8ed` |
| `contracts/generated/mcr-domain.openapi.json` | `c47e767505e660d8f5eb69bb9759eae1f0aaee6472e4fb963edc82e060385f51` |
| `contracts/domain/v1/inherited-map.json` | `2d7510d94d45efd3f77a5586800ab7a353326288bfebb3900157b80225329751` |
| `frontend/src/features/auth/model/session.ts` | `dae6680d1f4a0600b6d3012639fd82c9317d63d1a55eb856e5c4dc81f9a5dc53` |
| `frontend/src/shared/query/privacy.ts` | `9f39e9e25b1c322bbe4b2f19a52d930aa369940757e01611ed632aa11cf2f081` |

Delivered paths: safety contract/controller/index/panels/Dialog/CSS; safety fixtures,
controller tests and native preview/smoke; account contract/controller/index/panel and
controller tests; this document. Initial safety commit `009d833` is a direct child of
the recorded base. The account/native/review-fix slice and final handoff ancestry are
recorded below after committing. Only Delivery may publish reviewed work to master.
