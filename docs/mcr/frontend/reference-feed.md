# FE-REF-FEED presentation handoff

Status: **MOCK_READY** for the owned presentation, validated with production components and deterministic transport. This does not accept FE27, IN03, the combined root, or a real multiuser flow.

## Revision and ownership

- Base: `f837bef676cd26869c26c172a699a91b7842d832`, branch `codex/mcr-fe-ref-feed`. The delivery commit is the commit adding this document; its parent must be this base.
- Owned production paths: `frontend/src/features/event-feed/{EventFeed.tsx,FeedImage.tsx,FeedGlyph.tsx,presentation.ts,feed.css,index.ts}`. Owned tests/evidence: `frontend/src/features/event-feed/__tests__/presentation.test.tsx`, the existing `__tests__/visual-adoption/adoption.test.tsx`, and `__tests__/reference/**`.
- `index.ts` changes only the presentation type export `RenderPostAuthor`, explicitly coordinated with Frontendking. Controller, contract, runtime, shared files, root composition, manifests, retained lockfile, backend and `docs/source/` are unchanged.
- No copied legacy rendering or character artwork. The icon paths and presentation helpers are newly authored. Existing headless controller/privacy behavior is reused through its original interfaces.
- Contract/runtime provenance: the three unchanged source SHA-256 values and the eight rendered-source hashes are recorded in `browser-report.json`. All resolve from the base above. Retained `frontend/pnpm-lock.yaml` SHA-256: `47ca44459819ad0e887d2f4f93394fdf3b1053b8f45edc9ceb0236981feee1f6`; its source revision is `605b858b3baa2bb93ce4a01a6620cd25bc648c14`.

## Reference and implemented behavior

Reference: the user's supplied 1536×1024 multi-screen image, bottom-left FEED panel, approximately x=5..299/y=540..1020. Original SHA-256: `d9a8fae2f96a82253542019ab673589b3938627956eace15c8594c702b5c4617`. The supplied original was viewed and left untouched; it is not copied into the repository.

The browse presentation uses the reference's four adjacent tabs, two-column portrait/name layout, relative age, two-line copy, wide media inset aligned with the copy, small star/comment/share row, close article spacing and red NEW POST button. Symmetric borders and spacing provide structure; no decorative left accent bars are introduced. All visible feature buttons and the options summary retain at least 44×44 px hit areas. Reduced motion and visible keyboard focus are supported.

The feature adds no frame, duplicate title, close control or global page introduction. Shared root owns those. The native harness uses the existing ConcertWindow solely to qualify the feature in an approximate frame; its host geometry is not evidence for the new combined root.

Filter mapping preserves the frozen contract: ALL → `ALL`, MEETUP → `PARTY_LINK`, GIVEAWAY → `GIVEAWAY_LINK`; NOTICE selects already loaded notices locally. The existing `WITH_MEDIA` filter remains reachable as Images only in Feed options. Returning NOTICE → ALL while ALL remains selected does not refetch the feed. Switching actual contract filters continues to use the controller's existing behavior.

Counts come directly from each Post DTO. Reactions, comments and the sharing affordance open the existing thread callback. They do not invent a reaction mutation, share URL or share count. Compose and related-content callbacks remain intact, including removal of revoked links. Notices retain their source URI and confirmation timestamp. Read-only, unavailable, empty, pending, error, retry and pagination states remain reachable. Canonical invalidation removes private author/media content and compose controls.

`FeedImage` changes only the rendered image class and native lazy/async decode attributes; purpose, event/user/generation, ID, content-path, blob URL and cancellation checks remain unchanged. The existing `renderMedia(mediaId, post)` seam is preserved.

## Caller integration

`renderAuthor?: (author: Post['author']) => ReactNode` receives the exact public author and versioned AvatarConfig. The caller must resolve the authorized selected-static consumer and reviewed derivative metadata for that exact config. The 48 px square accepts an image/canvas/element. If no renderer exists, a readable initial explicitly labeled avatar unavailable appears; no legacy or guessed character fallback is used.

No avatar or counter requests are introduced. The shared selected atlas/metadata cache, exact config mapping, and era/tab reuse belong to root/art integration. No base64 atlas or per-row avatar URL is fabricated. Existing scoped media authorization/cache remains caller-owned; a component remount must not be interpreted as proof that a root cache is correctly bound.

