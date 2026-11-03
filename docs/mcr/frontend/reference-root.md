# FE-REF-ROOT shared reference chrome and default shell

Status: MOCK_READY for this shared frontend composition. This is not acceptance of all ten reference screens, selected character artwork, live services or multiuser behavior.

## Source, assignment and target

Clean assigned base: `b756e9d9d16e1541d8155e09537fcb142317aa44`. Branch: `codex/mcr-fe-reference-root`. Isolated checkout: `/Users/lhs/Desktop/mcr-worktrees/fe-reference-root`. The delivery commit is identified by this file's Git history and the committed handoff. The previous account deletion handoff is already integrated at this base; this patch does not integrate it again.

Viewed the exact ten-screen PNG at `/var/folders/j4/41h_5mjj7j9d8f2j2bzcsngw0000gn/T/codex-clipboard-1c0bbe99-a4bc-4721-a1d3-478a34ef9e76.png`, SHA-256 `d9a8fae2f96a82253542019ab673589b3938627956eace15c8594c702b5c4617`. The king identified the durable target as `docs/mcr/design/reference-target-2026-11-04.png`; that newer file is absent from this older assigned base. The older JPG review document is historical context, not the authority for this patch.

The reference World screen has a compact identity strip, three connected era tabs, dominant illustrated world and six icon tabs. The other mobile screens use nearly the entire window for content under a black titlebar and red close control. These differences drive actual route behavior; this is not a flattened screenshot or a replacement demo app.

## Implementation

- `frontend/src/app/WorldShell.tsx`: mobile content destinations default to full windows. Full mode hides global World identity/era/bottom navigation, preserves the mounted world with input blocked and rendering/connection paused, and exposes Back, reduce and minimize controls. New destinations reopen full content; an explicit size change remains until navigation. Desktop retains the half-panel default and the same full/half/peek controls. Covered world content is visually and accessibly withdrawn without destroying its game. Existing history, opener focus, Escape, scroll restoration, privacy providers and safe-area/visualViewport handling remain authoritative.
- `frontend/src/app/shell.css`: compact frame/body geometry, 6px mobile/8px desktop panel padding, original procedural paper/perimeter scuffs, root height seams, Map flex slot and the agreed world control host overlay. No decorative left rails or side accents.
- `frontend/src/app/shell-chrome/index.tsx` and `shell-chrome.css`: compact identity/event header, notification/settings icons, connected era buttons, six icon tabs with ochre selected state, black route titlebar and red close glyph. Actual controls retain 44px targets even where the visual glyph is smaller. All links use the incumbent router and focus IDs.
- `frontend/src/app/shell-chrome/identity.tsx`: observes the existing AccountProvider/Me state and validated bootstrap participant. It performs no request and creates no controller/cache/session. It never invents level, currency, attendance or live people. It uses AvatarThumbnail only with the existing authorized reviewed source; otherwise it shows a generic decorative star, not substitute character artwork. This component is lazy so account/avatar resources do not become eager public-entry dependencies.
- `frontend/src/app/shell-chrome/parade-footer.tsx`: one newly authored geometric SVG parade ornament for exact `/meet`, `/gifts` and `/me`, hidden in peek mode. Decorative figures are not avatars or representations of live users. No screenshot, source-pack character, third-party vector or historical rendering was copied. Feature owners retain their own CTA/actions; they should not duplicate this footer.
- `frontend/src/shared/ui/concert-window/{index.tsx,concert-window.css}`: denser shared typography, 6px body spacing and authored bell/settings/map/help/broadcast/wall icons. New ConcertWindowEmbedding context gives the first feature window an embedded presentation and resets for nested windows. Semantic headings remain. A window with an explicit onClose retains its own header and action. Standalone components retain their frame/titlebar.
- `frontend/src/shared/styles/base.css`: restrained paper/red tokens, smaller generic heading scale and tighter paragraph leading. No global control geometry or permission behavior changes.
- `frontend/src/app/__tests__/reference-root/{root.test.tsx,preview.html,preview.tsx,native.mjs}`: mobile sizing/Back/embedding tests and actual default-route qualification.

All rendering is newly authored. Existing account deletion consumers/transport, backend, manifests, locks, contracts, router.tsx, PublicBoundary.tsx, WorldHost, feature implementations and `docs/source/` are unchanged. Retained dependencies and pinned Node 24.22.2 were used; no installation or dependency reconciliation.

## Integration contracts sent to feature leads

Root PanelFrame wraps its content with `ConcertWindowEmbedding` exported from `shared/ui/concert-window`. The first ConcertWindow without onClose gets `concert-window--embedded`: outer frame/pseudo/background removed, first titlebar visually clipped while retaining its semantic heading, body padding 0. Nested windows keep their own frame. Feature authors should scope CSS and avoid restoring a second route title or large outside padding.

`.shell-panel-body` remains flex:1, min-block-size:0, overflow-y:auto. It supplies `--mcr-panel-available-height` and `--mcr-wall-viewport-height`: in full mode `calc(var(--shell-viewport-height) - 78px - env(safe-area-inset-top,0px) - env(safe-area-inset-bottom,0px))`; half mode uses viewport*0.55 minus 72px. These are intended for Map/Wall slots without the parade footer. The body containing a direct `.mcr-root-map` is a scoped flex column; `.mcr-root-map` flexes to the full available height with min-height:0. Wall keeps its own canvas/tools/palette/confirmation behavior.

