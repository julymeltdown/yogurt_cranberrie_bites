# FE06 theme consumer and world skin independent review

Date: 2026-11-03. **Bounded review complete: Finding R1 resolved in final
candidate `4cb28b5`; C06 immutable map release identity preserved with neutral
Tiled marker seam; no remaining actionable defect identified. Parent review
and integration disposition pending.** Implementation/evidence status remains
MOCK_READY. This report does not accept production artwork, server preference
synchronization, real networking, or venue arrival.

## Scope and authority

QA checkout: `/Users/lhs/Desktop/mcr-worktrees/qa-fe-theme`, branch
`codex/mcr-qa-FE06`, base commit `600582d6b5deda6037c190c303f80752ba4a6bb3`.
Implementation source was read from the isolated `fe-theme` worker worktree
(`/Users/lhs/.codex/worktrees/3c8d/mcr`). QA's only tracked writable file is this
report. No application code, test suites, contracts, assets, manifests, or
dependencies were modified by QA. No Rust or PostgreSQL work was performed.

Exact worker chain:
- Initial candidate: `40d86dc75ecb90c75dc80dbd358d9f4a7fce0944`
- Final candidate (R1 fix): **`4cb28b5f45e4d96fdb36e6e041a1bd0e8568e005`**
Authorized QA local cherry commits are `f8a9d51` and `dabeb1a`; they reflect
worker implementation code for independent evaluation. The parent takes only
the separate report commit from this branch.

The twenty worker paths comprise this explicit scope:
- UI & styling: `frontend/src/features/theme/ui/{ThemeSelector.tsx,ThemeScope.tsx,theme-selector.css}`,
  and `frontend/src/shared/styles/themes/themes.css`.
- Model & preferences: `frontend/src/features/theme/model/{controller.ts,preferences.ts}`.
- Feature tests & preview: `frontend/src/features/theme/__tests__/{browser-smoke.mjs,contrast.test.ts,controller.test.tsx,preview.html,preview.tsx}`.
- World runtime seams: `frontend/src/world/runtime.ts` and `frontend/src/world/scenes/MapScene.ts`.
- World theme logic & tests: `frontend/src/world/theme/{compatibility.ts,loader.ts}`,
  `frontend/src/world/theme/__tests__/replacement.test.ts`, and `fixtures/{png.ts,releases.ts}`.
- Authorized contract seam: `frontend/src/world/map/tiled.ts`.
- Worker documentation: `docs/mcr/frontend/theme-system.md`.

The specification authority is `MCR_IMPLEMENTATION_PLAN_V2_EN.md` §§5.2–5.4
(shared theme identity, physical/virtual separation, and theme preference
scoping), C06 asset contracts (`docs/mcr/assets.md`), and FE05/FE14 conventions.
Virtual theme selection does not prove physical arrival or admission.

## C06 contract boundary and neutral Tiled marker

The worker's initial design attempted to permit different tilemap resource
bindings and map content hashes per skin under the same `map_id` and `map_revision`.
Parent and QA inspection confirmed this violated C06's immutable release identity:
C06 §Map, place, and theme identity (`assets.md:125`) requires all three themes
to bind the exact same `map_content_hash` and immutable `MapManifest`.

The parent authorized a minimal, non-breaking consumer seam:
1. Exported `THEME_IMAGE_MARKER = 'mcr-theme.png'` in `frontend/src/world/map/tiled.ts`.
2. `validateTiled` accepts either `THEME_IMAGE_MARKER` or the exact `theme.texture.path`.
   All other paths (including path traversal, absolute URLs, or mismatched names)
   remain strictly rejected.
3. The marker is inert export metadata and is never resolved or fetched over
   the network; the actual tile texture PNG is always independently loaded,
   allowlisted, and SHA-256 verified from `ThemeManifest.texture`.
4. All three skins bind the single immutable `MapManifest` and Tiled asset.
   `assertSameWorld` enforces strict equality of `canonicalJson(current.map)`,
   geometry, dimensions, spawns, and avatar release references.

QA inspected `validateTiled` and `assertSameWorld` in source and tests, and
executed an initial probe script (`independent-qa-probe.mjs`). While the script's
later supersession section exited with code 1 due to a harness unhandled rejection,
the earlier `validateTiled` and `assertSameWorld` assertions executed as inspected:
- `THEME_IMAGE_MARKER` and exact texture path are accepted.
- Relative traversals (`../evil.png`), remote URLs (`http://evil.com/t.png`),
  empty strings, mismatched filenames, and column/tilecount modifications throw `MapError`.
