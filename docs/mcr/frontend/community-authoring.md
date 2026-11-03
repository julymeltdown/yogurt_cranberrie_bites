# Community authoring delivery

## FE22 bounded handoff — 2026-11-04

Status: **MOCK_READY**, not INTEGRATED or ACCEPTED. Worktree:
`/Users/lhs/Desktop/mcr-worktrees/fe-community-authoring`, branch
`codex/mcr-fe-community-authoring`, source base
`f0e44bd1bba943b02319f9d9a42433e253e8de6f`.

All implementation is newly authored. No Bigyeting rendering, source archive,
character original, dependency resolution, backend, shared transport, or root
composition was changed. Runtime dependencies are retained React, RHF, Zod and
resolver packages. Existing canonical Auth/FE02 and FE21 public response decoder
are imported directly; no duplicate identity/client or copied upstream module.

Contract: C04 candidate at source base, `contracts/domain/v1/{party,common}.schema.json`,
`operations.json`, `policy.json`, and generated `mcr-domain.openapi.json`.
Operations: `createConcertParty`, `updateConcertParty`, `getConcertParty`,
`getConcertCommandStatus`. This extends the consumer surface beyond the earlier
nine-operation decision using the explicitly assigned candidate; it does not
claim a whole-C04 freeze or production endpoint availability.

| Pinned input | SHA256 |
|---|---|
| party.schema.json | 7cbea6af623668a36d641e7498bf52f4869b9bc44594b46e7f4a3806cc907d03 |
| operations.json | 42ed48ee0cad846b94f9f7a5d39a38beeeb5f5b41e0129f157e51ff31c635223 |
| policy.json | 508d22af964b718cdcbd5a25b32b9a1e3f7b651a5b8c25c87d56f1f6b68d0ebe |
| mcr-domain.openapi.json | c47e767505e660d8f5eb69bb9759eae1f0aaee6472e4fb963edc82e060385f51 |

`party-authoring/index.ts` exports `createPartyAuthoring`, `PartyAuthoring`,
`PartyDraft`, and typed options for the delivery owner to compose at M02 and the
M04 edit surface. Supply the shell's canonical auth controller, scoped capability
port, an optional party ID for editing, and a live qualified-place projection.
No free-text place ID, invented venue description, or physical-arrival action is
provided. Missing qualified places disables publishing. Capability availability
is only a UI prerequisite; server owner/policy checks remain authoritative.

The RHF form validates Unicode text limits, capacity 2–50, strict UTC calendar
values, ordered time windows and place selection. Requests are additionally
checked against the pinned closed schema. Edit capacity cannot undercut the
latest known membership count. Editing loads the current party and verifies
owner identity. Version conflicts keep the draft and require a server reread
and explicit draft/revision confirmation. Cancelled or unauthorized owner
responses clear the form and private data.

Drafts are memory-only for the controller lifetime, isolated by canonical
user/event/session and policy generation. Save/restore is explicit; logout,
revocation and access loss synchronously remove saved and displayed drafts.
Route composition should retain this controller while its scoped edit journey
is active and dispose it on exit. Browser reload does not restore drafts or
unknown command state; it never replays a write. Durable cross-reload intent
reconciliation remains a separate FE26 obligation.

Each user intent gets one UUID idempotency key. Duplicate clicks, pending and
unknown outcomes are locked. A status check uses only the captured operation/key;
404, retired/expired receipts and outage never imply failure or authorize a new
write. Valid successful receipts require an authorized resource reread before
success. No command retry loop or mutation refresh/replay exists.

### Validation

Use Node `/Users/lhs/Desktop/mcr-tools/node-v24.22.2-darwin-arm64/bin/node` and
pnpm `/Users/lhs/Desktop/mcr-tools/pnpm-11.7.0/package/dist/pnpm.mjs` from the
assigned checkout. Commands and exit codes:

- `python3 scripts/mcr/check-plan.py`: **0**; planning/source archive check only.
- `pnpm --dir frontend install --frozen-lockfile --offline`: **0**; retained resolutions.
- `pnpm --dir frontend test src/features/party-authoring`: **0**, eight focused tests.
- `pnpm --dir frontend typecheck`: **0**.
- `pnpm --dir frontend build`: **0**; existing >500 kB bundle warning remains.
  Root build does not yet route/bundle the new isolated authoring entry.
