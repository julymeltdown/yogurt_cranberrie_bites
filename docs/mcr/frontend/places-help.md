# FE19 — map overview, place detail and controls help

Status: **MOCK_READY for integration review**, not BE08, venue, device or multiuser acceptance. Base `bed5ffa8cb52f5dc337bcde7d50a428f9bce74f9`, branch `codex/mcr-fe-FE19`, isolated checkout `/Users/lhs/Desktop/mcr-worktrees/fe-places-help`. Only `frontend/src/features/places/**`, `frontend/src/features/world-help/**` and this document are delivered. All UI is newly authored; no legacy rendering, character assets, dependencies, backend or central composition changes.

## Behavior and contract pins

- W02 `/world/map`: schematic overview from the existing validated C06 collision grid/interactions, numbered landmarks and a complete DOM list, search, virtual/physical legend, camera look/zoom/recenter, loading/empty/error/retry and mismatch recovery. Camera actions never move the avatar or confirm arrival. Closed targets are disabled. A missing camera leaves the list accessible.
- W03 `/world/places/:placeId`: current detail and accessibility notes, concept/verified/unverified/closed/source-unavailable presentation, source revision/reviewer/date/author/license/photo permission, safe source link, explicit all-places/help navigation. A 404/410 is unavailable/possibly removed, not a fabricated `REMOVED` DTO state. There is no physical arrival, beacon mutation or destination-selection action.
- W08 `/world/help`: existing FE15 keyboard, touch, nearby interaction and camera controls, focus/input cancellation, release recovery, explicit virtual/physical separation and meeting audience/revision/confirmation/expiry guidance. No new movement bindings are installed.
- C03 generated core at `153a3dc9d1ba11facead968e4980adbd659aaf89`: `listConcertPlaces` and `getConcertPlace`, authenticated GET `/api/concert/events/{eventId}/places` and `/places/{placeId}`, `PlaceList` / `PlaceResponse`, no-store and existing Auth coordinator policy. Error categories come through FE01's safe `TransportError`; raw server messages/bodies are never presented.
- C06 schema origin `12acc2d7a93701d89aad63ea7c3376f91b7fcb13`, shared map/Place/Provenance definitions. FE14/FE15 public runtime/camera baseline includes `d91ee0209c557a1e6b3e7053be5d7974a2e46b3a`; exact input hashes are in the native report. No contract or shared type was amended.

`PlaceList.revision` is a catalog snapshot revision, **not** `map_revision`. Detail must match the selected canonical UUID, current event, catalog revision and complete catalog item. Geometry targets additionally require the separately injected current/expected C06 map identity and owning canonical user/event/generation to match. Catalog items must resolve to map IDs with matching kind, label, source ID and source revision. Changing closure/verification can restrict the latest read without guessing a new coordinate. Both catalog and map closure must be open for a target. Unknown map associations, changed source identity, detail disagreement, stale release scope or map-version errors pause targeting. Map mismatch offers **Reload event**, a normal browser document reload that reopens the canonical app/bootstrap and map owner; a catalog-only retry cannot fix an old pin. It may require sign-in under the existing Auth policy, and does not claim the server release changed. Other transient/revision failures retain the scoped Reload places action. Independent review R1 found that a detail 404/410 or typed map-version error could be bypassed by returning to the overview. Individual missing-place errors now retain only that target's invalidation across unselect/navigation and catalog-only retries; matching successful detail clears it, and successful catalog omission prunes it. Unrelated targets and camera controls remain usable. A typed map-version rejection retains release-wide invalidation through navigation and same-pin catalog retries, including camera/recenter/zoom. Only a new matching release/catalog pair or a fresh canonical owner through Reload event reconciles that release; catalog revision remains distinct from map revision. Latches are scoped to the canonical user/event/generation and cleared on scope change or disposal. Original failing evidence is preserved at `/tmp/mcr-fe19-review-original/`; the same probe logic against corrected source records all three after-Back target calls rejected at `/tmp/mcr-fe19-r1-probe.json` (worker replay, not independent review closure). Every reload clears old metadata before awaiting; scope abort, map replacement, selection changes and disposal reject delayed responses, even if the lower adapter ignores cancellation. No private persistent cache is added.

The feature-local Zod decoder consumes the generated public shapes and rejects unknown fields, malformed/cross-event data, duplicate IDs, inconsistent physical/concept/review metadata and oversized lists. Synthetic fixture provenance cannot verify physical places. A source URI being schema-valid does not make it a browser destination: only bounded explicit HTTPS without credentials, control characters, whitespace or backslashes is linked, with `noopener noreferrer` and no referrer. Other source values display an unavailable state. No raw unsafe URI is rendered.

C03 provides no photo URL or route instructions. The detail therefore states that no authorized photo is available and uses the supplied accessibility notes; `photo_permission: GRANTED` alone cannot manufacture a photo. No real venue facts, routes, travel time, GPS or admission claims are introduced.

## Integrator-owned wiring

Exports: `createPlacesController`, `PlacesOverview`, `PlaceDetail`, `PlaceLegend` from `features/places`; `WorldHelp` from `features/world-help/WorldHelp`. Create one feature controller per active shell owner and dispose it when that owner ends. Components subscribe/start idempotently, but do not dispose the shared controller on panel navigation.

