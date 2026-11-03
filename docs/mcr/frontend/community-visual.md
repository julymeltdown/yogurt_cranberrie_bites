# UIV03 community visual adoption

Status: **MOCK_READY / READY for integration review**. The three actual browse components adopt the published UIV01 concert-window vocabulary. Native browser evidence uses deterministic, explicitly synthetic data. No live backend, real multiuser flow, approved avatar/media delivery, production population, IN05 or whole-IA acceptance is asserted.

## Assignment and provenance

- Task: UIV03; branch `codex/mcr-fe-UIV03-community`; exact base `5ef2df7f9fa01868b0f734233c87a00e37195b61`.
- Only six assigned production paths changed: `frontend/src/features/event-feed/EventFeed.tsx`, `frontend/src/features/event-feed/feed.css`, `frontend/src/features/party-discovery/ui/PartyDiscovery.tsx`, `frontend/src/features/party-discovery/ui/party-discovery.css`, `frontend/src/features/gift-discovery/ui/Gifts.tsx`, `frontend/src/features/gift-discovery/ui/gifts.css`.
- New supporting paths are confined to the three assigned feature `__tests__/visual-adoption/` subtrees and this document. The commit containing this document is the delivery revision; its parent is the exact base above. Main owns integration and publication.
- Existing FE21 party presentation originates at `78cfb7c`; existing FE24 gift presentation originates at `3cb3f9c`. The existing feed/PostContent implementation, controllers, contracts and qualified `renderMedia` seam are retained from the assignment base. No external application rendering or visual dependency was copied.
- Reused public components: `shared/ui/concert-window/{index.tsx,concert-window.css}` at the assignment base. No shared component, router, provider, controller, manifest, lockfile, backend, source pack or source document was edited. No dependency was added or re-resolved.
- Human reference actually viewed: `docs/mcr/design/ui-reference-audit-2026-11-04/user-reference.jpg`; SHA-256 `2b3988d546f87b813d78ca13bf267730a6dd19118bd6535a2d3c3032b6f12d23`. Read `docs/mcr/design/ui-ux-reference-review.md` and UIV01 guidance before implementation. The reference and `docs/source/` remain byte-identical to the assignment base.

## Delivered behavior

FEED uses a black window titlebar, ivory symmetric frame, compact four-way semantic tabs, red compose action and smaller paper refresh/thread controls. Arrow keys, Home and End use the published tab implementation. Only the selected panel renders posts/media, avoiding duplicate authorized media consumers. Public author, time, text, images through the existing optional renderer, related links, reaction/comment counts, notice selection/pagination and feed pagination remain controller-owned. Notices are available through a compact native disclosure. Read-only removes compose; unavailable, empty, pending and failure/retry remain truthful.

MEET uses the same window and dense public party rows with title, host display name, section/state, time, member count, remaining spaces/full state and a red **Details** link. Parties/MY PLAN remain native pressed view controls; People remains its existing router link, rather than being misrepresented as a local tab. Filters are unchanged behind their disclosure. Detail, host approval, application note, cancellation, uncertain/pending command checks, MY PLAN paging and physical-arrival guidance retain their existing handlers. Browse does not submit or promise a join.

GIVEAWAY uses the same frame, compact Browse/current and MY GIFTS navigation, existing filter disclosure, and rows with public owner, status, stock and pickup times. Title and red **Details** link retain `/gifts/:giveawayId`. Pagination and validation remain unchanged. Detail retains its existing disabled `Reserve unavailable` action; this patch does not invent a reservation capability. Read-only is explicitly described. The shared `GiftRow` also remains usable in MY GIFTS, including a plain owner-only draft without a public detail action.

Browse CSS is feature-scoped. Existing detail, application, thread/composer and pickup/authoring structures remain intact. Native controls retain 44px targets, visible symmetric focus outlines and reduced-motion handling. Browse wrappers reserve bottom safe-area space; existing body/shell owns top and horizontal safe areas. No decorative left rail, accent border or pseudo-element bar was introduced. The published symmetric perimeter treatment is reused unchanged.

## Reference comparison and limits

Open [the side-by-side comparison](../../../frontend/src/features/event-feed/__tests__/visual-adoption/evidence/comparison.html). It displays the unchanged full reference and actual 390px populated component screenshots. Evidence images have visible synthetic labels. The match is in hierarchy, black titlebar, ivory window, deep-red actions, condensed headings, compact filters/navigation, and dense useful public metadata. Reference action labels that imply instant JOIN/RESERVE are deliberately translated to the actual detail-first flow.

