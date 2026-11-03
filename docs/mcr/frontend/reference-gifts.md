# FE-REF-GIFTS — compact giveaway presentation

The production `GiftDiscovery` presentation now uses Browse / My giving / My pickups semantic tabs and three-column information hierarchy within each image-led paper row: a quarter-width item-photo slot, item/creator/stock/public-place/pickup time copy, and a red reservation-entry control. Item titles open the existing giveaway detail route. The feature no longer adds its own title bar or window frame. The shared root owns the close control, distressed frame, body padding and single marching footer.

This is a presentation handoff from `codex/mcr-fe-ref-gifts`, based on `f837bef676cd26869c26c172a699a91b7842d832`. It changes only `frontend/src/features/gift-discovery/ui/**`, presentation-focused tests/fixtures/evidence under that feature's `__tests__/**`, the feature index's presentation type exports, and this document. Model/controller/contracts, other features, root/shared UI, manifests, lockfiles, backend, original artwork and source revisions remain unchanged. Only Frontendking integrates and pushes.

## Exact reference and visual comparison

The supplied PNG has SHA-256 `d9a8fae2f96a82253542019ab673589b3938627956eace15c8594c702b5c4617`. The target is the bottom middle GIVEAWAY phone: three compact tabs, dense bordered paper rows, a large square image on the left, condensed bold titles and red Reserve controls on the right. Its bytes are retained unchanged in test evidence; no crop, extraction, redraw or generation was performed. It is not imported into the application.

[Reference/current comparison](../../../frontend/src/features/gift-discovery/__tests__/reference-native/evidence/reference-comparison-390.png) shows the complete unchanged reference, uniformly scaled, beside the actual production component at native 390px width. Final native captures: [360px](../../../frontend/src/features/gift-discovery/__tests__/reference-native/evidence/browse-360.png), [390px](../../../frontend/src/features/gift-discovery/__tests__/reference-native/evidence/browse-390.png), [430px](../../../frontend/src/features/gift-discovery/__tests__/reference-native/evidence/browse-430.png). These use the actual component with explicit synthetic frozen-contract data and a shared `ConcertWindow` fixture surrogate; they are not screenshots of the integrated product or real stock/users/places.

The item image area occupies approximately 25% of row width. Native normal rows are approximately 146–150px tall, including 44px touch controls. Three rows fit without horizontal overflow at all requested widths. Long titles and public display names wrap instead of clipping. Pickup times display compact KST hours while accessible time labels and titles retain the complete date. Filters remain available in a collapsed disclosure below the rows. Single-page collections omit meaningless disabled pager controls; actual cursor pages retain independent previous/next actions. Styles are scoped beneath `.gifts-reference`; no global heading/paragraph rule or decorative left rail was added.

Actual gift photography is **pending root integration**. Contract `image_media_id` is respected, but no authorized gift photos were supplied to this assignment. Native screenshots therefore show an honest “No photo” area. No reference merchandise pixels, source-world illustration, generated photo, avatar, legacy fallback, fake user or invented availability was substituted. The root's marching footer and combined full-window composition are not claimed by these captures.

## Authorized image port

The index adds type-only exports `GiftDiscoveryProps`, `GiftImageRequest`, and `RenderGiftImage`. Frontendking accepted this optional prop:

```ts
interface GiftImageRequest {
  readonly mediaId: string
  readonly giveawayId: string
  readonly scope: GiftContext['scope']
  readonly alt: string
}
type RenderGiftImage = (request: GiftImageRequest) => ReactNode
// <GiftDiscovery controller={controller} renderGiftImage={rootRenderer} />
```

The UI calls the renderer only for a genuine non-null `image_media_id`, within current `GiftAccess`. The image wrapper is keyed by the full privacy scope, giveaway ID and media ID; actor/event/policy invalidation removes the row and rendered content. The root owns authorized metadata/byte reads, scope-aware shared caching, permitted URL/blob handling and synchronous cache eviction. An unavailable callback result renders “No photo”. The UI never fetches media, guesses a media URL, creates base64, maps arbitrary avatar configuration to static art, or refetches media per tab/era. The root should fit returned imagery inside the supplied square; image DOM content is constrained with `object-fit: contain`.

## Routes, eligibility and privacy

Reserve is a link to `/gifts/:giveawayId/reservation`, the existing FE26 journey. It is offered only for an open, in-stock, unended gift owned by another actor, in a mutable OPEN/LIVE event with current Gifts and mutation capabilities. It is explicitly an entry for further review: FE24 does not claim current server eligibility, reserve stock, issue POST, create a receipt, update counters or announce success. Existing detail eligibility text remains unconfirmed and now also links that real journey. Paused, sold-out, ended, closed, cancelled, own-gift and read-only states do not offer a new reservation entry.

