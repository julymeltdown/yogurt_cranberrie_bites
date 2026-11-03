# FE-ASSET-LEASE: account-owned reviewed atlas images

Status: implemented account-scoped checkpoint; live-service and shared World-renderer acceptance remain NOT_RUN. Assignment base: `26a8eb61afc1bfc5310c1628543691b4e725e840`. Delivery branch: `codex/mcr-fe-asset-lease`, target `julymeltdown/mcr:master`.

## Behavior and ownership

`createAccountOwner` owns one `createReviewedAtlasLeases` instance for its original Auth privacy scope. A pool survives ordinary account panel unmounts, while each preview/thumbnail compositor and its DOM canvases remain independent. The pool retains only verified decoded atlas images and immutable manifest/policy metadata. It contains no private outfit keys, credentials, canvases, identity controller, persistent storage, or global cache.

Every acquisition validates the manifest document/hash/pin and complete reviewed binding before a cache hit, then checks the captured owner scope again after asynchronous validation and loading. The owner closure checks original user, event, session generation, policy generation and open privacy gate. The key includes resource-port identity, manifest pin and the complete binding receipt. Mutable document, pin and binding inputs are snapshotted before asynchronous validation. A changed source/provider requires owner replacement, rather than silently authorizing old resources under a new scope.

One owner signal controls a coalesced read/hash/decode. Caller aborts reject/release only that caller; they do not cancel another consumer or the retained warm image. Completed and pending acquisitions are reference counted independently. Released images remain warm within the bounded owner cache. Default limits are two entries and 32 MiB of reserved decoded RGBA pixels, including in-flight reservations; hard configurable ceilings are four entries and 64 MiB. Eviction selects only decoded entries without active or pending consumers. Failure removes the entry for retry. Privacy cleanup/owner disposal aborts the owner signal, disposes dependent compositors and images, and rejects late stale completion. Consumer and owner disposal are idempotent.

The existing compositor remains unchanged: original layer order, 320×256 composed sheets, 64×64 pose thumbnails, smoothing disabled, validated tones/fallback policy, and at most 50 composed entries per consumer. Closing an editor clears its private composed canvases while retaining only the decoded atlas. Existing local draft retention, server reconciliation, save conflicts/uncertainty and HTTP force-cache behavior predate this change and retain their existing meaning.

## Public source-pool port

The runtime exports `createReviewedAtlasLeases` and `ReviewedAtlasLeases`. `createReviewedAvatarCompositor` accepts an optional `leases` property. Existing callers without it retain their previous standalone lifecycle. Passing the owner pool shares the verified decoded image, while the returned compositor is still exclusively owned by the caller and must be disposed.

The account owner publishes the retained port as `owner.source.options.leases`; `owner.source` also carries its original `scope`, exact manifest/pin, binding and resources. Forward the whole source, without recapturing or replacing its scope. Account thumbnails/editor automatically use this source. `ProfileAvatarOptions.source` optionally accepts the same `{ scope, options }`; it checks the source scope before borrowing, and otherwise retains the existing required `resolve` fallback. Root composition owns forwarding this optional source into other consumers. This patch changes neither root ports nor the World adapter.

The guarantee is account-only: Header/Me/Customize can reuse one image across World-route boundaries. Actual World renderer decode sharing is NOT_RUN and requires an explicitly coordinated bridge. The selected whole-character variant contract belongs to a separate runtime owner; a selected whole-front ID is not an `AvatarConfig`. No static-to-layered mapping, artwork binding, fallback artwork, contract, dependency or source-pack change is introduced here.

## Paths and provenance

New runtime logic: `frontend/src/entities/avatar/runtime/atlas-lease.ts`. Adapters: runtime `index.ts`, account-entry `owner.ts`/`assets.ts`, avatar `preview.ts`, Me `AvatarThumbnail.tsx`, profile `Avatar.tsx`. Existing controller/compositor, AvatarEditor, World/map byte cache/resolver, root ports, contracts, central manifests/locks, artwork and `docs/source/` are untouched. All logic is newly authored against existing C06 validators/resources and canonical Auth privacy lifecycle; no external code copied and no new dependencies.

Tests are confined to `frontend/src/app/account-entry/__tests__/asset-leases/`. Their catalog explicitly substitutes a synthetic test pack and source, so test receipts cannot authorize any selected production artwork. The browser build aliases that catalog only in the isolated test entry. Synthetic geometric PNG generation reuses the existing `world/avatar/__tests__/fixtures/png.mjs` fixture unchanged. The source pack and supplied originals retain their original bytes.

## Validation

Pinned Node 24.22.2 and retained dependencies; no install or lock reconciliation. Commands run from `frontend/` unless noted, with the pinned Node bin directory prepended to PATH:

- `node node_modules/vitest/vitest.mjs run src/app/account-entry/__tests__/asset-leases/leases.test.tsx`: exit 0, 18 tests. Covers coalescing, caller abort independence before/after acquisition, all-waiters abort, warm reuse, independent canvases, capacity/eviction, invalid receipts/pins/hashes/resources, corrupted bytes/decode failure retry, pre-abort, late ignored-abort results, user/event/session/privacy invalidation, retained drafts and profile borrowing/stale-source rejection.
- `node node_modules/typescript/bin/tsc --noEmit --pretty false`: exit 0.
- `node node_modules/oxlint/bin/oxlint src vite.config.ts`: exit 0 after removing a test-harness unused import.
- `node node_modules/vite/bin/vite.js build`: exit 0; existing large Phaser chunk warning remains.
- `node src/app/account-entry/__tests__/asset-leases/native.mjs`: exit 0 at 360/390/430 px. Production-built isolated test entry, actual production account owner/editor/thumbnail/profile components, real Chrome image decode. One read/decode over account World-boundary → Me → Customize → World-boundary → Customize → Profile; unsaved draft pixels retained, original pixels equal an unpooled baseline, smoothing disabled, zero browser errors. A separate unpooled comparison adds one deliberate decode; both object URLs and all captured canvases are released on teardown. Evidence: `/tmp/mcr-fe-asset-lease-evidence/result.json` and `profile-{360,390,430}.png`.
- Root `python3 scripts/mcr/check-plan.py` and `node scripts/mcr/check-ui-origin.mjs`: exit 0; planning checks verify archived source checksums, and origin checks report no violations.

The initial combined affected run passed 260/261 tests (25 files); `account-entry/__tests__/root.test.tsx:20` intermittently timed out finding its still-valid `Synthetic listener` heading during cold lazy imports. The same failure reproduced with the six changed existing product files restored to assignment-base bytes (`/tmp/mcr-fe-asset-lease-baseline-root.log`). All five root tests subsequently passed standalone without source/assertion changes (`/tmp/mcr-fe-asset-lease-root-detail.log`). Record this cold-load sensitivity separately from the new lease tests; fresh-remote reconciliation checks follow the source checkpoint.

This evidence certifies synthetic account resource lifecycle and rendering parity only. It does not certify selected character approval, actual World renderer sharing, authenticated backend integration, multiplayer behavior, deployment, or IN05 acceptance. Builds/captures check at least 8 GiB free first; initial disk checks were above that threshold.
