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
