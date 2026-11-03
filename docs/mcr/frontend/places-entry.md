# FE19-ENTRY — Places lifecycle and routes

**READY for integration; MOCK_READY composition evidence.** This delivers a production adapter module, not default map/detail registration, live backend/world admission, venue verification, approved art, device acceptance or FE19/QA07 as a whole.

Exact base/parent: `fe0c3359b53bf928fd5d43fe7fdb646f9cdf03de`. Branch `codex/mcr-fe-FE19-entry`, isolated checkout `fe-places-entry`. Writable delivery is exactly NEW `frontend/src/app/places-entry/**` and this NEW document. All existing files remain byte-for-byte unchanged. Main/Delivery integrates and publishes; this worker does not push or create an additional review gate. The candidate is the commit adding these paths, discoverable with `git log -1 --format=%H -- docs/mcr/frontend/places-entry.md`.

## Exported API and ownership

`app/places-entry/index.ts` exports:

- `createPlacesEntry({ auth, source, release })`: framework-independent owner exposing `controller`, `read`, `subscribe`, idempotent `dispose` and `isDisposed`. Owns only a Places controller.
- `PlacesEntryProvider({ auth, source, children })`: stable React owner boundary, suitable for a shell lifetime rather than individual panels. Null/unqualified inputs render unavailable routes.
- `PlacesOverviewRoute`, `PlaceDetailRoute`: current unchanged `PlacesOverview` and `PlaceDetail`; detail obtains the existing `placeId` route param. No duplicate place UI or wire schema.
- `usePlacesEntry()`: current controller or null for separately composed consumers.
- `PlacesEntryRelease` / `PlacesEntryPort`: local lifecycle inputs, not a server contract or new endpoint. A stable immutable release contains captured `PrivacyScope`, owning `AbortSignal`, existing `PlaceMapPort`, and optional existing `PlaceCameraPort`. The source has stable `read()` snapshots and replacement/removal notifications.

Future root composition consumes the exported elements:

```tsx
const panels = {
  '/world/map': <PlacesOverviewRoute />,
  '/world/places/:placeId': <PlaceDetailRoute />,
}
// Wrap the stable authorized shell once:
<PlacesEntryProvider auth={auth} source={qualifiedPlacesPort}>
  {existingShell}
</PlacesEntryProvider>
```

The source must come from the **existing** world mount's qualified map and FE15 attachment. The native fixture wraps RouterProvider at an explicitly test-only composition seam and withdraws its release with the existing world mount; this is not a production root implementation or a fixture flag in the module.

## Eligibility, cleanup and retained state

The owner captures privacy scope and the exact map snapshot object. It requires current canonical identity/event/session generation, current open privacy generation, an un-aborted release, the same source binding and map snapshot. Missing/foreign/stale input is rejected before starting HTTP. Existing Places map validation checks schema, map ID/revision/content hash/geometry hash and target metadata; FE14 remains responsible for actual asset loading/hashing/qualification. No bootstrap document is cast into an admitted map.

Every Places request goes through **the existing `auth.request`**, inside `auth.privacy.run` with the captured privacy scope and combined controller/release/feature cancellation signals. There is no second HTTP client, Auth session, QueryClient, refresh policy, socket, world, game, map loader or camera in production code. The unchanged Places controller owns its endpoint/decoder/revision semantics. Bearer refresh preserves the same feature owner; identity/event/policy withdrawal ends it.

Privacy cleanup, release abort, source withdrawal/replacement and map withdrawal/replacement synchronously dispose the feature, abort pending reads, remove subscriptions and clear controller data. Late responses cannot restore old data. Reentrant withdrawal while a subscription is being installed also removes that subscription and never dispatches HTTP. A fresh explicit *genuinely qualified* binding can create a new owner; old inputs must never be restamped to claim new authority.

Ordinary map → detail → All places/browser Back keeps one controller under the same valid release. Its existing missing-target and map-mismatch latches are retained. Catalog retry does not silently undo an authoritative missing target; matching detail can reconcile it. A mismatch stays latched across navigation and same-release retry. **Reload event** still performs an actual full document reload, creating a fresh qualified map/feature lifetime. Do not manufacture a source replacement just to reset a latch.

Camera commands use only the captured existing FE15 attachment. They are refused if its object changes, disappears, throws, or lacks a live camera snapshot; observed camera loss is latched so restoring an old pointer cannot silently reenable it. A release initially lacking a camera stays camera-unavailable. The map/list remains readable when camera commands are unavailable. Look/zoom/recenter never move the avatar or establish physical arrival.