- `frontend/node_modules/.bin/oxlint frontend/src/features/party-authoring`: **0**, no warnings.
- `node frontend/src/features/party-authoring/__tests__/browser-smoke.mjs`: **0**;
  native Chromium 156.0.8078.80, widths 360/390/430/1280, local deterministic fetch
  through actual Auth/privacy. Screenshots and source hashes are in
  `party-authoring/__tests__/evidence/browser/`. The 390px form was visually inspected.
- `git diff --check`: **0**.

Tests exercise invalid inputs, exact create payload and duplicate clicks,
unknown network response plus 404 receipt, contextual pending/succeeded receipt
recovery, version-conflict rebase, non-owner denial, private/mismatched response,
late response after identity/policy change, RHF errors, explicit restore and
rendered logout cleanup. Browser checks additionally cover server input failure,
unknown-write lock, recovery without additional POST, no horizontal overflow,
and no browser errors.

Real backend authoring/authorization, physical place verification, full shell
route integration, multiuser races, iOS/Android keyboard/safe-area/device and
IN02 acceptance are **NOT_RUN**. FE23/FE25/FE26 are still **NOT_STARTED** at this
first bounded handoff. Image upload requires the future qualified FE04/BE24
port and does not block FE22.

Changed paths: only `frontend/src/features/party-authoring/**` and this document.
The implementation commit is a direct descendant of the recorded base; the
exact commit is supplied in the delivery message (no self-referential SHA).

## FE23 bounded handoff — 2026-11-04

Status: **MOCK_READY** for owner/member/rally consumer behavior. Parent source is
FE22 commit `1c38ad4`; candidate C04 inputs remain pinned to `f0e44bd` above.
Changed paths for this increment: `frontend/src/features/party-management/**`
and this document only. Newly authored; dependencies are existing React,
canonical Auth/privacy, the FE21 public-party decoder and owned FE22 closed-schema
interpreter. No backend/shared/root/contracts changes or old rendering copied.

Exports: `createPartyManagement`, `PartyManagement`, `PartyRally`, and typed
`ManagementOptions`/`MeetingAuthority`. Owner decisions use captured application
and party revisions, applicant/party/event identity and expected target state.
Member removal and party cancellation require explicit confirmations. Paginated
applications and members have independent cursors and contextual duplicate/limit
checks. A last-slot/version conflict rereads current counts and revisions; no
optimistic approval or fabricated capacity. Non-owner tools stay unavailable;
403, policy/access changes and logout scrub private rows and meeting views.

Rally requires complete authorized membership reads, current active party,
explicit physical acknowledgement and a scoped expiring authority input. Meeting
revision changes supersede old rally; expired/missing authority hides physical
description and disables reports. No world/GPS/arrival inference exists. The
`reportIntent({state, meeting_revision, physical_presence_acknowledged:true})`
method is directly compatible with FE20 `ExplicitArrivalControls.onIntent`.
The delivery composer can use FE20 controls with this controller instead of the
standalone fixture UI. Production MeetingAuthority must be projected from FE20's
qualified server beacon/audience authorization, never from public place labels.
The authority read/subscribe port does not itself mint a server grant.

Receipt conventions explicitly qualified by delivery and communicated by the BE
owners on 2026-11-04: Member result `resource_id=user_id`, `Member.revision`, bound
to exact event/party/user canonical command path; Rally result `resource_id=actor
user_id`, `RallyEntry.revision`, additionally bound to the captured meeting
revision and report. Never compare either revision across parties. Recovery
rereads the exact authorized result; membership recovery scans up to ten
additional pages and remains uncertain if it cannot verify the target. Event
outbox invalidation is coarse community-query invalidation, not a global
per-user/member cursor. No wire field or new resource UUID is invented.

Pending, uncertain, missing/retired/expired receipts remain locked; status reads
use the original command/key and never replay writes. Approval recovery binds
application ID/applicant/state/revision, cancellation requires actual CANCELLED
party, removal requires REMOVED membership in this party, and rally requires the
same actor/party/meeting/state. Safety removal/cancel honors the C04 READ_ONLY
exception when mutations are disabled specifically for EVENT_READ_ONLY.