```tsx
const places = createPlacesController({
  session: auth.session,
  http: { request: auth.request }, // FE09 retains refresh/expiry/cross-tab policy.
  map: mapPort,
  camera: { read: () => activeMovementAttachment?.camera ?? null },
})
// Mount these through createAppRoutes(shellPorts, { panels: ... }):
const panels = {
  '/world/map': <PlacesOverview controller={places} />,
  '/world/places/:placeId': <PlaceRoute />,
  '/world/help': <WorldHelp />,
}
function PlaceRoute() {
  const { placeId = '' } = useParams()
  return <PlaceDetail controller={places} placeId={placeId} />
}
```

`PlaceMapPort.read()` returns `null` until the existing FE14 owner has validated its release, then an immutable `{ scope, manifest, expected }` snapshot. `scope` is the **captured world mount** `SessionScope`, never a later identity read. `manifest` comes from that owner's `LoadedMap.map`; `expected` contains `{ mapId, revision, contentHash, geometryHash }` from its trusted release pin. The subscription must publish every replacement/removal, including stop/disposal. FE19 checks schema/identity/scope defensively; asset content hashing, geometry qualification and lease ownership remain FE14's responsibility. The camera port must resolve the same live owner's FE15 `MovementAttachment.camera`. Do not construct another game, camera, session, HTTP client or socket for these panels.

Use `createBrowserRouter(createAppRoutes(...))` for this composition. The current one-argument `createAppRouter` convenience helper does not accept panels. Root route/port wiring is not in this worker's ownership. Current `routes.ts` assigns the place route's parent to `/world`; header Back therefore closes to World. To make header Back return to the map opener, the integrator should set only `/world/places/:placeId`'s parent to `/world/map`. The feature's All places link already works; actual browser Back focus restoration to the original list link is independently exercised. Existing shell panel focus, input blocking, safe areas and scrolling remain the owner.

## Executed validation

Pinned Node 24.22.2 / pnpm 11.7.0, installed Chrome 156.0.8078.80. Commands run from `frontend/` unless indicated:

| Command | Exit / evidence |
|---|---|
| `corepack pnpm@11.7.0 install --offline --frozen-lockfile` | 0; `/tmp/mcr-fe19-install.txt`; no manifest/lock changes |
| `corepack pnpm@11.7.0 exec vitest run src/features/places/__tests__` | 0; **61 tests / 2 files**, `/tmp/mcr-fe19-r1-tests.txt` |
| `corepack pnpm@11.7.0 build` | 0; strict TS + production build, `/tmp/mcr-fe19-r1-build.txt`; retained 544.82 kB main-chunk warning |
| `corepack pnpm@11.7.0 exec oxlint src vite.config.ts` and runner `node --check` | 0; no lint diagnostics, `/tmp/mcr-fe19-r1-lint.txt` |
| `MCR_FE19_EVIDENCE=/tmp/mcr-fe19-r1 node src/features/places/__tests__/browser-smoke.mjs` | 0; three phone widths, `/tmp/mcr-fe19-r1-native.txt`, `/tmp/mcr-fe19-r1/report.json` |
| Root C08, plan, exact frontend boundaries and whitespace | Exit codes and artifact hashes in the accompanying sealed handoff receipt |

Core probes cover concept/verified/unverified/closed/source unavailable; unsafe URI and contradictory metadata; canonical HTTP delegation with no independent refresh flag; invalid/duplicate/cross-event data; list/detail revision disagreement; release identity/scope mismatch and recovery; empty/error/explicit retry; logout/account/event changes, ignored aborts, selected-detail races and reentrant disposal. Rendered UI tests cover source safeguards, actual states, search and help semantics.

The native fixture uses the real shell/panel composition, FE14 loader and Phaser 3.90.0, FE15 movement/camera APIs, synthetic terrain and deterministic C03 response bodies. At **360×800, 390×844, 430×932** it proves one persistent canvas across overview/detail/help, actual camera zoom/look/recenter, unchanged avatar foot, keyboard disclosure/navigation, destination-heading focus and browser-Back opener focus, ≥44px feature controls and disposal of canvas/controls. At 390 it additionally exercises all five place states, search, empty/error/rate/denied/removed/revision/mismatch recovery, including R1 navigation away from missing/mismatched details, same-catalog retry retaining disabled targets, unrelated-target availability, matching detail reconciliation and an actual Reload event navigation: the document identity changes, a fresh FE14 owner loads, the stale fixture pin is gone and exactly one current canvas remains. Persistence is asserted across panel navigation within each document; the explicit reload intentionally creates a new owner. Screenshots `overview-390.png`, `detail-390.png`, `help-390.png`, `mismatch-390.png` are bound in the receipt; mobile layouts were visually inspected.

The harness reserves its own ephemeral port with strict binding and closes Chrome/Vite. No Cargo/PG/Rust target, real account, live API, external source navigation, real map or character art is used. Initial failures are preserved: one premature test wait, initial strict typing errors, and native runs assuming header Back returned to map. Native runs first/second/third stop at that root-parent expectation; fourth and later intermediate runs pass their then-current bytes. The initial committed handoff and its sealed receipt remain unchanged as historical evidence. R1/recovery follow-up evidence supersedes it after adding authoritative invalidation latches and the actual full-document map recovery action; scope binding, compact legend and canonical Auth-request delegation remain covered. No broad frontend/world matrices were rerun.

Limitations: actual BE08 producer, approved venue/source truth and photos, production shell/world wiring, authoritative multiplayer, real devices and full FE19/QA07 acceptance are **NOT_RUN**. Only the integrator publishes; no push. Goal stays ACTIVE through review and required fixes.
