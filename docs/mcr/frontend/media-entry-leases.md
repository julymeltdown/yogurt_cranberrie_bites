# FE-MEDIA-LEASE — scoped retained media previews

Status: **MOCK_READY**. The pure adapter and synthetic browser proof are delivered. Actual Feed/Gift row bindings, live authorization, shell readiness, physical devices and multiuser acceptance remain **NOT_RUN** here and belong to their existing owners.

## Ownership and provenance

Assigned base: `335066396502782426c4b86ae8aacb9704def988`, branch `codex/mcr-fe-media-lease`, reused isolated checkout. Explicit root delegation grants only new `frontend/src/app/media-entry/**` and this document. No existing FeedImage, Gift UI, public UI, Auth, shared reader, ports, manifests, lockfiles or backend files are modified. `docs/source/` remains byte identical.

The adapter composes the existing `shared/media/read-media.ts:createMediaPreviewController`, `shared/media/contract.ts`, image policy and browser preview port. It imports the canonical SessionController type and routes both protected reader requests through `auth.privacy.run(fixedScope, …, readerSignal)` and canonical `auth.request`. It preserves metadata/context/status validation, exact same-event protected read path, purpose, bounded response type/bytes, PNG/JPEG/WebP headers, dimensions and metadata/byte agreement. It adds no server endpoint, parent query, authentication identity, readiness producer or transport. Source revisions and SHA-256 pins are recorded in the owned qualification evidence. Unit PNG bytes reuse the existing synthetic reader-test fixture; browser photos are newly generated 96×64 canvas fixtures, never product/user artwork.

## Root API and lifecycle

```ts
const scope = auth.privacy.capture()
const owner = createMediaOwner({ auth, scope, maxEntries: 8, maxConsumers: 64 })
const createPostPreview = (mediaId: string) => owner.acquire({ mediaId, purpose: 'POST' })
const createGiftPreview = (mediaId: string) => owner.acquire({ mediaId, purpose: 'GIVEAWAY' })
```

Pass a stable `createPostPreview` into existing FeedImage. GIVEAWAY leases expose the same structural `read/subscribe/load/cancel/dispose` interface for the root GiftImage owner; `release()` is an idempotent terminal alias for `dispose()`. Every rendered consumer acquires its own facade and disposes it on effect cleanup. `cancel()` detaches that facade, clears its visible state and may be restarted with `load()`; an optional caller AbortSignal releases it terminally. Consumers must use only Blob URLs from ready state, retaining their existing identity/purpose/path and accessibility checks. The adapter does not change FeedImage markup or author a root GiftImage component.

Keep one owner above route/component lifetimes for its complete captured PrivacyScope. Successful route unmounts release facades while the owner retains their verified entry; a later explicit consumer load can reuse it. Root must supply media IDs from an actually accessible, current parent post/giveaway row. `acquire` alone performs no read; `load` makes the existing media requests. Possessing or guessing a UUID does not prove accessible parent ownership. Server checks remain authoritative, and this cache does not invent a parent authorization claim. Do not prewarm guessed IDs or use it to restore stale parent rows.

Keys include generation, user, event, policyGeneration, gate, purpose and media ID. POST and GIVEAWAY never reuse one another's entries even for the same ID. Full `privacy.isCurrent(scope)` gates construction, acquisition, loading and state publication. The owner registers synchronous Auth privacy cleanup and becomes terminal on **every** policy or identity advance, including same-user open-gate changes. Root then creates a fresh owner from a new valid scope. Explicit owner disposal and cleanup cancel readers, revoke all retained/visible object URLs, clear consumer state and reject late work. No owner, private Blob, URL or metadata is persisted or dehydrated.

Shared reads have one underlying reader per key. Each consumer awaits either that flight or its own cancellation. Aborting/releasing one facade does not abort another's reader; releasing the final in-flight consumer disposes its entry. Failed reads can retry and are not retained after their final consumer leaves. Reader notifications can safely re-enter load or release without starting duplicate flights. Faulty observer callbacks cannot prevent other private state from being cleared.

## Bounds and capacity behavior

All entries, including active/in-flight entries, count toward `maxEntries` (default 8, allowed 1–32). Only entries without consumers can be evicted, in least-recently-used order, with URL revocation. If every slot is held, a new lease's load returns failed state `media-capacity` without a request; that same facade can retry when capacity becomes available. Never evict a held image to make room.

Owner-held facades count toward `maxConsumers` (default 64, allowed 1–256), including acquired idle facades. Acquisition beyond that cap throws RangeError before registering a facade or reading anything. Root must bound its mounted image count, release stale consumers, and select explicit limits for its row window. Invalid IDs/purposes/limits and stale scopes are rejected; REPORT is outside this adapter's public purpose union.

Each entry's private Blob is bounded by existing `MAX_IMAGE_BYTES` = 10MiB. The default resident Blob ceiling is **8 × 10MiB = 80MiB**; the largest allowed configuration is **320MiB**. `inspect()` exposes counts and this worst-case Blob ceiling, not an allocation measurement. HTTP buffers, temporary header inspection ArrayBuffers, JavaScript overhead and browser image decoding/GPU surfaces are additional memory. Existing 4096px/16-million-pixel limits remain enforced. No decoder-cache, total-process-memory or zero-decode claim is made.

## Validation and measured limits

Commands and exits are recorded in `frontend/src/app/media-entry/__tests__/evidence/qualification.json`. The initial focused run passed 56 tests across the new owner, existing shared reader and privacy coordinator. The final expanded run passes 74 tests in four files, also including canonical Auth session regressions. It covers inert acquisition, cold/warm sharing, same-ID purpose denial, valid GIVEAWAY, retry, independent caller abort and early promise settlement, final-reference abort, ignored late responses, LRU and held-entry pressure, idempotent release, identity/event/policy revocation, fresh policy owners, reentrant observers and bounded registration. Strict TypeScript, scoped lint, production Vite build and plan checks pass (exit 0). The existing large-chunk build warning remains; an application build cannot prove the adapter's future root binding.

The browser fixture uses actual SessionController/session/privacy, shared protected HTTP reader and production FeedImage. Its local server authorizes only explicit synthetic identities and known parent-row media IDs. It serves a real 4,453-byte synthetic PNG with `Cache-Control: no-store`. A cold POST consumes one metadata request and one content request; leaving and remounting the POST reuses the same Blob URL with **zero additional HTTP requests or object-URL creation**. GIVEAWAY performs its own metadata/content pair. Browser Resource Timing and server response-body counts are recorded independently; these are loopback fixture measurements, not production transfer claims or HTTP-cache savings.

Two URLs are created; policy cleanup revokes both, removes the current image, empties the owner and makes fetching the old Blob URL fail. Three explicit `img.decode()` calls qualify cold POST, warm POST and GIVEAWAY display. They do not count every internal browser decode or prove decoder-cache reuse. `synthetic-gift-390.png` and `revoked-390.png` show the actual fixture before/after cleanup. No production sign-in, readiness, live photo authorization or actual GiftImage binding is claimed.

The worker removes its own temporary dependency symlink, build/Vite/TypeScript caches and closes its own browser/server at final handoff. Fresh remote history is reconciled before non-force publication; prior PUBLIC/consumer/art commits are not republished as new worker patches.