Validation (Node/pnpm paths above), all exit **0**:

- Focused `test src/features/party-management`: **13 passed**. Prior combined
  FE22/FE23 run: **20 passed** before the additional qualified-rally regression.
- `typecheck` and `build`: passed; existing bundle-size warning unchanged.
- Scoped `oxlint frontend/src/features/party-management`: no warnings.
- `node frontend/src/features/party-management/__tests__/browser-smoke.mjs`:
  Chromium 156.0.8078.80 at 360/390/430/1280; confirmation without write,
  acknowledgement, last-slot refresh, superseded authority, unknown-command
  lock, logout eviction, overflow/error checks passed. Source hashes and images
  reside under `party-management/__tests__/evidence/browser/`. 390px owner/rally
  image visually inspected.
- `git diff --check`: passed.

Routed shell composition, FE20 verified live authority adapter, public HTTP
producers, real multiuser last-slot races, physical operation/device and IN02
acceptance remain **NOT_RUN**. This does not establish BE09/BE19 completion.
FE25/FE26 remain next in the active goal; FE04 owner has published its exact
READY attachment seam while implementation/qualification continues separately.

### FE22/FE23 semantic follow-up

BE19 clarified that a current Rally can retain entries from an older meeting
revision. They now render `STALE · previous meeting report`; old HERE/MET is never
presented as current. A fresh explicitly acknowledged report retains the prior
self revision (for example 7) while supplying the new meeting revision (2),
rather than inventing self revision zero. Future entry meeting revisions are
rejected. A focused regression covers this exact transition. FE22 also rejects
an already-ended, otherwise ordered meeting window before dispatch.

Both feature suites: **23 passed**, exit 0. Updated native browser receipts and
source hashes at all four widths: exit 0. Typecheck: exit 0. This follow-up changes
only the two owned party features and this document; FE25 remains an uncommitted
separate increment at this checkpoint.

## FE25 bounded handoff — 2026-11-04

Status **MOCK_READY**. Parent `58e18ab`, original source/candidate base `f0e44bd`.
Exact changes: `frontend/src/features/giveaway-authoring/**` and this document.
All UI/controller/validation is newly authored; only existing React/RHF/Zod,
canonical Auth/privacy, FE24 public types and the owned FE22 closed-schema
interpreter are used. No source archive, contracts, locks, shared media/transport,
backend or root route changes.

Exports: `createGiveawayAuthoring`, `GiveawayAuthoring`, `GiftDraft`, typed
`QualifiedImagePort`, `QualifiedPickupMeeting` and options. New creates send only
C04 CreateGiveaway fields and require DRAFT response. Owner management reads
validate owner/event/giveaway and reservation context, stock equation, partial
page counter bounds and duplicate IDs. Total quantity cannot undercut reserved
plus claimed inventory; outstanding reservations prevent window/place edits.
Independent reservation continuation rejects changed stock revisions. Version
conflicts keep the form and require explicit reread/rebase. Pause/completion/
cancellation are the frozen UpdateGiveaway state field; cancellation requires
explicit acknowledgement. Current capability checks fail closed.

The structural image seam matches the FE04 owner's published ImageUploadPort
subset. It is an injected port, not a local upload transport. Only a current
scope, GIVEAWAY-purpose READY attachment passing C04 Media validation and exact
same-event media path is attached. Upload failure/missing port never pretends
media exists. Text-only drafts send `image_media_id=null`; optional blob preview
comes only from the qualified ephemeral upload port. Privacy cleanup cancels the
upload and clears saved forms/inventory. Session drafts restore text/form values
only, not an old upload attachment.

An optional qualified pickup-meeting read/subscribe port enables the existing
`setConcertPickupMeeting` operation, with an explicit physical acknowledgement.
The projection must already carry reviewed place/map/instruction authority and
current scope/expiry; the controller never creates that authority. Missing port
shows an honest publication prerequisite. Server authorization, instruction
validation and lifecycle invariants remain authoritative. Owner reservation-row
callback allows delivery to wire the separate FE26 claim journey.

