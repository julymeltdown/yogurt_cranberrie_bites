# FE21 — current client completion

Original FE21 pure-client criteria: **SATISFIED**. Qualification status: **MOCK_READY** against frozen contracts and synthetic HTTP/C05 in the actual default application. The current [FE21/FE24 delivery record](fe21-fe24-client-delivery.md) records the final criterion matrix, source pins, failures/fixes, 148 focused checks, three native touch journeys, and limits. Publication is handled by Root153 with fresh owner remote verification; no King/SL review stage applies.

The historical source/validation record below is preserved. Its initial review queues, uncomposed-root statements and earlier goal states are superseded by the current record. The default root now composes the feature; server/IN02 and physical-device qualification remain separate.


## Historical initial delivery

Status: **MOCK_READY**, pending integrator/independent review and application composition.
This delivers the full assigned FE21 consumer feature against the frozen six-operation
subset. It does not certify live backend behavior or task/IA acceptance.

## Scope and provenance

Worker branch: `codex/mcr-fe-FE21`. Exact base:
`5f884edfe636a3a9a8f8b08fb33044c5d57cd0aa`. The delivery commit is the commit
introducing this document and `frontend/src/features/party-discovery/`; the handoff
message records its exact SHA and ancestry. Only those two assigned paths change.
The integrator records reviewed integration ancestry and final task status.

Product authority: `MCR_IMPLEMENTATION_PLAN_V2_EN.md` FE21, M01 discovery and M03
detail; `docs/mcr/project-assessment.md`, `docs/mcr/execution-plan.md` and root/
frontend working agreements. Consumer authority:
`docs/mcr/party-gift-consumer-contract.md` revision 1 at the base above. Frozen
C04 definitions/artifact revision: `ca9f955cd22f7e4c1da029d034c1923f1e98bb9d`;
canonical transport/capability prerequisite `2645b6f4cc639837932fb9f8480046533987a3de`;
independent context regressions integrated in
`88fd8f3e37fe3fe8cb9e63f99a1e22d2c16c9762`. Whole C04 remains REVIEW_CANDIDATE.

| Read-only authority input | SHA-256 |
|---|---|
| `contracts/generated/mcr-domain.openapi.json` | `c47e767505e660d8f5eb69bb9759eae1f0aaee6472e4fb963edc82e060385f51` |
| `contracts/domain/v1/operations.json` | `42ed48ee0cad846b94f9f7a5d39a38beeeb5f5b41e0129f157e51ff31c635223` |
| `contracts/domain/v1/party.schema.json` | `7cbea6af623668a36d641e7498bf52f4869b9bc44594b46e7f4a3806cc907d03` |
| `contracts/domain/v1/common.schema.json` | `303f3019611c636679282d6c29106deab83d7c0c9717bd8b9ba526d2ff865a35` |
| `contracts/domain/v1/policy.json` | `508d22af964b718cdcbd5a25b32b9a1e3f7b651a5b8c25c87d56f1f6b68d0ebe` |
| Retained `frontend/pnpm-lock.yaml` | `47ca44459819ad0e887d2f4f93394fdf3b1053b8f45edc9ceb0236981feee1f6` |

All product rendering, CSS, controller and consumer decoder code is newly authored.
No Bigyeting rendering or new dependency is imported. The decoder reads the frozen
generated public contract, including reachable C03 PublicProfile/AvatarState/
AvatarConfig schemas. Avatar configuration is validated but no character image is
rendered. No source-pack originals or legacy artwork are copied or modified.
`docs/source/` remains unchanged; the planning checker verifies all 13 original
archive checksums.

Test-only browser setup follows the existing FE19 harness pattern at this base,
reusing current `app/router`, Auth, FE02, FE14 `loadMap`, FE15 `attachMovement`,
`world/runtime`, and existing synthetic terrain helpers through imports. Its party,
profile and receipt fixtures are newly authored metadata, never approved art or
production content. Connection counters model shell ownership; they are not real
WebSockets. Destination paths are the exact assigned feature paths below.

## Delivered behavior

- Discovery has ticket-section, state, available-space and local-time start-range
  filters (canonical UTC on the wire), clear empty/error states, explicit list
  pagination, capacity/state labels and shell-aware detail navigation.
- Detail presents the public owner projection, description, requirements, section,
  member count, full/closed/cancelled/completed states and times. A place link opens
  existing reviewed place information; no physical destination or arrival is
  inferred from virtual position or a public place ID.