World controls owner is adding `.world-controls-stage` and `.world-controls-host` to the existing composition DOM. Root CSS makes the composition relative, stage flex:1/min-height:120px, and host absolute inset:0 0 34px to reserve the truthful summary. It does not set height:100% on the owner's inset controls. Pointer events are restored only for actionable controls/secondary help content. The assigned base lacks these new semantic hooks: the controls owner and king own that independent integration. Root does not edit composition source or counterfeit a resource/authority binding.

The shared parade footer belongs to Root only for Meet/Gifts/Profile main routes. Domain CTAs remain feature-owned. World, Social, Community, Ops, Wall author and Wall QA received exact seam messages; no additional review gate is imposed.

## Validation and evidence

Commands use the pinned Node PATH and retained tool entrypoints. TypeScript writes only `/tmp/mcr-fe-ref-root.tsbuildinfo`. Native evidence is bounded by an 8GiB free-space preflight. The authorized sparse checkout excludes backend and historical tracked src/doc PNG captures while preserving source, contracts, original public resources and docs/source; it does not create Git deletions.

- `vitest run --maxWorkers=1 src/app/__tests__/reference-root src/app/__tests__/shell.test.tsx src/app/shell-chrome src/app/account-deletion-entry src/app/account-entry src/features/me src/features/settings src/features/auth src/features/account src/shared/ui src/app/__tests__/public-entry.test.tsx`: exit 0, 600 tests across 32 files.
- `tsc --project frontend/tsconfig.json --incremental --tsBuildInfoFile /tmp/mcr-fe-ref-root.tsbuildinfo --pretty false`: exit 0.
- `oxlint src vite.config.ts`: exit 0, no warnings.
- `vite build`: exit 0, 420 modules. Existing large-chunk advisory remains. A new eager identity import initially pulled account resources into the entry bundle; making it lazy removed the ineffective-dynamic-import warning and reduced the entry from 378.99kB to 314.32kB (98.48kB gzip).
- `python3 scripts/mcr/check-plan.py`: exit 0, planning/source checksums preserved.
- `node scripts/mcr/check-ui-origin.mjs`: exit 0, no violations; sparse source scan 1154 files.
- `node src/app/__tests__/reference-root/native.mjs`: exit 0, all three widths and zero page errors. Uses actual createAppRoutes defaults, canonical Auth/resolver/production composition, actual Phaser and original illustrated scenery. It imports the retained illustrated-overview fixture unchanged, which simulates HTTP/C05/admission metadata and geometric avatar test resources. No panel overrides, fake product rows or service authorization are introduced.

Native checks cover 360x800, 390x844 and 430x932: World, Map, Feed, Meet, Gifts, Wall, Customize and Profile; full-window default, hidden World chrome, all sizing modes, keyboard opener/Back focus, direct-route parent fallback, 430px short viewport, 44px chrome targets, no horizontal overflow, unchanged game/camera/outfit when published, active socket peak<=1, and release on unmount. Coverage pauses the existing C05 connection and may open a replacement when returning; total lifetime socket count is not falsely constrained to one.

Artifacts: `/tmp/mcr-fe-reference-root-evidence/result.json` and `{world,feed,map,meet,gifts,wall,customize,profile,settings-short}-{360,390,430}.png`. Result JSON binds source hashes and geometry. Captures were visually inspected for World, Feed and Meet. The new frame/full-window/header/footer are visible, while feature content/art differences remain explicitly open below. A mid-run source edit once caused Vite HMR to replace the game and invalidate that attempt; final qualification runs with source frozen. Initial harness corrections concerned direct Customize/Settings Back correctly returning through Me, port collision fallback and waiting for lazy content; they were not product regressions.

## Remaining product and integration limits

The shared shell's World region measures 571/800, 615/844 and 703/932px (71.4–75.4%). In this assigned base, the old in-flow controls reduce the actual rendered canvas to 390, 434 and 522px respectively. Do not cite the World-region ratio as canvas/reference acceptance. The controls owner's overlay patch and subsequent combined native check must close this real visual gap.

Full content panels measure viewport minus the 8px outer frame: 792/800, 836/844 and 924/932px. The base fixture does not populate all services: some Meet/Gifts/Profile/Wall/Customize states remain unavailable, and selected character derivatives are not yet bound. Their independent feature/art deliveries and final combined route screenshots remain necessary. This root delivery cannot establish exact ten-screen fidelity or real multiuser/service behavior.

No new character design or invalid static-front animation binding is used. Real account deletion, production cookies, providers, PostgreSQL, backend cleanup, device/load tests and IN05 acceptance are NOT_RUN. No scope cuts are implied.

## Final ownership check

`python3 scripts/mcr/check-boundaries.py --base b756e9d9d16e1541d8155e09537fcb142317aa44 --lane frontend --allow-doc docs/mcr/frontend/reference-root.md` exited 0 for exactly 14 changes. Manual grant/import review and result.json source-hash verification passed. `git diff --check` exited 0. The task-owned dependency symlink was removed; no source writer remains active after committed handoff. Final disk headroom was 8.9GiB. Sparse checkout settings are local only and original tracked bytes remain in Git.

The published root follow-up replaces the fixed34px control bottom inset with a shared explicit grid row for scene and controls and an automatic live-summary row. Fresh actual-default-root Chrome checks at360/390/430 pass control/canvas alignment within1px and prevent control overlap with the summary; route sizing, keyboard/Back/focus and singleton cleanup still pass. See `world-control-overlay-sizing.json`. This is synthetic integration evidence, not visual/live acceptance.