Unknown writes lock the form and use original-key receipt recovery. Successful
save/receipt requires an authorized management reread at least as new as the
confirmed result revision. Server errors preserve form; owner denial evicts it.
Draft restore is memory-only. No photo, token, code or physical instruction is
persisted by this feature.

Validation, all exit **0**:

- `pnpm --dir frontend test src/features/giveaway-authoring`: **8 passed**.
- `pnpm --dir frontend typecheck` and `build`: passed (existing bundle warning).
- scoped Oxlint: passed with no warnings; `git diff --check`: passed.
- Native `giveaway-authoring/__tests__/browser-smoke.mjs`: Chromium
  156.0.8078.80, widths 360/390/430/1280. Checks loaded edit values, honest absent
  upload, stock-conflict form preservation, explicit rebase, uncertain-write
  lock, owner inventory, logout eviction, no overflow/browser errors. Evidence
  and source hashes: `giveaway-authoring/__tests__/evidence/browser/`.

An initial test-fixture relative import and a TypeScript JSON structural type
were corrected before the passing checks. Backend/public upload and owner routes,
actual BE24/FE04 upload integration, FE20/live physical instruction projection,
shell routing, real stock races, devices and IN02 remain **NOT_RUN**. FE26 remains
active and is the next bounded increment.

### KST display correction (supersedes earlier explicit-UTC UI wording)

Specification §7.4 requires UTC transport and KST display. The three delivered
features now use KST date/time inputs and English KST meeting/inventory labels.
Owned `party-authoring/model/time.ts` converts strict KST local input to UTC
RFC3339 and formats server UTC in Asia/Seoul; no backend contract is changed.
Two regressions cover date rollover and impossible calendar input. Native
receipts/screenshots/source hashes for all three features were regenerated and
pass at 360/390/430/1280; typed date controls use normalized minute input in the
browser harness. Combined feature/discovery/domain-consumer checks **213/213**,
strict build/typecheck, four-feature Oxlint and source-origin/boundary checks all
exit **0**. FE26's separately staged consumer uses the same owned helper.

## FE26 bounded handoff — 2026-11-04

Status **MOCK_READY**, parent `30c7ad5`, original C04 candidate base `f0e44bd`.
Exact increment paths: `frontend/src/features/gift-pickup/**` and this document.
C04 giveaway schema SHA256:
`3a5af6301bb9514623aa7d6e68b3ee27b4b89709836801f4c10741f149a7577d`.
Newly authored UI, controller and journal; existing canonical Auth/privacy,
FE24 public types, owned FE25 contextual validation and FE22 time/receipt/schema
helpers are imported directly. No new dependency, server, root route, contract,
shared media or transport modification.

Exports: `createGiftPickup`, `GiftPickup`, `createSessionIntentJournal`, and
`GiftPickupOptions` / `PickupPlaceAuthority` / `IntentJournal`. Compose recipient
routes with an exact giveaway ID and optional reservation ID. An owner must use
the giveaway-bound management route to resolve an owner reservation; a naked
reservation-only route authorizes only its recipient. Supply the current gift
capability context, canonical auth, a qualified advancing server clock estimate,
and the reviewed current place/map authority read/subscribe projection. Missing
or expired authority hides private instructions before they are ever published
to observers and disables recipient code issuance. No virtual location evidence
or invented physical arrival is used. UI dates use English KST; requests use UTC.

The default browser session journal stores only version, actor/event IDs,
operation, original idempotency key, giveaway/reservation IDs and expected
revision. A strict closed schema rejects unknown fields. Storage is scoped by
actor/event; new writes fail closed when the journal is unavailable/invalid.
It is written before dispatch, restored before reads/new intents, and blocks
concurrent/new operations until the original outcome is resolved. Route disposal
clears in-memory private UI while retaining an unfinished minimal journal for
navigation/reload recovery. Active privacy/logout cleanup removes the previous
scope's journal and private state. No token, plaintext code, instruction, form,
HTTP request body or raw server error is stored. No automatic command replay or
receipt polling exists; backoff/expiry timers update UI only.

