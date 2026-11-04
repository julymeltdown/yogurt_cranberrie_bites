# FE24 — current client completion

Original FE24 pure-client criteria: **SATISFIED**. Qualification status: **MOCK_READY** against frozen contracts and synthetic HTTP/C05 in the actual default application. The current [FE21/FE24 delivery record](fe21-fe24-client-delivery.md) records the final criterion matrix, source pins, failures/fixes, 148 focused checks, three native touch journeys, and limits. Publication is handled by Root153 with fresh owner remote verification; no King/SL review stage applies.

The historical source/validation record below is preserved. Its initial review queues, uncomposed-root statements and earlier goal states are superseded by the current record. The default root now composes the feature; server/IN02 and physical-device qualification remain separate.


## Historical initial delivery

Status: **MOCK_READY**, pending independent review and parent disposition. The
unbudgeted FE24 goal remains **ACTIVE** through review fixes. This does not accept
the complete project, real giveaway service, FE25 authoring, or FE26 pickup.

Assignment base: `5f884edfe636a3a9a8f8b08fb33044c5d57cd0aa`, isolated branch
`codex/mcr-fe-FE24`, checkout `/Users/lhs/Desktop/mcr-worktrees/fe-gift-discovery`.
Only `frontend/src/features/gift-discovery/**`,
`frontend/src/features/my-gifts/**`, and this document are delivered. No push.
The delivery commit and exact changed-path/hash inventory are sealed in the
feature's evidence receipt after the implementation commit, without a circular
self-hash claim.

## Contract and provenance

- Product authority: `MCR_IMPLEMENTATION_PLAN_V2_EN.md` FE24 and G01/G03/G07.
  Assessment and execution documents do not reduce that scope.
- Frozen consumer decision: `docs/mcr/party-gift-consumer-contract.md` revision1.
  Exact operations/reachable schemas and browser bundle at `ca9f955`; shared
  request prerequisites `2645b6f`, contextual qualification `ca9f955`/`88fd8f3`
  are ancestors of the assignment base. Whole C04 remains a review candidate.
- Only `listConcertGiveaways`, `getConcertGiveaway`, and
  `getConcertGiftActivity` are used. Existing canonical FE09 `auth.request`
  supplies credentials, refresh/expiry policy and typed domain error handling.
  Every call binds `errorFamily: 'domain'` and its exact `domainOperation`.
- `model/contract.ts` compiles the reachable closed schemas directly from
  `contracts/generated/mcr-domain.openapi.json` into bounded Zod checks. There
  is no independent endpoint registry or permissive response fallback. Unknown
  schema keywords/types fail closed. Typed feature projections follow those
  checked wire values. The JSON source is recorded by hash in native evidence.
- Product UI, styles, controllers and tests are newly authored. No Bigyeting
  rendering, legacy art, or new dependencies were copied. Existing internal
  Auth/FE02/FE07 APIs are consumed at the assignment base. The native runner uses
  the established internal Vite/Chrome harness approach from
  `frontend/src/features/places/__tests__/browser-smoke.mjs` at that base; no old
  product rendering is reused.
- Synthetic test data adapts
  `contracts/fixtures/domain/giveaway-management.json` at the assignment base;
  avatar configuration is metadata only and is never rendered as art. The
  browser fixture supplies synthetic identities, labels, stock and fetch
  responses. Its world lease is explicitly instrumented, with no real game or
  socket. `docs/source/`, all contracts, manifests and lockfiles are unchanged.

## Behavior

`GiftDiscovery` offers status, remaining-stock, public-place and pickup-start
time filters. Local date inputs explicitly use KST; wire bounds are UTC, lower
inclusive and upper exclusive. The controller rejects invalid/equal/reversed
bounds before dispatch. Filters reset the list cursor. Detail reads independently
of whether the target appears on the currently loaded list page.

The list/detail display authoritative available/total stock, owner public name,
pickup window and OPEN/PAUSED/SOLD OUT/ENDED/COMPLETED/CANCELLED presentation.
`CLOSED` is displayed as Completed per the frozen transition decision. Pickup
start/end and reservation-expiry presentation use a local clock with one bounded
next-boundary timer, and never assert server execution or real arrival. The
server remains authoritative about clock, stock, authorization and transitions.