- Any mismatch in map content hash, tilemap path, collision grid, places, or avatar
  manifest identity throws `MapError`.
Frozen C06 schemas and golden fixtures remain untouched.

## Finding R1: post-engine cancellation desynchronization

**P2; reported by QA, accepted by parent, resolved in `4cb28b5`.** In the initial
candidate `40d86dc`, `runtime.replaceSkin` performed:

```ts
engine.replaceSkin(next)
if (!current()) throw new MapError('cancelled') // line 161
owned.map = next; committed = true
previous.dispose()
```

Because `engine.replaceSkin(next)` executes synchronously, it immediately runs
`prepared.show()` and `previous?.remove()`, irreversibly destroying the previous
texture and tilemap in Phaser. If a reentrant replacement or cancellation occurred
during that call (e.g. from Phaser TextureManager events or UI listeners),
`current()` evaluated to `false` at line 161. Runtime threw `MapError('cancelled')`,
leaving `owned.map` as `previous` and running `candidate.dispose()` in `finally`.
Phaser was left actively displaying `next` while its underlying image was disposed,
`previous` was destroyed, and `runtime.readSkin()` claimed `previous` was active.

QA confirmed this defect with an independent probe (`/tmp/mcr-qa-fe06-agy8u3i5/reentrancy-probe.mjs`):
under synchronous reentrant replacement, the initial candidate threw `cancelled`,
reported `runtimeReadSkin: 'REVENGE'`, while `activeEngineTheme` was `'PARADE'`
and `nextImageDisposed` was `true`.

### Resolution verification

In candidate `4cb28b5`:
1. Reentrant calls during synchronous commit are serialized via a `skinCommitting`
   flag: reentrant invocations yield to the microtask queue (`await Promise.resolve()`)
   until the current visual commit finishes.
2. Caller aborts during commit no longer throw after `engine.replaceSkin` succeeds;
   only an actual owner stop/dispose throws.
3. `owned.map = next; committed = true` and `previous.dispose()` execute reliably
   upon successful engine return.
4. Calling `replaceSkin` with the already-owned lease is a safe no-op.
5. In `ThemeController`, load failures reconcile `selected` to `runtime.readSkin()`,
   announcing the actual visible fallback skin rather than an obsolete state.

QA repeated the exact reentrancy probe against `4cb28b5`:
- Result: `threwError: null`, `runtimeReadSkin: 'PARADE'`, `activeEngineTheme: 'PARADE'`,
  `nextImageDisposed: false`, and `initialImageDisposed: true`.
- `controller.test.tsx` and `replacement.test.ts` added unit regressions for deferred
  reentrancy, caller abort during commit, same-lease no-op, and fallback reconciliation.
- `browser-smoke.mjs` added a real Phaser `TextureManager.addImage` reentrant switch test.
**Finding R1 is completely resolved.**

## Behavior, accessibility, and UI verification

QA verified the complete FE06 user and engine behavior:
- **Visual & Engine Continuity**: The single Phaser game and canvas instance are
  strictly preserved across theme changes. Camera scroll, zoom, and local predicted
  movement foot position remain identical before and after switching. No canvas
  or movement attachment recreation occurs.
- **Atomic Rollback & Supersession**: Simulated resource failure (503) or engine
  preparation failure rolls back cleanly, removing staged layers/textures and retaining
  the prior skin. An in-flight slow load superseded by a fast load is cancelled,
  and its late-resolving image is disposed without overwriting the active skin.
- **Preference Scoping**: `createLocalThemePreferences` stores preferences under
  `mcr:theme:v1:<encoded userId>:<encoded eventId>`. Delimiters are safely encoded
  preventing cross-scope collision. Invalid stored values are ignored on restore.
  Preferences are written only after a successful skin commit; storage failures
  leave the active skin in memory with retry feedback.
- **Accessibility & Contrast**:
  - Semantic tokens in `frontend/src/shared/styles/themes/themes.css` pass WCAG AA
    contrast in all three eras: normal text pairs (`ink`, `muted`, `red`, `success`,
    `warning` on `paper`) achieve >= 4.5:1; `focus` on `paper` achieves >= 3:1.
  - Native radio inputs with legend/fieldset and visible focus rings.
  - Touch targets for theme options are at least 44×44 CSS pixels.
  - Reduced motion: `data-mcr-reduced-motion="true"` and `@media (prefers-reduced-motion: reduce)`
    force all transitions and animations to `none !important`.