Frontendking owns serial integration, the root frame/body and the portrait/media bindings. Root commit `7201b875f9ed6a07a0a77b9ae4aeac2f3f2fbd57` was reported by Community SL during this handoff and is **not** included or tested in this isolated branch. No rebase/cherry-pick of shared root work was performed here.

## Validation

Use Node `24.22.2`, retained dependencies and the existing lockfile. No install or dependency copy was performed. An authorized temporary node_modules symlink pointed at the integration checkout for validation and is removed before clean handoff. Build/capture checked the 8 GiB available-disk floor. The harness builds under its own `/tmp/mcr-fe-ref-feed-build`, disables public-directory copying, and closes its browser and server. There is no Cargo operation.

Run from the repository root with the pinned Node binary directory on PATH:

| Command | Exit | Result |
| --- | --- | --- |
| `node frontend/node_modules/vitest/vitest.mjs run --root frontend --no-cache --maxWorkers 1 src/features/event-feed src/features/feed-thread` | 0 | 38 tests in five files; includes existing controller, media/privacy, thread and StrictMode regressions plus six presentation cases |
| `node frontend/node_modules/typescript/bin/tsc --noEmit --incremental false --project frontend/tsconfig.json` | 0 | Strict frontend typecheck |
| `node frontend/node_modules/oxlint/bin/oxlint frontend/src/features/event-feed` | 0 | Scoped lint, no warnings after removing the unused harness binding |
| `node frontend/src/features/event-feed/__tests__/reference/browser.mjs` | 0 | Isolated production Vite build plus actual installed Google Chrome `156.0.8078.80`; 360/390/430 px PASS |
| `python3 scripts/mcr/check-plan.py` | 0 | Planning/provenance checks, including all 13 original archive checksums |
| `git diff --check` | 0 | Whitespace check |

The retained runtime resolved Vite `8.1.3`. Its existing large-chunk advisory remains; this feature does not change bundler composition. The production build above compiles the actual EventFeed/FeedImage and their imported dependencies in an isolated qualification entrypoint. A complete application release build is **NOT_RUN** in this branch.

Native checks cover decoded media, exactly four keyboard-operable tabs, notice source, thread/compose callbacks, focus, 44 px targets, no horizontal overflow, all five alternate states, and session invalidation removing images and revoking both fixture blob URLs. These are deterministic fixture assertions, not service authorization proof. Original controller/thread tests remain unchanged; the presentation adoption test now asserts the four tabs and absence of a feature-owned frame while retaining callback, media and single-consumer checks.

| Viewport | Article heights | Second article bottom | Footer bottom | Horizontal overflow |
| --- | --- | --- | --- | --- |
| 360×844 | 218.30 px | 550.59 px | 616.59 px | None |
| 390×844 | 229.63 px | 573.25 px | 639.25 px | None |
| 430×844 | 244.72 px | 603.44 px | 669.44 px | None |

Evidence directory: [native report](../../../frontend/src/features/event-feed/__tests__/reference/evidence/browser-report.json), [360 px](../../../frontend/src/features/event-feed/__tests__/reference/evidence/feed-360.png), [390 px](../../../frontend/src/features/event-feed/__tests__/reference/evidence/feed-390.png), [430 px](../../../frontend/src/features/event-feed/__tests__/reference/evidence/feed-430.png), [keyboard focus](../../../frontend/src/features/event-feed/__tests__/reference/evidence/feed-focus-390.png). Alternate-state captures are adjacent. All three loaded captures and the 390 px focus capture were opened and visually inspected against the original reference.

## Fidelity assessment and remaining gates

The inspected captures match the reference's content hierarchy and column alignment, with two media-bearing posts and the compose control visible at every tested width. Text is readable without overflow. Touch targets make the tabs/actions taller than the supplied miniature panel. This is an explicit accessibility tradeoff, not a pixel-perfect claim.

The synthetic authors/counts are marked in the harness and never enter production data. Synthetic attachments reuse existing supplied venue illustrations, with original paths and byte hashes recorded in the report. They are not photographs attributed to those fixture users and do not validate real uploaded media. Portraits remain initials in these captures. Therefore the reference's character portraits and concert/sticker imagery are **not** certified by this handoff.

Remaining owner: Frontendking/root/art integration must bind authorized selected portraits and actual scoped media, then verify the combined root against the reference with those bindings. Real backend/media authorization, device behavior, multiuser privacy, IN03 and release acceptance remain **NOT_RUN** here. Do not promote MOCK_READY to ACCEPTED from these component captures or the planning check.