- MY PLAN has independently paged owned parties, memberships and own applications.
  Each cursor is advanced only by its explicit collection control. Empty pages
  with continuation remain incomplete. Other collections returned by a continuation
  request are validated but do not overwrite their retained pagination state.
  Per-page duplicate identities, over-limit results, repeated continuation cursors
  and duplicates across accumulated pages fail closed. Membership identity is
  `party_id`, so one actor can have memberships in multiple parties.
- Apply requires fresh ready detail/activity, complete relevant MY PLAN pages,
  current actor/event/policy scope, OPEN party with space, no owner/active member/
  pending or approved application, and available meet+mutations in OPEN/LIVE.
  Partial absence never proves eligibility. The server remains authoritative.
- Cancellation requires a current own PENDING application and its revision.
  READ_ONLY with meet available and mutations denied specifically for EVENT_READ_ONLY
  permits this SAFETY action. Other unavailable/deny reasons and DRAFT/ARCHIVED
  fail closed. Approved membership removal remains outside FE21.
- The original command intent captures operation, actor/event/privacy scope,
  target, revision, note and canonical UUID once. Double clicks are locked before
  dispatch. Apply sends POST JSON; cancel sends DELETE `expected_revision` query
  with no JSON body. Both use typed `idempotencyKey` through canonical Auth.
- Direct final Application responses bind actor/event/party and known application
  ID and expected PENDING/CANCELLED transition. 202 admits only matching PENDING
  receipt metadata. Network loss, malformed successful writes, ambiguous server
  failures and idempotency uncertainty retain the captured command without replay.
- Status checks are explicit bounded GETs with the original operation/key. A known
  final resource ID remains bound across later checks. SUCCEEDED requires final
  status 200 and authorized fresh MY PLAN reconciliation; receipt metadata alone
  never becomes current application state. If the result is absent from the first
  page, the UI stays reconciling and allows further explicit application pages.
- Status 404, outage, RETIRED and retention expiry never prove nonexecution/success
  and never permit a new key to escape uncertainty. Client-clock expiry is advisory.
  There is no replay method, automatic polling, reconnect resend or mutation refresh.
  Definitive failure evicts eligibility data; fresh detail/activity reads are required
  before reviewing a different intent. The exact failed input remains locked.
  Confirmed cancellation allows a later explicit new application with a new key.
- All private projections and merged pages are immutable in memory. The existing
  canonical Auth request/session/QueryClient/FE02 coordinator remain the owners;
  no second client, token store, QueryClient, private query namespace, persistence,
  world instance or connection is created by this feature. FE02 cleanup synchronously
  aborts reads/commands/status work and clears rows, form drafts, note, intent locks,
  keys, receipt presentation and timers on session/event/policy changes. Late work
  is fenced against full `PrivacyScope`, including same-identity policyGeneration.
- Safe enum-based errors cover 403/404/429/503, conflicts, full/duplicate and read-only
  outcomes. Raw messages/bodies/keys are not rendered or exposed in feature state.
  No decorative left rails/borders or character fallbacks are introduced. Controls
  have visible keyboard focus and 44px targets; panel navigation uses FE07 ShellLink
  and stable opener IDs for heading/Back focus and scroll memory.

## Files and composition contract

Product entry: `frontend/src/features/party-discovery/index.ts` exports
`createPartyController`, `PartyController`, `PartyAccess`, `PartyAccessPort`,
`PartyDiscovery` and `PartyDetail`. Models are `model/contract.ts` and
`model/controller.ts`; UI/CSS are under `ui/`. Tests, test-only fixtures and
native runner are under `__tests__/`.

The integrator owns root composition. Create exactly one controller for the active
shell owner and retain it across panel navigation. Supply the canonical auth object
and a scope-bound access port. It contains **all seven** validated C03 capability
decisions, event phase and the full PrivacyScope captured before the successful
canonical bootstrap validation. Never attach today's policyGeneration to an older
bootstrap or infer capability availability from a role/phase alone.

```tsx
const party = createPartyController({ auth, access: partyAccessPort })
function PartyDetailPanel() {
  return <PartyDetail controller={party} partyId={useParams().partyId ?? ''} />
}
const routes = createAppRoutes(ports, { panels: {
  '/meet': <PartyDiscovery controller={party} />,
  '/meet/parties/:partyId': <PartyDetailPanel />,
} })
```