A known current HELD/READY reservation links to `/gifts/pickups/:reservationId`; My pickups uses only the current actor's receiving projection and that existing pickup route. Expiry remains explicit. No recipient identities, private meeting instructions or confirmation codes enter public rows. My giving renders the controller's private giving projection; published owner items link to existing management. Drafts remain private and do not get a public detail link. Existing MyGifts consumers retain their original implementation.

The existing shared Tabs primitive provides roving focus, ArrowLeft/Right, Home/End and accessible tab/panel relationships. The first private collection visit loads the existing activity resource once; repeated collection switches do not reload the catalog/activity. Cursor navigation and explicit retries remain controller-owned. Account/event/policy scope changes reset local collection state and evict old content. The component does not dispose a shell-owned controller. Unknown receipts, storage recovery, atomic capacity and pickup authorization remain in the existing server-authoritative FE26 controller/journey.

## Validation

Commands ran from the assigned checkout with the existing dependencies through an authorized temporary `frontend/node_modules` symlink and pinned Node **24.22.2** (`PATH` prefixed with `mcr-tools/node-v24.22.2-darwin-arm64/bin`). No install, dependency copy, lockfile regeneration, Cargo or art generation occurred. The symlink and disposable build/cache outputs were removed before handoff. Disk free was checked before build/capture and stayed above the 8GiB floor at those operations.

| Command | Exit/result |
| --- | --- |
| `frontend/node_modules/.bin/vitest run --root frontend src/features/gift-discovery src/features/my-gifts src/app/community-entry/__tests__/routes.test.tsx` | 0; six files, 77 tests |
| `node frontend/node_modules/typescript/bin/tsc -p frontend/tsconfig.json --pretty false --incremental false` | 0; full configured source/type scope |
| `frontend/node_modules/.bin/oxlint frontend/src frontend/vite.config.ts` | 0; final source lint after disposable build output removal |
| `node frontend/node_modules/vite/bin/vite.js build frontend --outDir src/features/gift-discovery/__tests__/reference-native/build` | 0; actual application production build, 408 modules, Vite 8.1.3; existing >500kB chunk warning remains |
| `node frontend/src/features/gift-discovery/__tests__/reference-native/native.mjs` | 0; Chrome 156.0.8078.80, 18 scenario captures plus one comparison |
| `python3 scripts/mcr/check-plan.py` | 0; planning provenance only |
| `git diff --check` / final staged path/hash audit | 0 |

Native checks cover 360/390/430px at DPR1 with touch: normal, paused/sold-out/ended, read-only, long text, empty and dependency-failure states; image-slot ratios, dense layout, one fixture frame, unclipped >=44px row controls, no horizontal overflow, keyboard collections and entry navigation, no tab-switch refetch, no mutations, privacy eviction and unmount cleanup. Raw transport failure text stays hidden. Entry navigation verifies the existing route target, not a real successful reservation. Existing community-entry tests additionally exercise unknown-command recovery without replay. [Native report](../../../frontend/src/features/gift-discovery/__tests__/reference-native/evidence/report.json) records dimensions, screenshots and their hashes.

Manual inspection covered the exact full reference; normal captures at all three widths; long content at 360px; unavailable controls at 360px; dependency failure at 430px; and the final direct comparison. This is not a claim that all 18 screenshots were individually human-reviewed.

The first full type check failed because sparse provisioning omitted two JSON fixtures imported by unrelated source tests. The scrum lead authorized re-including their exact HEAD bytes; no test or compiler rule was weakened. The installed Git does not accept `sparse-checkout add --no-cone`; the operation succeeded without that flag because the checkout was already non-cone. A concurrent broad lint initially scanned disposable minified build output inside the test folder; the output was removed and the complete source lint passed. Neither condition was a product defect.

## Delivery and remaining qualification

The evidence handoff receipt records the exact base, changed paths, immutable input hashes and source/evidence receipts. Its own hash and the final commit cannot be embedded recursively; the direct handoff supplies the full commit SHA. The parent is the assigned base. No shared-root commit was cherry-picked here. Root author commit `7201b875f9ed6a07a0a77b9ae4aeac2f3f2fbd57` was reported ready by the scrum lead; combined-root integration and fidelity remain the integrator's check, not verified in this checkout.

Presentation source and deterministic fixture checks are **MOCK_READY**. Actual authorized photos/cache binding, integrated root footer/chrome comparison, real backend reservation/pickup, real devices and full multiuser/release acceptance are **NOT_RUN**. The committed production component and accepted typed port can be integrated without waiting for backend or character-art approval. Concrete related fidelity/export fixes remain in this chat's follow-up scope; unrelated work requires a new exact assignment.