Public DTOs contain neither recipient identities nor private pickup instructions,
codes or exact meetings. DRAFT is rejected in public list/detail and allowed only
in giving rows whose owner matches the captured actor. Draft rows have no public
detail link. No media ID is converted into an invented image URL, and no avatar
fallback artwork is rendered. Public place references navigate only to the
existing FE19 reference route; they do not certify venue provenance or arrival.
Optional filter labels come from a separately qualified injected public catalog.

`MyGifts` supplies GIVING and RECEIVING with independent cursors, page numbers,
previous-page histories and explicit retries. The endpoint always returns both
collections; advancing one sends both current cursor positions, validates the
whole response, and commits only the selected collection. Empty pages with a
continuation remain navigable. No continuation is auto-followed. Each browse
session is capped at twenty pages (at most 2,000 remembered IDs per collection).
Filter/scope resets release that history. Cursor cycles, duplicate IDs and
forward pagination-order regressions are rejected.

Reservation eligibility retains all seven C03 decisions and reasons. It checks
gift/mutation denial, event phase, ownership, state, stock, local end time and
positive active-reservation evidence. Missing, partial or terminal pages cannot
prove nonmembership or eligibility. The read contract provides no authoritative
per-target eligibility grant, so otherwise eligible-looking gifts explicitly
remain unconfirmed. FE24 performs no command and never enables a fake Reserve,
Claim, Cancel or Manage action. FE25/FE26 must qualify their own routes and current
server authority before enabling those journeys.

## Privacy and lifetime

The feature requires the canonical Auth controller's `session`, `request` and
`privacy`. Every request runs through `privacy.run` using the captured FE02
privacy lease and a feature-owned cancellation signal. Private state stays in
the scoped ephemeral controller; it is not persisted, dehydrated, placed under
an unrelated query key, or sent through a second QueryClient/token store.
No central cache-key extension is needed.

`registerCleanup` synchronously aborts pending work, clears all list/detail/own
data, filters, selection, timers, cursor and duplicate histories, and withdraws
capability context on account/event/logout/block/consent/suspension/closure
changes. Late abort-resistant results cannot commit. A fresh context must carry
the current privacy generation and all seven newly qualified decisions; merely
reusing the old bootstrap after a policy change is rejected. Refreshed bearer
versions do not create another feature identity.

At most one flight each exists for list, detail and activity. A response must
match captured actor/event/generation, HTTP200, exact requested target, closed
schema, page limits, unique IDs, requested filters, counts, timestamps, projection
rules and own identities before commit. Claim receipts must agree with terminal
reservation state. Failures display fixed local categories, never server bodies.
Rate limits use the registered delay and require manual retry. Disposal is
idempotent, clears timers and subscriptions, and is safe during reentrant loading
publication.

## Integrator composition prerequisite

The feature exports `createGiftDiscoveryController`, `GiftDiscovery`, `GiftDetail`
and `MyGifts`; it does not mount another world, own app routes or edit the root.
The parent must:

1. Create one controller for the active shell, using the existing Auth controller
   and a `GiftContextPort`. Its `read/subscribe` publishes validated bootstrap
   capabilities/event phase bound to the **current** `auth.privacy.capture()`.
   Withdraw it while authorization is unknown; requalify after policy changes.
2. Inject `GiftDiscovery` for `/gifts`, `GiftDetail` with the actual route parameter
   for `/gifts/:giveawayId`, and `MyGifts` for `/gifts/mine`, through FE07's
   existing `createAppRoutes(..., { panels })` seam. Dispose the controller when
   its shell owner exits. Components start the shared controller idempotently;
   they do not dispose a shared owner during panel navigation.
3. Optionally inject already qualified `{ id, label }` place options. No place
   catalog, asset or media-read operation is invented in this task.
4. Keep FE25/FE26 commands unavailable until those independent increments and
   their server authorization/recovery contracts are qualified.

No shared API, Auth, FE02, root router, manifest or lockfile change is requested.
Default production composition still requires the parent-owned panel injection.

## Verification and evidence