Commands: `reserveConcertGiveaway`, `cancelConcertReservation`,
`issueConcertPickupCode`, `claimConcertReservation`; reads:
`getConcertGiveaway`, `getConcertReservation`, `getConcertPickup`,
`getConcertCommandStatus`. Cancellation requires explicit confirmation and
honors the safety-policy READ_ONLY exception. HELD is not READY, code issuance
is not collection, and expired/sold-out/duplicate outcomes do not show a fake
active hold or collection. Owner code rejection is a failure, never success.
Successful owner collection additionally rereads the exact CLAIMED reservation
and its matching durable pickup receipt ID; repeated UI confirmation is blocked.

Delivery qualified all four reservation command references as reservation UUID
plus post-action Reservation.revision; the separate first-claim
PickupReceipt.receipt_id is not the BE15 command receipt UUID. Status recovery
binds actor/event/key/operation, exact reservation/giveaway and same-context
revision. A code/cancel/claim receipt must advance beyond the captured expected
revision. Successful reserve recovery that finds a subsequently expired/terminal
resource reports current unavailability instead of an active success.

Codes are memory-only, validate exact reservation/revision, and are cleared on
expiry, authority loss, reload and privacy changes. A lost code response can
recover **issuance only** through safe receipt metadata plus authorized
reservation reread. It shows code unavailable; a new explicit rotation requires
fresh pickup details, revision and key. The C04 same-key code replay wire variant
remains an explicit contract gap, unused by this consumer. No fabricated 202,
plaintext recovery or retry of that mutation is implemented.

### Final qualification

All commands below exited **0** using the pinned runtime. Final source hashes
and native screenshots are retained in each feature's `__tests__/evidence/browser`.
The final 390px party form and pickup-code fixture were visually inspected.
Synthetic displayed codes are test fixtures, not real attendee credentials.

- FE26 focused suite: **17 passed** (reservation duplicate/unknown/reload,
  404/retired status, sold-out/duplicate/expired/foreign projection, journal
  denial/corruption, physical authority, private-code expiry/loss/rotation,
  one-time owner claim, invalid code, explicit cancellation, identity/policy
  cleanup, strict storage schema, stale receipt revision and terminal recovery).
- Combined `pnpm --dir frontend test src/features/party-authoring
  src/features/party-management src/features/giveaway-authoring
  src/features/gift-pickup src/features/party-discovery src/features/gift-discovery
  src/features/auth/__tests__/domain-request.test.ts`: **213 passed in 12 files**.
  This is the affected consumer set, not a full-repository acceptance claim.
- `pnpm --dir frontend build` / strict typecheck: passed; the existing bundle
  >500 kB warning remains. Root application does not yet bundle these new routes.
- Oxlint over all four owned feature directories: passed, no warnings.
- All four feature native-browser scripts: Chrome **156.0.8078.80**, widths
  **360/390/430/1280**, passed after the KST correction. FE26 uses real browser
  sessionStorage and actual reload with a synthetic server fixture. It verifies
  no write replay, current authorized hold reconciliation, private pickup and
  one-time owner collection, no persisted plaintext, missing plaintext after
  reload, logout eviction, and no horizontal overflow or browser error.
- `python3 scripts/mcr/check-boundaries.py --base f0e44bd --lane frontend
  --allow-doc docs/mcr/frontend/community-authoring.md`: passed. Manual changed
  path/import review confirms the four assigned folders plus this document only.
- `node scripts/mcr/check-ui-origin.mjs`: passed, no source-origin violations.
- `git diff --check`: passed. Initial planning/protected-source check passed;
  no source archive path is changed anywhere in this branch.

Root composition, production FE04/BE24 integration, live qualified physical
place/meeting authority, real HTTP producers, real multiuser races, actual iOS
Safari/Android Chrome and IN02 are **NOT_RUN** and remain delivery/qualification
work. These limitations do not block this expressly assigned fixture-backed
consumer handoff. No task or IA entry is promoted to ACCEPTED by these tests.

Final bounded worker sequence (each commit descends from the prior one):
`f0e44bd` → `1c38ad4` FE22 → `7342058` FE23 → `58e18ab` semantic fixes →
`dee1392` FE25 → `30c7ad5` KST correction → FE26 handoff commit supplied directly
to delivery. Only the designated delivery integrator publishes master.
