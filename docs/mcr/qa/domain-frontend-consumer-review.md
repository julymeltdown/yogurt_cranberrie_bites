# C04 frontend consumer review — FE21 / FE24

Status: **C04_REVIEW_CANDIDATE; changes required before scoped freeze.** Independent review, not feature/runtime acceptance.
Date: 2026-11-03. Base: `4dc3bb3dd8015abcf26839c34954f7ca91870ceb`; branch `codex/mcr-fe-C04-consumer`.
Only this report changes. Read specification §§5–8 and FE21/FE24, assessment/execution plan, C04 operation/schema/policy/combined candidate, and actual FE01/FE02/FE07/FE09 sources. No application, contract, dependency, source archive edits or push.

## Candidate pins

SHA-256, relative to this base; additional consumer/source pins are in the private evidence manifest.

| Input | SHA-256 |
|---|---|
| `contracts/domain/v1/operations.json` | `42ed48ee0cad846b94f9f7a5d39a38beeeb5f5b41e0129f157e51ff31c635223` |
| `contracts/domain/v1/policy.json` | `0e04fe478d757d9d6ab434d3aa25aade523d35d2d2a2a0574f17a04f7f395339` |
| `contracts/domain/v1/common.schema.json` | `303f3019611c636679282d6c29106deab83d7c0c9717bd8b9ba526d2ff865a35` |
| `contracts/domain/v1/party.schema.json` | `6a1ae03455cea256e26b75b0063927a4972c5e63ceaceeb45bf9b48837f63578` |
| `contracts/domain/v1/giveaway.schema.json` | `3a5af6301bb9514623aa7d6e68b3ee27b4b89709836801f4c10741f149a7577d` |
| `contracts/generated/mcr-domain.openapi.json` | `40b0791c1172c983fac7f7f51dedc43b482ba74cb10151baed5b7efac0f98625` |

## Findings and minimum parent changes

**R1 [P1] FE21 commands cannot carry required idempotency or retain domain error reasons.**
`applyConcertParty` and `cancelConcertApplication` require canonical UUID `Idempotency-Key` in both registry and assembled header parameters.
`frontend/src/shared/api/http.ts:121,145–148` accepts only auth/core families and three correlation headers; actual `auth.request` rejects a valid key with `invalid-request`, zero dispatches. `errorFamily: domain` also fails before dispatch.
With the key deliberately omitted solely to isolate response parsing, contract-valid `409 DUPLICATE_APPLICATION` becomes `TransportError.server=null` through `core` (`shared/api/errors.ts:30,112–137`); expiry/conflict/transition/feature reasons also extend the core allowlist.
Parent must add a bounded, operation-qualified UUID header path and domain status/reason parsing through existing FE01/FE09. Retain no automatic command replay, same-origin/no-store/redirect restrictions, generated intent and browser-owned Origin; do not allow arbitrary headers or create another fetch/client.

**R2 [P1] Valid discovery state filters are blocked by the auth-secret query guard.**
`listConcertParties` accepts `state=CLOSED`; `listConcertGiveaways` accepts `state=PAUSED`. Both targets validate against C04, but canonical GET requests fail `invalid-request` with zero dispatches because `shared/api/http.ts:140` rejects every query key named `state`.
Parent must distinguish bounded registered domain filters from forbidden Auth/OAuth state parameters. Keep the credential guard on unqualified targets; a global removal is unnecessary. Freeze operation/query binding before feature filters are written.

**R3 [P2] The shell projection cannot drive required capability-aware actions.**
`features/auth/model/bootstrap.ts:82–91` validates full C03 capabilities but returns only `worldAvailable`; `app/ports.ts:15–20` and the FE07 panel seam supply no retained meet/gifts/mutations decision.
The actual decoder maps otherwise identical valid bootstrap payloads with all three enabled versus disabled to identical shell results. Inferring eligibility from event OPEN, spare capacity or stock would ignore explicit server disable.
Parent must retain scope-bound validated capability/reason data in one canonical owner and inject it into panels. OWNER/APPLICANT/MEMBER presentation derives from authorized DTO identities/own activity, never URL, admin port or client assertions.
Public DTOs have no viewer-specific eligibility decision. Freeze conservative unknown/disabled behavior until relevant own activity and current capabilities are known, or define reviewed viewer metadata; absence on one activity page proves neither nonmembership nor application/reservation eligibility.

