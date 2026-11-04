# FE07 persistent shell journey qualification

Status: **IN_PROGRESS — whole original FE07 client criteria SATISFIED; validated handoff awaiting designated integrator publication**. The actual goal remains active until publication is verified. Final qualification candidate: `8cd2046601b21e957e73a099d3975aa740a2a1bd`. This document supplements the historical `world-shell.md` implementation; it does not amend the specification or accept other feature tasks.

## Scope and provenance

New fixture/runner implementation: `009327ca52d127db39b615e6049e4c66f37decef`. The assigned branch is `codex/mcr-fe-FE07-shell-journey`, based on `5dbb99a041195ec751ea141ca3c03ede100c70d1`. The only new writes are `frontend/src/app/__tests__/fe07-shell-journey/**` and this document. No application, existing test, backend, contract, manifest, lockfile, source archive, character original, or art asset is edited by this handoff. Retained installed dependencies and Chrome are reused; no installation or dependency change is required.

The original implementation is recorded in `docs/mcr/frontend/world-shell.md` (`d7c720ce6322515280a302f39c8f9a66c790ffa8`, reviewed implementation `18740f2895fce2d2f5fe3be1695bc007a4f3a1d0`). Its instrumented world ports did not prove the actual Phaser/socket criteria. `reference-root.md` subsequently proved actual root lifetime but did not qualify the entire browser history/direct-gift journey. This packet closes that distinction with the real browser router and default production composition.

| Input | Use and qualification |
|---|---|
| `MCR_IMPLEMENTATION_PLAN_V2_EN.md`, §§5.1, 6 and FE07 | Original header, six tabs, nested panels, browser history, safe returns, lazy boundaries, direct gift and single-world criteria |
| `docs/mcr/project-assessment.md`, `docs/mcr/execution-plan.md` | Current delivery context; no scope override |
| `frontend/src/app/router.tsx` / `createAppRouter` | Actual browser router under StrictMode, normal public/admin lazy modules and shell route composition |
| `frontend/src/features/auth/model/session.ts` | Canonical Auth; actual SignIn form, memory session, privacy scope and production bootstrap loading |
| `frontend/src/app/ports.ts` / `createProductionPorts` | Default production world loader, engine import and renderer; no engine/loader/world/panel overrides |
| `frontend/e2e/reference-integrated/whole/data.ts` | Existing data-only selected-character/illustrated-source fixture, synthetic C03 bootstrap/C05 frames; imported without copying product rendering |
| `contracts/fixtures/domain/giveaway-management.json` | Existing C04 shape; 18 explicitly synthetic, descending-date giveaway records create genuine native list scrolling |
| Retained React Router 7.18.1, React 19.2.7, Phaser 3.90.0 | Existing pinned runtime; no dependency changes |

The fixture is a separate development-only HTML entry, never imported by `main.tsx`. It authenticates only its synthetic HTTP producer, records request path/method/protected-header presence without tokens, and replaces the socket producer with deterministic C05 frames. Map/place review and local movement policy are explicitly test-only. Selected-character original bytes are read unchanged. No previous character atlas or legacy fallback is introduced.

## Product fixes and evidence

Application fixes were routed to their existing single writers; FE07 did not acquire or edit their paths.

| Criterion | Before | Delivered correction |
|---|---|---|
| Preserve connection under full panels | Ten sequential socket allocations during one 390px journey, despite peak concurrency one | FE15 implementation `977b773d`, published through `50d950d79939a539ebe953744287f7a280ce488b`: panel visibility no longer withdraws replication; actual document background still does |
| Resume controls after editing | Typing state survived textarea unmount | Same FE15 implementation: synchronize typing from the active element after route changes |
| Restore gift list scroll | Native POP restored opener focus but scroll 1894 → 0 at base `5dbb99a0`; preserved `evidence/before-scroll.json` and `.png` | FE15 explicitly allocated support implementation `416b4e5f1adf6c1e6d1ad7f376dbdd92b7573a63`, published through `1b4ac4579764c2a475b729f702f56d9e3ec311aa`: PanelFrame captures pre-replacement position and extent; own independent recheck 1894 → 1894 at `a84885f2` |
| Public/admin exits | Social navigation guard rejects public/admin targets as unsafe participant destinations; no pending edit is required. Continue navigation remains disabled. Preserved `evidence/before-boundary.json` and `.png` | Social owner implementation `daa1c757aba7a48f6841abfc6dbc815089e55c9d`, integrator-mapped source `a572b101b67c07c75fda1fa9be359c264ff7d3e8`, published through `4052d709fef976d14653351cd8bf78ef85bd1579`. Registered public/admin exits use a separate lexical policy, retain social command/draft guards, and defer destination authorization to its own boundary. Own final clean-exit/Back/admin cases pass all three widths |

The old `meeting-entry/__tests__/root-registration.test.tsx` direct-beacon test is a separate readiness diagnostic: its second click follows router publication before the destination DOM commits, invoking the old handler again. The already-published `back-readiness/navigation.test.tsx` demonstrates that ordering. The native journey observes each committed destination before another click and qualifies direct beacon → Meet → World through actual browser history, keeping the same game, camera and connection. This does not grant or expose real meeting guidance.

## Original FE07 criterion mapping