Evidence root: `frontend/src/features/gift-discovery/__tests__/evidence/`.
Commands ran in the assigned checkout using Node24.22.2 and pnpm11.7.0 from
`/Users/lhs/Desktop/mcr-tools/`. Package execution prefix was
`PATH=/Users/lhs/Desktop/mcr-tools/node-v24.22.2-darwin-arm64/bin:$PATH`
followed by that Node binary, the pnpm11.7.0 `package/bin/pnpm.cjs`, and
`--dir /Users/lhs/Desktop/mcr-worktrees/fe-gift-discovery/frontend`.

| Command / artifact | Exit / result |
|---|---|
| `install --offline --frozen-lockfile` | 0; retained resolutions, no dependency edits |
| `test src/features/gift-discovery/__tests__` / `focused-fourth.log.gz` | 0; 60 tests in three files |
| `build` / `build-final.log.gz` | 0; strict TypeScript + Vite; existing627.99kB main chunk warning |
| `lint` / `lint-first.log` | 0; entire current src and Vite config |
| Node `frontend/src/features/gift-discovery/__tests__/browser-smoke.mjs`, `MCR_FE24_EVIDENCE` set to new absolute `.../evidence/browser-final` | 0; native Chrome156.0.8078.80, 390×844 and1280×900 |
| `python3 scripts/mcr/check-plan.py` / `plan.log` | 0; planning guard only |
| Node `scripts/mcr/check-ui-origin.mjs --root <assigned checkout>` / `origin.log` | 0; no violations |
| `python3 scripts/mcr/check-boundaries.py --base 5f884edfe636a3a9a8f8b08fb33044c5d57cd0aa --lane frontend --allow-doc docs/mcr/frontend/gift-discovery.md` / `boundaries.log` | 0; working/index boundary check |
| `git diff --cached --check` | 0 |

Focused coverage includes public privacy/closed schemas, captured-context binding,
all filters, exact bounds, Unicode lengths, duplicates/order/page limits, own
owner/recipient and draft identity, independent and empty continuation, unknown
eligibility, all capability-denial reasons, stale scope/filter results, cooldowns,
reentrant disposal and next-boundary timer cleanup. The own-activity component
tests live under the discovery test directory but exercise `MyGifts` directly.

Native `browser-final/report.json` records all181 source/contract hashes, actual
browser version, widths and seven screenshots. Both journeys exercise filter
query conversion, list/detail/back focus, own draft and independently paged
GIVING/RECEIVING, disabled reservation, and a single persistent instrumented shell
owner. The390px journey additionally exercises empty/error/403/404/429/private
projection rejection, rate-limit re-enabling, abort-resistant late completion
after policy revocation and account+event replacement. Peak resource owners=1;
disposal leaves0 resources and0 context listeners. No second game/socket is
created by these features; real Phaser/WebSocket ownership is not tested here.

All seven final screenshots were inspected. Their bytes match the individually
inspected `browser-third` images: compact text-led panels, readable stock/status,
ordinary symmetric controls, no decorative left accents or unapproved art, no
horizontal overflow, and feature touch targets at least44×44. Long content
scrolls inside the existing shell panel. The native report's181 input hashes
were independently compared to current bytes with zero mismatches.

Raw build/test logs containing terminal whitespace are losslessly gzip-archived
(`*.log.gz`); decompression reproduces their exact bytes. This keeps raw evidence
intact without adding terminal-generated whitespace to the source patch.
Historical evidence is preserved: first/second unit runs failed during schema
compiler construction (default metadata and boolean handling), third passed51;
fourth added adversarial coverage and passed60. Initial direct typecheck found an
ES2022 string API mismatch, which was corrected without changing TypeScript
configuration; `typecheck-first.log` records the subsequent pass. Browser first
found ambiguous select labelling, fixed with explicit accessible names; second
found a test selector matching both an option and visible stock status, fixed by
scoping to the status element. Third and final browser journeys pass. The first
build passed before final pagination hardening; the final build covers it.

No broad unchanged suite was rerun after final focused/build/lint success. Backend,
database races, real multiuser pickup/reservation, device and release gates are
**NOT_RUN**. Native deterministic fetch evidence is not backend integration or
full feature/project acceptance.