**R4 [P1] Unknown-key receipt recovery is undefined after uncertain apply/cancel.**
`getConcertCommandStatus` uses `GET .../commands?operation_id=...&idempotency_key=...`; `common.CommandReceipt` has PENDING/SUCCEEDED/FAILED/RETIRED. Policy specifies status-before-identical-retry and expired-key tombstones.
The status-query 404 is only `RESOURCE_UNAVAILABLE`, also used to conceal denied/missing targets; neither schema nor policy defines never-recorded keys versus revoked authority, or when retry is permitted after this response. Timeout followed by 404 establishes neither nonexecution nor a remaining retention window.
Parent must freeze an authorized unknown-key rule on this existing operation: a safe distinguishable outcome with retry eligibility/retention semantics, or explicit fail-closed unavailability that never authorizes replay. Define RETIRED/expired mapping; preserve original key/input, never mint a new key to escape uncertainty.
Bind receipt to captured actor/event, requested command/key and expected result target/kind. Oracle `scripts/mcr/validate-domain.py:398–407` checks actor and any registered command, not requested command/key: a reserve receipt passes an apply lookup with a different key.
202 already reaches synchronous decoders through `DecodeContext.status`; no new response API is needed. PENDING is not success; SUCCEEDED metadata requires current authorized activity/resource reread, FAILED is terminal feedback, RETIRED/expired never reexecutes. Session/policy changes clear presentation and suppress delayed completion.

**R5 [P2] Closed schemas do not yet qualify operation-specific read projections.**
`scripts/mcr/validate-domain.py:395–408` compares only top-level event IDs and special-cases receipts. Probes pass a foreign-event row in `listConcertParties`, another recipient in `getConcertGiftActivity.receiving`, and another owner's row in `getConcertPartyActivity.owned`, despite supplied actor/event context.
`giveaway.schema.json:6` permits DRAFT in the shared public/owner DTO; DRAFT public list/detail responses pass the oracle although `policy.json:218` excludes drafts from public reads. This proves missing contextual qualification, not an observed server leak.
Before read freeze, specify/test predicates: row event and selected detail ID match request; own owner/applicant/member/recipient identities match captured actor; public gifts exclude DRAFT while own giving/management may retain it. Check linked IDs, duplicates, state/time/count invariants and current authorization before cache/UI commit. Split public/owner schemas only if preferred; new endpoints are unnecessary.
Public party/gift projections must exclude applicant/recipient lists, private meeting instructions, handles and pickup secrets. Owner activity remains private when its elements reuse public shapes; public DTO naming does not make it anonymous/cacheable.

## Bounded implementation decision

Do not freeze as-is. The six party/gift list/detail/own-activity reads can qualify independently after R2/R3 and narrow R5 predicates/fixtures, without waiting for all 106 operations, live BE routes or global legal/deletion work. Their read errors use existing core reasons.
FE21 apply/cancel additionally needs R1/R4. FE24 gains neither FE25 owner mutations nor FE26 reserve/pickup scope.
Party activity has owned/membership/application cursors; gift activity has giving/receiving cursors, independently paged at default 20/max 100. Preserve each continuation, reset on actor/event/filter/policy changes, allow empty pages with continuation without automatic loops.
Use protected canonical `auth.request`; query absence is omission, DELETE cancellation uses `expected_revision` query and no JSON body. Existing intent, event-path fencing and decoder status seams work.
FE02 `privacy.run` plus `registerCleanup` supports scoped local state/drafts/mutations/subscriptions, including policy changes within one Auth generation. If adopting `privacy.options`, parent must extend `shared/query/keys.ts:8–22` for domain resources and canonical filter/page identity; today's `'parties'` key throws. Do not disguise domain data under another resource or add persistence.
FE07 accepts injected elements at `/meet`, `/meet/parties/:partyId`, `/gifts`, `/gifts/:giveawayId`, `/gifts/mine` through `createAppRoutes(...,{panels})`. Reuse one shell/world owner; feature decoders retain context/authorization responsibility. MY PLAN can be in-panel; no invented API/route is needed.

## Commands, evidence and limits

Private evidence: `/tmp/mcr-c04-fe-review/`; no server/browser port opened. Requests use synthetic injected fetch. `manifest.json` SHA-256: `496bb2489e0c204811a8438b07608158e69e49bccd5b4ebb9b9a466f24b46d1c`; all tested source pins verified unchanged.
`/Users/lhs/Desktop/mcr-tools/node-v24.22.2-darwin-arm64/bin/node /tmp/mcr-c04-fe-review/consumer-probe.mjs`: exit 0, ten cases exercising actual transpiled pinned sources, including FE09 request/bootstrap; hashes/results `consumer-result.json`, output `consumer.log`.
Transpiler reads retained TypeScript/dependencies from stopped FE19 checkout and writes modules only under private `/tmp`; no install/dependency edits. First attempt exit 1 used an invalid synthetic capability reason; corrected to `FEATURE_DISABLED`, preserving `consumer-first.log`. Product sources were not changed to pass probes.
`python3 -B /tmp/mcr-c04-fe-review/contract-probe.py`: exit 0, 24 narrow checks/reproductions, including nine assembled operation registrations, valid state/cursor/DELETE/status targets and domain error. Inputs/results/pins `contract-result.json`; output `contract.log`.
`git diff --check` and sole-path staged review: exit 0. Commit descends directly from full base above; SHA reported separately to avoid self-reference.
No broad 47-test/domain/build reruns, Cargo/PostgreSQL, production authorization/races, browser/device or feature acceptance is claimed. Goal remains ACTIVE through parent review and required report corrections.
