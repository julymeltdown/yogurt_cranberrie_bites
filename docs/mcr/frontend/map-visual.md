# UIV05-MAP — actual Places presentation

Status: **MOCK_READY / READY for integration**. This is the bounded presentation adoption of actual `PlacesOverview` and `PlaceDetail`. It does not accept approved map artwork, real venue/source truth, production world wiring, backend/device/multiuser behavior, FE19/QA07 as a whole, or IN05.

## Assignment and provenance

- Branch: `codex/mcr-fe-UIV05-map`; exact published base: `bea2e72f4d6f0e42be32700b5546c266bb157231`. The single commit containing this document is the delivery candidate, directly on that base; Main owns integration/publication. No worker push.
- Production changes are exactly `frontend/src/features/places/ui/Places.tsx` and `frontend/src/features/places/ui/places.css`. New support is confined to `frontend/src/features/places/__tests__/visual-adoption/**` and this document.
- Required implementation specification, assessment, execution plan, `docs/mcr/frontend/places-help.md`, and the current reference audit were read. The actual unchanged ten-screen image and the native before fixture were viewed before presentation changes. Existing Places revision/recovery UI last changed at `70d89728a035ae448b0fa136fabcb14e44e5bc10`; its controller and contract are unchanged here.
- Human reference: `docs/mcr/design/ui-reference-audit-2026-11-04/user-reference.jpg`, SHA-256 `2b3988d546f87b813d78ca13bf267730a6dd19118bd6535a2d3c3032b6f12d23`. Compare the upper-row MAP composition. Neither reference, source documents, public world/map art nor character pack changed.
- Presentation reuses published `shared/ui/concert-window` at the assignment base. No external rendering or visual dependency was copied. No install, package change or lockfile resolution occurred.
- New `visual-adoption/preview.tsx` is a test-only derivative of the existing FE19 `__tests__/fixtures/preview.tsx` at the base. It reuses the original synthetic metadata/terrain, shell, FE14 loader/Phaser and FE15 movement attachment; changes are a test bridge name, pending-response mode and optional unavailable camera port. Production components contain no fixture mode. The new build/runner uses Vite production bundles and a fixture-only `/world/map` document response to verify actual reload recovery. No runtime, camera API, controller, model, root wiring, WorldHelp or shared component changed.

## Delivered presentation and preserved behavior

The MAP window has compact black title chrome, the published symmetric ivory frame and condensed heading. The existing validated schematic is the first useful object. Its caption explicitly says **schema-derived virtual schematic**, identifies the existing map revision and numbered/closed landmarks, and states that it is neither venue artwork nor a physical route. Existing camera recenter/zoom controls sit immediately below the figure, with a live camera-result notice adjacent to them. The virtual-arrival warning, keyboard-operable full legend, search, shown count, complete numbered DOM list, detail links, existing Show on map actions, refresh and help navigation remain available.

The SVG's collision-cell/interaction calculations, map identity, validated geometry and numbering are unchanged. No new coordinates, paths, labels, controls, source URLs, photos, artwork or fallback characters were invented. The optional camera port remains the same FE15 attachment; no additional world, movement binding, game or controller lifetime is constructed. The wording **Recenter avatar** retains its original camera callback; it does not move the avatar.

Place detail uses its current label in the black window header, followed by catalog revision and full current status. Accessibility notes, virtual/concept/closed/unverified/verified caveats, source reference/revision, review state/reviewer/date, author, license and photo permission remain readable. Safe HTTPS source-link checks, no-referrer/noopener protection, missing-source text and explicit unavailable-photo text are unchanged. Existing Show on virtual map, refresh, All places, help and browser-Back/focus behavior remain intact.

All target eligibility remains in the existing controller. Closed, missing, stale, mismatched or unavailable-camera targets remain disabled. Unverified physical metadata is still described as unverified; looking at a permitted **virtual** target never verifies it or offers a physical arrival action. Authoritative missing-place invalidation survives All places and catalog retry; only matching detail can restore that target. A detail/map mismatch remains latched across navigation and same-release retry. **Reload event** remains the actual document reload, not a cosmetic retry or catalog-only request. Denied reads offer no fabricated retry capability. No necessary status or recovery action was hidden.

CSS is feature-scoped, retains at least 44px controls, visible symmetric focus outlines, reduced-motion rules and bottom safe-area spacing. The existing shell/body owns top/horizontal safe areas, input blocking and panel scrolling. No decorative left rail, colored left border or pseudo-element accent was added.