No avatar renderer is currently supplied to these party/gift presentation seams, and the selected reference's art is not a qualified asset source. Synthetic avatar metadata is not product art. No character fallback, invented asset URL, private handle, redrawn/cutout reference image, generated art or fake production population was added. FEED's existing authorized-media seam is preserved and exercised with an inert test renderer; browser populations contain no media URLs. Missing avatar/item imagery and untouched root shell chrome remain visible differences. UIV01 and this task do not establish overall visual acceptance.

## Verification

All commands below exited **0** on the delivered source unless otherwise marked. Runtime: Node `24.22.2`, retained pnpm `11.7.0`, Vitest `4.1.9`, Vite `8.1.3`, native Chrome version recorded in the browser report. Temporary dependency reuse is a removable `frontend/node_modules` symlink to the existing installation; it is removed before final boundary checks and commit. No dependency install or lockfile change is part of delivery.

| Check | Evidence and result |
| --- | --- |
| Focused regression + new adoption tests | `node frontend/src/features/event-feed/__tests__/visual-adoption/validate.mjs`: **261 tests / 20 files passed**. Covers feed/thread, party discovery/authoring, gift discovery/authoring/pickup, MY GIFTS, concert-window, community-entry, default community composition and shell. |
| Strict types | `tsc --noEmit --incremental false --project frontend/tsconfig.json` passed; no shared build-info cache writes. |
| Lint | `oxlint frontend/src frontend/vite.config.ts` passed. |
| Planning | `python3 scripts/mcr/check-plan.py` passed. This is planning consistency, not implementation acceptance. |
| UI provenance / whitespace | `node scripts/mcr/check-ui-origin.mjs` and `git diff --check` passed. |
| App + native fixture production bundles | `node frontend/src/features/event-feed/__tests__/visual-adoption/browser.mjs` builds the actual app entry plus new populated fixture, existing FE21/FE24 shell fixtures and default root fixture into `/tmp/mcr-uiv03-community-build`; Vite preview, no dev optimizer. Passed. Existing large Phaser chunk warning remains. |
| Native browser | Same browser command: **96 screenshots**, 18 grouped checks, 360/390/430px mobile/touch/reduced-motion contexts, zero page errors. See report for source/contract/reference SHA-256 values, exact geometry and browser version. |
| Lane/assignment boundary | After dependency symlink removal: `python3 scripts/mcr/check-boundaries.py --base 5ef2df7f9fa01868b0f734233c87a00e37195b61 --lane frontend --allow-doc docs/mcr/frontend/community-visual.md`; additional exact six-path/new-subtree allowlist and protected-byte comparison. Receipt saved with evidence. |

[Browser report](../../../frontend/src/features/event-feed/__tests__/visual-adoption/evidence/report.json) and [validation commands/exit codes](../../../frontend/src/features/event-feed/__tests__/visual-adoption/evidence/validation.json) are durable. Sibling logs contain final passing test, type, lint, plan, origin and whitespace results. Browser geometry verifies visible loaded/read-only control targets and no horizontal overflow at each width; first populated row starts before 330px in loaded standalone fixtures and before 400px with the explicit read-only notice, including fixture labeling. Keyboard checks exercise feed filter arrows/Home/End, compose/thread callbacks, and party/gift Details routes. Read-only detail gates remain disabled.

Every width exercises loaded, empty, failure, loading, read-only and unavailable states. Existing FE21 shell evidence includes actual Phaser with synthetic terrain and an instrumented connection counter; it is not a real world socket. FE24 shell evidence uses an instrumented world lease. Default unmodified `createAppRoutes`/`CommunityFeatures` composition additionally exercises browse/detail, MY GIFTS draft, pending party command retained across tabs, command reconciliation, MY PLAN and full owner disposal. Root navigation and wrappers are intentionally not reassigned here. FEED is exercised as the actual component with actual controller; wiring a new feed feature into the production root is outside this assignment.

## Handoff

The delivery commit is a single reviewed candidate directly on the assignment base; no push from this worker. The main integrator should review the six production paths and evidence, integrate serially, then publish master. This scope is complete when that scoped commit and evidence are handed off and concrete review fixes are addressed. Existing backend/runtime/IA release gates and FE39's recorded blocker remain separate.