`partyAccessPort.read()` returns `{ scope, phase, capabilities } | null`;
`subscribe(listener)` notifies on valid release or withdrawal. To release from
canonical `ports.bootstrap.load`, capture `auth.privacy.capture()` before awaiting
that existing load, retain its validated result, and verify the same current privacy
scope plus participant/event binding after it returns. Publish that captured scope
and result's phase/capabilities. On FE02 cleanup withdraw the access projection;
a fresh canonical validation is required before republishing. The feature starts
once when mounted and reads when a fresh access release becomes available.
It never calls bootstrap itself. Dispose it at shell-owner teardown, not when one
panel unmounts. The existing FE07 route wrapper owns panel/world lifetimes.

No root router, capability adapter, shared registry, manifest, lockfile, backend,
contract or infrastructure file is changed in this handoff. MY PLAN is a retained
in-panel tab on `/meet`; no new route/query grammar is required. Existing FE07
People/create-party/intro links remain separately owned FE22/FE29 flows.

## Validation and evidence

Runtime: macOS arm64, retained Node 24.22.2 / corepack pnpm 11.7.0. Installed
Google Chrome 156.0.8078.80 (exact runtime version also in browser report). Dependency
installation used `pnpm install --offline --frozen-lockfile`, exit 0, no downloads
or lock changes. Use the retained Node bin on PATH for these commands.

| Command from repository root (frontend commands run in `frontend/`) | Result |
|---|---|
| `python3 scripts/mcr/check-plan.py` | Exit 0, PASS; planning/provenance only |
| `pnpm exec vitest run src/features/party-discovery/__tests__` | Exit 0, 65 checks across 3 files |
| `pnpm build` | Exit 0; TypeScript and Vite build pass; existing >500kB chunk advisory |
| `pnpm lint` | Exit 0, no feature warnings |
| `MCR_FE21_EVIDENCE=/tmp/mcr-fe21-browser-20261103-08 node src/features/party-discovery/__tests__/browser-smoke.mjs` | Exit 0, PASS at 390×844 and 1280×900 |

Private local logs: `/tmp/mcr-fe21-tests-final.log`, `/tmp/mcr-fe21-build-final.log`,
`/tmp/mcr-fe21-lint-final.log`. Native report/screenshots:
`/tmp/mcr-fe21-browser-20261103-08/`. The report records exact source hashes,
contract hashes, checkout base, installed browser, widths, assertions and cleanup.
Screenshots are not committed to the repository. Test assertions cover closed nested
public projections, count/time/actor/event/resource/status binding, pagination,
eligibility, all relevant write capabilities, canonical headers/body/query, double
clicks, uncertainty, original-key recovery, receipt states, fresh reconciliation,
401 without refresh/replay, and privacy/selection/disposal races.

Native browser journey covers filters and empty results, keyboard detail and restored
Back focus, independent partial MY PLAN completion, one POST for double clicks,
READ_ONLY DELETE cancellation, a later application through 202 PENDING and explicit
status checks, fresh SUCCEEDED reconciliation, then lost-response/404/outage recovery
with the original key, full/cancelled/private states, safe copy, no horizontal overflow,
44px controls, movement blocked by panels, one persistent actual Phaser canvas and
one instrumented connection owner across navigation, privacy eviction, zero page
errors and complete game/control/connection disposal. Phone and wide screenshots
were visually inspected, including discovery, detail/apply, pending/recovery and
MY PLAN states. Earlier private iterations remain distinguishable: 01/02 exposed
navigation-test/focus gaps, 03 passed the initial journey, 04 exposed repeated
synthetic application IDs in the expanded fixture, 05/06/07/08 pass the expanded journey.
Only report 08 matches the final production source; the repeated-ID fixture was
corrected to issue distinct identities for distinct applications.

## Remaining gates and limitations

- Actual root composition and independent review/fixes are integrator-owned. The
  current product entry remains uncomposed until that integration; build success
  alone is not proof of visible feature delivery. The browser harness composes the
  actual persistent shell with this feature through the same public seam.
- Deterministic custom fetch fixtures exercise canonical request/decoder/privacy
  behavior, not real browser-to-server Origin/CORS, cookie authentication, service
  authorization, SQL capacity/uniqueness races, real multiuser state, or a network
  WebSocket. Those checks are NOT_RUN here and cannot be certified by these tests.
- No FE22 authoring, FE23 party administration/rally, FE24/25/26 gifts or FE29 people
  implementation is included. Their task/IA obligations remain intact.
- Public place IDs are not physical meeting instructions. Private meeting access,
  place verification, expiry/audience and physical-arrival acceptance remain their
  server-owned workflows.
- Eligibility reads can become stale before dispatch; current server authorization,
  revision checks, bilateral policy, capacity and application uniqueness remain
  mandatory. A fresh private scope loses prior local command keys by design and
  never resumes or resends an old intent from storage.