| Original requirement | Actual qualification |
|---|---|
| Header and six labeled tabs | Default shell, all six real links, current-page semantics, 44px minimum targets at 360/390/430 |
| One primary nested panel; peek/half/full | Default feature routes; real panel size controls, hidden navigation in full state, disabled movement controls while blocking |
| History/back | Gift list → detail → native POP/forward/Back; opener focus; direct gift/beacon foreign-history fallback replaces known parents instead of traversing outside the shell |
| Preserve camera, room, position and list scroll | Same real game/camera; camera world center/zoom/following preserved across viewport resize; same C05 room/position; native scroll comparison |
| Safe drafts | Actual Me textarea retains unsaved text through panel resizing and World navigation; no save mutation. Revert through the real editor before boundary cleanup checks |
| Direct gift initializes shell | Anonymous cold deep link → actual SignIn with safe return target → held production bootstrap (zero game/socket) → requested default gift detail plus exactly one real Phaser game |
| Safe returns and denied targets | External, protocol-relative, admin and traversal login return targets normalize to World; denied gift detail has safe copy and no raw diagnostic; parent/world Back remains available |
| One world connection/game per active shell | Same game and camera; exactly one allocation throughout tab/history navigation, peak one; public/admin release to zero; returning to World creates one replacement |
| Lazy public/admin boundaries | Anonymous public entry does not load Phaser; legitimate public/admin transitions and denied admin gate use actual normal modules; all three widths pass after the published boundary correction |

## Verification and delivery

The runner `frontend/src/app/__tests__/fe07-shell-journey/native.mjs` launches retained installed Chrome with an isolated temporary browser context and ephemeral Vite server, and closes both in `finally`. It defaults to 360/390/430 × 844 CSS pixels. `FE07_WIDTHS=390` is a diagnostic subset, not full acceptance. The final `evidence/native.json` records the exact Git candidate and SHA-256 hashes of the application seams and runner. Screenshots and pre-fix evidence stay in the assigned prefix.

The final frozen candidate passes **11/11 native cases** with zero browser errors and no open findings: three complete gift/tab/history/draft journeys, three public/admin boundary lifecycles, one direct-beacon history case, and four unsafe login-return cases. Native scroll is 1889 → 1889, 1894 → 1894 and 1901 → 1901; each same-shell journey retains exactly one actual game, camera and connection allocation. Public/admin exits release game and active sockets to zero; browser Back creates one replacement with no overlap. Anonymous and held-bootstrap samples have zero game/socket. The runner fingerprints its source before capture and rejects source changes during capture. Chrome captures were visually inspected for default World and direct gift presentation.

| Command (retained Node 24.22.2; repository cwd unless noted) | Result |
|---|---|
| `node frontend/src/app/__tests__/fe07-shell-journey/native.mjs` | Exit 0; 11/11, widths360/390/430 |
| `node node_modules/vitest/vitest.mjs run src/app/__tests__/routes.test.ts src/app/__tests__/shell.test.tsx src/app/meeting-entry/__tests__/back-readiness/navigation.test.tsx src/app/social-composition/__tests__/fe07-boundary-exits/boundaries.test.ts src/app/social-composition/__tests__/owner.test.ts src/app/social-composition/__tests__/provider.test.tsx` (frontend cwd) | Exit 0; 155 tests / 6 files |
| `node frontend/node_modules/typescript/bin/tsc --project frontend/tsconfig.json --incremental --tsBuildInfoFile /tmp/mcr-fe07-journey.tsbuildinfo --pretty false` | Exit 0 |
| `node node_modules/vite/bin/vite.js build --configLoader runner --outDir /tmp/mcr-fe07-build --emptyOutDir` (frontend cwd) | Exit 0; existing >500kB chunk warning; fixture strings absent from emitted application JS |
| `node frontend/node_modules/oxlint/bin/oxlint frontend/src/app/__tests__/fe07-shell-journey` | Exit 0 |
| `python3 -B scripts/mcr/check-plan.py` | Exit 0; all13 original archive hashes preserved; planning only |
| `node scripts/mcr/check-ui-origin.mjs --root "$WORKTREE"` | Exit 0; static bytes/import scope |

Reproduction uses the existing retained dependency tree, installed Chrome, and pinned Node on PATH; no install or browser download is needed. Build/cache outputs are temporary, outside source ownership. `evidence/checks.json` and sanitized logs record commands and exit codes. `evidence/native.json` records candidate, toolchain, start/end times, per-case data and source/contract hashes. `direct-gift-{360,390,430}.png` and `world-after-history-{360,390,430}.png` show final states. The two pre-fix packets are explicitly historical.

The unchanged old `meeting-entry/__tests__/root-registration.test.tsx` diagnostic at `8de6c411` passed71/72 and exited1 solely for the obsolete DOM-readiness assertion described above. Its actual log is preserved as `historical-meeting-readiness.log`; it is not presented as a passing suite. The final155-test selection includes the three committed-DOM readiness regressions and87 boundary/owner/provider checks, while the native browser qualifies the actual beacon route behavior. Correcting that old test is a separate existing-test ownership matter; FE07 has no edit grant to it.

Only the three new harness files, their new owned evidence prefix and this document are delivered. No application source is authored by this packet, and the supported fixes retain their own committed provenance. The designated frontend delivery integrator receives the fixture implementation plus the subsequent documentation/evidence commit, compares exact paths and source fingerprints, and publishes serially. FE07 does not push master directly. Publication ancestry and actual goal completion are verified after integration; client criteria satisfaction alone is not represented as delivered publication.

## Limits

This is native installed Chrome at mobile CSS widths, with real client rendering, Auth ownership, browser history and default Phaser/production world paths, but deterministic HTTP/C05/source producers. It accepts only original FE07 client behavior. The draft criterion covers panel changes within the active identity/event shell; it does not claim a newly added cross-owner Account draft exit guard. Live backend/refresh cookies, two-user delivery, physical/device keyboard and safe-area behavior, movement authority, real meeting audience/place/expiry, giveaway inventory/reservations/pickup, production art admission, performance/load, all 112 tasks/61 IA entries, IN02/IN05 and release readiness are **NOT_RUN** by this packet. Existing source admission guards remain closed. No mock is presented as a real multiuser flow.