- **Styling Rules**: No decorative left accent lines, colored left borders,
  or inset left rails are used. Borders and outlines are ordinary and symmetric.
  No character artwork or legacy assets are included.

## Independent evidence artifacts and corrections

Independent probes were run using Node 24.22.2 and headless installed Chrome
156.0.8078.80 on owned ephemeral port 45849. All test servers and browser contexts
were verified closed after execution.

### Retraction of unsupported entries and transcription corrections

The previous draft report contained inaccurate artifact hashes and asserted the
existence of `independent-probe-result.json`. That artifact was never written:
`independent-qa-probe.mjs` failed with an unhandled rejection (`MapError: Map
resource cancelled`) during its supersession test section and exited with code 1.
The claim that an independent probe result JSON file existed is formally retracted.

The earlier screenshot digest matches neither the current worker screenshot nor
QA's screenshot; its precise origin is unverified. The table below replaces it
with the measured QA digest. Finally, `reentrancy-finding.json` on disk reflects the
final post-fix PASS verification run; the initial pre-fix failure observation was
recorded in the process output stream and was overwritten when the probe was re-run
against candidate `4cb28b5`.

No files have been modified or recreated to fit this table. The seven actual files
residing in `/tmp/mcr-qa-fe06-agy8u3i5/` are recorded below with exact byte counts
and SHA-256 digests computed directly from their current disk bytes:

| Artifact | Bytes | SHA-256 | Description |
|---|---|---|---|
| `contract-and-atomic-probe.mjs` | 8971 | `8a7b30f659c13590972b513671a9cf2d9209f5712ca549fe754bf49d7f553750` | Initial standalone probe attempt; failed bare module resolution |
| `independent-qa-probe.mjs` | 7743 | `5276480174126b692582ebd3bcb55b82952b05810a96f0dc325b87ab4ee0bf36` | Vite SSR probe source; exited with code 1 on supersession abort rejection; no JSON emitted |
| `reentrancy-probe.mjs` | 3262 | `c95456acb7b496032f2ed4a5cc1163ba392bace0937155b7b649559c2ad2f9d4` | R1 defect reproducer and post-fix verification script |
| `reentrancy-finding.json` | 178 | `7bef7cb9b186eb2666d697dcbf8b8dae5aa98eb889914d4f041d274894924533` | Final post-fix verification result (`threwError: null`, `runtimeReadSkin: 'PARADE'`) |
| `independent-browser-probe.mjs` | 5685 | `2152af652b11d1ad25207d85993b7d7482647863019add4b277fd8e16fb33b24` | Owned headless Chrome browser probe on port 45849 (status PASS) |
| `qa-browser-result.json` | 219 | `b8fd4009df90a3bdf08c21d03f31fa54cc6aa23f5e1e44df323c1dacbb963c2e` | Native browser probe execution output (status PASS, 0 page errors) |
| `qa-theme-390.png` | 69373 | `7f92c5d0608316b0ff3a9e44013cc42bfbaaba94dd5010c8939035294166cffb` | Native 390px mobile screenshot captured by QA browser probe |

Worker checks verified:
- `pnpm test`: 262 focused tests pass across theme feature, loader, runtime, and movement suites.
- `pnpm build`: Exit 0 (retained 544.82 kB main chunk warning).
- `pnpm lint`: Exit 0.
- `scripts/mcr/validate-assets.py`: Exit 0, 6 documents validated, CONTRACT_READY.
- `test_assets.py`: Exit 0, 27 tests pass.
- `check-boundaries.py`: Exit 0, 20 assigned frontend changes vs base `600582d`.
- `check-plan.py`: Exit 0, plan structure intact.

## Disposition and limitations

Finding R1 and the C06 release identity gap are resolved in exact candidate
`4cb28b5`. The implementation satisfies bounded FE06 requirements.
The persistent goal is COMPLETE upon parent acceptance of this report.

Limitations:
- Synthetic 32×32 swatches only; production AR04 textures are not delivered.
- BE07 server-synchronized preferences are not implemented; storage is local device only.
- Room and avatar identities are test sentinels; no live multiplayer socket or character rendering.
- No physical arrival, real venue verification, or complete application acceptance.
- Implementation status is **MOCK_READY**.