Important public-port prerequisite: `PlaceCameraPort` exposes only `read`, not an attachment-disposed signal or subscription. An attachment can be internally disposed without changing that object or its view snapshot. The producer **must abort/withdraw this release before disposing/replacing the map/camera owner, and publish every such change**. The adapter supplements that owner contract with identity/snapshot fences; it does not invent private introspection or claim to discover unpublished disposal. Native fixtures qualify a camera lasting exactly as long as the existing mounted world. Unpublished disposal in a future default producer remains an integration prerequisite.

## Existing UI and native evidence

The current published Places presentation and controller are imported unchanged. No new artwork or decorative accent rails are added. The new unavailable state is a plain semantic status paragraph. Native captures at **360×800, 390×844 and 430×932** include loaded overview, reviewed detail, latched mismatch, actual document recovery, unavailable binding and direct detail: **18 PNGs** in `__tests__/evidence/`.

The fixture adapts the existing FE19 preview from this exact base. It uses canonical SessionController/request/privacy with clearly synthetic C03/C06 metadata and terrain, the actual FE14 loader, actual Phaser 3.90.0 runtime and actual FE15 `attachMovement` camera. No production fixture mode, fallback character, venue grant, source review, clock, physical arrival or authorized product-art claim is introduced. The real default source pack and all human reference/origin files remain immutable.

Native checks prove one existing canvas and one Places owner across map/detail/browser Back; actual zoom/look/recenter with unchanged avatar foot; direct detail and current header Back behavior; mismatch latch across same-release retry; new document identity and recovered map after real Reload event; unavailable input with the existing world intact; zero page errors; no horizontal overflow; ≥44px visible Places controls; and canvas/control disposal. Synthetic unit world counters are not native socket proof, and a Phaser canvas is not live authoritative multiplayer.

Inspected captures: loaded 360, reviewed detail 390, mismatch 430 and unavailable 390. They retain the current black titlebar/ivory frame and readable source/status text. Remaining visual gates are the unchanged root shell presentation and approved map/character art; the collision schematic explicitly identifies itself as virtual rather than a physical route.

## Required root work — not implemented here

1. `app/ports.ts` / `app/world-entry/production.ts` currently expose worldAccess replication and its entry release, not a qualified public map/camera release. The actual world/composition owner must expose its existing FE14 `LoadedMap.map`, trusted expected map ID/revision/content/geometry hashes and **captured mount SessionScope**, paired with the same existing FE15 `MovementAttachment.camera`, captured current privacy scope and an abortable lifetime. Publish nothing until those inputs are qualified. Abort/remove before stop, failed initialization, policy change, attachment replacement or disposal. Asset/admission/owner checks stay with their existing owners.
2. Mount `PlacesEntryProvider` once in the stable authorized shell and register its two route elements in reserved `router.tsx`/the existing root composition. `main.tsx`, `createAppRouter`, WorldShell and ShellPorts are unchanged in this delivery. The `composition.panels` test seam proves route compatibility but does not make either route reachable in the default application.
3. The existing route table gives `/world/places/:placeId` parent `/world`, so header Back currently returns to World. Actual browser Back from map-opened detail and All places return to the map. To align header Back with detail-before-map navigation, Delivery must explicitly change only that reserved route's parent to `/world/map`. This module does not override the shared handler or silently edit `routes.ts`.
4. Qualify the above producer/registration against the real admission/asset/privacy lifetime and live BE08. This module cannot qualify inputs that the current production root does not publicly expose. Venue/source/device/multiuser/IN05 gates remain separate.

## Executed validation

Pinned Node **v24.22.2**, retained Vitest **4.1.9**, native Chrome **156.0.8078.80**, Phaser **3.90.0**. Temporary `frontend/node_modules` symlink consumed retained dependencies; no install, dependency/lockfile edit or shared build-info write. Vite uses config runner and a private `/tmp/mcr-places-entry-build`; tests use `--no-cache`, TypeScript `--incremental false`. Link removed before final handoff.

| Command/check | Result | Exit |
| --- | --- | --- |
| Focused Places + new entry + affected shell/routes | **150 tests, six files**, zero failed/skipped | 0 |
| Strict combined frontend TypeScript | No diagnostics | 0 |
| Strict combined frontend lint | No diagnostics | 0 |
| Native runner: production app + fixture build and phone checks | 18 screenshots, three widths, zero page errors, cleanup verified | 0 |
| C08 source-origin guard | Static origin/import checks pass | 0 |
| Initial planning consistency | Provenance/export checks pass; not application acceptance | 0 |

Commands from checkout root with pinned Node directory on PATH; lint runs inside `frontend/`:

```sh
node frontend/node_modules/vitest/vitest.mjs run --root frontend --configLoader runner --no-cache src/app/places-entry/__tests__ src/features/places/__tests__ src/app/__tests__/shell.test.tsx src/app/__tests__/routes.test.ts
node frontend/node_modules/typescript/bin/tsc --noEmit --incremental false --project frontend/tsconfig.json
# From frontend/:
node node_modules/oxlint/bin/oxlint src vite.config.ts
# From checkout root:
node frontend/src/app/places-entry/__tests__/native.mjs
node scripts/mcr/check-ui-origin.mjs
python3 scripts/mcr/check-plan.py
```

The native runner bundles both actual app entry and the new fixture; the existing >500 kB Phaser chunk warning remains. No broad 96-file suite was run. Final validation was rerun after the additional camera snapshot/loss guard. Earlier failed new-test/type outputs remain in evidence: an initial policy UI assertion observed the feature fallback before shell rebootstrap committed, and unsupported Testing Library `exact` options failed strict types. The corrected policy test asserts synchronous owner/data eviction and then awaits the actual fallback; no existing test was weakened or skipped.

## Provenance and hashes

Production owner/provider/routes are newly authored using existing public types and components. Canonical owner patterns were inspected in account-entry/world-entry; no legacy rendering was copied. Test-only preview adaptation source: `frontend/src/features/places/__tests__/fixtures/preview.tsx` at `fe0c3359b53bf928fd5d43fe7fdb646f9cdf03de`, destination `app/places-entry/__tests__/preview.tsx`; it replaces the preview's independent bare HTTP/session/controller with the canonical Auth/new entry while retaining its already synthetic map/runtime/camera qualification. Dependencies are already retained React/React Router/Vitest/Testing Library/Playwright/Vite/Phaser; none added.

New source SHA-256:

| Relative to `frontend/src/app/places-entry/` | SHA-256 |
| --- | --- |
| `__tests__/fixture.ts` | `a319f5b5e7b2acd5228eeca037778e32f872264640ee8eb62dc4e5f3f8d07841` |
| `__tests__/native.mjs` | `e3c9633c767d826b347a89afa2bfed5bcf7b4916218bb009622382b4c8c7e192` |
| `__tests__/owner.test.ts` | `e621adc234aa7285bf89374a0098a93e0719897bb3ffa80218d1002c7bab39e7` |
| `__tests__/preview.html` | `cbef6699c50fe409740e471589c414d20ef7613e49a58829e805d67342bae8f3` |
| `__tests__/preview.tsx` | `ef3b9ac2c979967576f3d963a7271d1b518a227dbbf6581420fe0a26747189c5` |
| `__tests__/routes.test.tsx` | `f60eeaba7e72f8216432d7f95df2437a03acb04d791f206c6074900eb7004c64` |
| `index.ts` | `bf86889f5bc5dc3ffb560dd65212261a0d76e4e07186dc0e78ce9c0a66b05cd5` |
| `owner.ts` | `d8dc03c6fa7e08a1bc22504fe6f84140c886970a9a4b697759ab5e66b381ddf9` |
| `provider.tsx` | `ca59285cd6b4556a0f3b59144288ef9ae1bda6e974cfe704581ea48317e33e4c` |

`__tests__/evidence/validation.json` records all new source hashes, 37 unchanged source/contract/reference pins, every screenshot/log/native report hash, command exits, provenance and remaining gates. Initial failure logs are historical intermediate outputs, not seals for the final source. Authored code/document/manifest whitespace is checked separately from exact raw command logs, whose original trailing blank lines are preserved.

Before committing, the worker verifies exact NEW-only paths, parent ancestry, unchanged protected inputs and all evidence bindings, removes the dependency symlink and confirms a clean committed handoff. Fresh actual `get_goal` returned aligned **active** with no token budget during implementation. Completion covers this bounded adapter delivery and concrete scoped fixes; default-root registration and real producer exposure remain explicit independent gates. Stop the writer on READY; Main/Delivery publishes.

## Main integration checkpoint

Author handoff `4809fad3c890eb6b4744e437bedf3ebd4a16e62d` was applied to published base `17adc599302109a3e9a44fb114c44663fed4c9d4`. Main verified all 38 assigned new paths, all 73 author source/reference/evidence hashes against the author checkout, and exact integrated file bytes before adding this paragraph. All 36 executable/contract/reference inputs still match in the combined tree; the sole newer read-only pin is the UI audit prose, updated for the subsequently published Customize presentation. No cross-lane implementation imports were found. Fresh whole-frontend strict TypeScript (`--noEmit --incremental false`) exited 0 in 4.53 seconds; retained dependencies were used temporarily and the symlink was removed. The existing 150-test and 18-capture evidence is retained without repeating the owner's review. The author verified goal completion and a subsequent null goal, then stopped its writer. The root composition and live qualification gates above remain open.