## Before / reference / after

Open [the comparison](../../../frontend/src/features/places/__tests__/visual-adoption/evidence/comparison.html): unchanged full reference, actual 390px before/after overview and reviewed-place detail, all fixture-labeled. Three before captures bind to the exact assignment-base UI bytes; `baseline.mjs` now refuses to recapture a changed UI as “before.” Inspect `baseline.json` for source/reference hashes. After evidence and its 23 tested source/contract/reference pins are in [report.json](../../../frontend/src/features/places/__tests__/visual-adoption/evidence/report.json).

Native captures were inspected for the 360px overview, 390px detail and 430px mismatch. The useful map now starts **64px below the feature frame top** at every tested width, with camera controls 47–64px after the diagram, including its required honest caption. Map heights were approximately 170/187/211px at 360/390/430px respectively. No feature control was smaller than 44px and no tested state had document-level horizontal overflow.

Remaining differences are explicit: the current input is a schema-derived collision schematic, not the reference's illustrated venue map. No approved map-art delivery is claimed. The root header, theme navigation, shell Back/Hide controls and bottom navigation are outside this assignment and retain their current presentation. The real places and official physical paths have not been verified by these fictional fixtures. Existing source/photo permissions do not authorize an invented image.

## Executed checks

Runtime: pinned Node `24.22.2`, retained pnpm `11.7.0`, Vite `8.1.3`, Phaser `3.90.0`, native Chrome `156.0.8078.80`. Temporary `frontend/node_modules` symlink reused the existing installation read-only and is removed before final boundary/commit. Build output is private to `/tmp/mcr-uiv05-map-build`; no dev optimizer, install or shared TypeScript build-info cache is required.

| Command | Exit and result |
| --- | --- |
| `vitest run --root frontend --no-cache src/features/places/__tests__` | **0 — 61 existing FE19 tests / 2 files passed**. No repeated broad frontend/world suite. |
| `tsc --noEmit --incremental false --project frontend/tsconfig.json` | **0 — strict combined frontend types passed**. |
| `oxlint frontend/src frontend/vite.config.ts` | **0 — lint passed**. |
| `python3 scripts/mcr/check-plan.py` | **0 — planning consistency passed**, not implementation acceptance. |
| `node scripts/mcr/check-ui-origin.mjs` | **0 — source-origin guard passed**. |
| `node frontend/src/features/places/__tests__/visual-adoption/baseline.mjs` | **0 before production edits** — app/native-fixture production build plus three exact-base native screenshots. Baseline guard intentionally rejects running it against the adopted UI. |
| `node frontend/src/features/places/__tests__/visual-adoption/browser.mjs` | **0 — actual app + native-fixture production builds, 54 after screenshots, all 3 phone widths passed**; zero page errors. Existing large Phaser chunk warning remains. |
| `python3 scripts/mcr/check-boundaries.py --base bea2e72f4d6f0e42be32700b5546c266bb157231 --lane frontend --allow-doc docs/mcr/frontend/map-visual.md` | **0 after dependency symlink removal**; exact two-path/new-subtree allowlist, protected-byte and import checks also passed. |
| `git diff bea2e72f4d6f0e42be32700b5546c266bb157231 --check` | **0**, including staged evidence and documentation. |

Final command/results are in [validation.json](../../../frontend/src/features/places/__tests__/visual-adoption/evidence/validation.json) and sibling logs; [delivery.json](../../../frontend/src/features/places/__tests__/visual-adoption/evidence/delivery.json) records exact ownership, hashes and protected-byte checks.

Every width, **360×800 / 390×844 / 430×932**, exercises loaded, empty, error, denied, pending, search/no-match, concept, reviewed, unverified, closed, unsafe-source, camera-unavailable and mismatch states. New browser checks assert map-first geometry, adjacent controls, target sizes/overflow, keyboard disclosure and primary navigation, destination heading/opener focus, real virtual camera zoom/look/recenter, unchanged avatar foot, one persistent canvas across map/detail/help, authoritative removed-target latch and matching-detail reconciliation, detail mismatch retained after navigation/retry, actual full-document recovery with a new document identity/fresh owner, and canvas/control cleanup. This is stronger than a restyled static specimen while remaining synthetic HTTP/terrain evidence.

No live backend, real device, venue admission/safety/source verification, approved art or real world connection was exercised. The original FE19 runtime gates and release acceptance limits remain unchanged. Main can integrate this scoped candidate directly; there is no additional general lead approval gate.
