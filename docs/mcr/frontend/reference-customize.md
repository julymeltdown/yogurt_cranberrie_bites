# FE-REF-CUSTOMIZE handoff

## Delivery and status

- Delivery repository: `julymeltdown/mcr`, integrator-owned `master`; this worker does not push.
- Worker branch: `codex/mcr-fe-REF-CUSTOMIZE`.
- Base: `f837bef676cd26869c26c172a699a91b7842d832`.
- Source commit: `d7c4577b995cfe4b2b85d07d62defce41973f2c8`; subsequent evidence commit is its direct descendant. Its sole harness correction waits for the loaded candidate before asserting candidate state.
- Presentation: **MOCK_READY**, with production components exercised through deterministic fixture ports and real Chrome/canvas. Approved-art visual comparison and combined root integration: **NOT_RUN**. This is not FE13/IN05 acceptance or a real multiuser proof.
- Required artwork comparison remains open in the worker's ACTIVE persistent goal. Implementation writing stops at handoff; no dependency is falsely marked complete.

## Reference and result

The exact new 1536×1024 target has SHA-256 `d9a8fae2f96a82253542019ab673589b3938627956eace15c8594c702b5c4617`. The comparison displays screen 5 at source coordinates x=1238, y=0, width=298, height=530 beside the actual standalone editor at the same 390px width/aspect. The target is served read-only by the test harness; it is not product artwork or an avatar fallback.

The production editor now shows CUSTOMIZE, a dominant centered existing canvas, cream graph paper, a narrow named icon rail, a black category caption, compact same-owner full-outfit thumbnails, red selection, and Random/Reset/Save. Hair is initially active. Rail order is Hair, Outfit, Accessory, Tones, Presets, Body, Eyes. All seven existing contract categories remain available; no pants/glasses IDs were invented. Choose by ID shares the palette row. The normal draft notice remains available to assistive technology without occupying a separate visual row; errors, progress, reconciliation and saved notices stay visible. Visible interactive controls retain at least 44×44px targets and keyboard focus.

Thumbnails deliberately continue to represent full outfits. The reviewed layer-isolated hair thumbnails in the target cannot be claimed from this collection. The three-option preview bound and existing controller/compositor ownership are unchanged. No Phaser instance, world connection, asset owner, session cache or network endpoint was added.

## Action and palette semantics

Random filters current presets with existing `canSelect`, excludes the current configuration with `sameConfig`, and calls `selectPreset` once. It changes only the local draft and is disabled when no supported different candidate exists. Only explicit Save writes, with the existing expected revision and authorization gates.

Reset uses existing `cancel()` only when editable and no server candidate is loaded. Before reconciliation loads a candidate it restores the persisted draft baseline while retaining unresolved reconciliation. While a candidate is loaded Reset is disabled, requiring explicit Keep my draft or Use saved outfit. This closes a peer-reviewed case where cancel could implicitly adopt the candidate and invoke `onCommitted`. No controller semantics were modified. Reset never promises to undo an uncertain server commit.

Optional feature prop: `tonePalette?: readonly { readonly id: string; readonly color: string }[]`. The root owner supplies current validated manifest skin-tone metadata. The feature intersects current catalog IDs, rejects colors outside six-digit hex, deduplicates IDs, and respects `canSelect` for every swatch. Omitted metadata yields no fabricated swatches; the original tone control remains. Test colors come from explicitly synthetic manifest metadata. Frontendking owns root production wiring; this patch does not modify `app/`, entity adapters or shared contracts.

## Exact remaining dependencies

At this base the selected character source pack is revision 3, production atlas NOT_PREPARED, runtime manifest absent, and hair-layout production eligibility false. Actual reviewed runtime construction therefore fails closed. Supplied originals and `docs/source/` remain unchanged. AR03's six-frame photo_01 front-idle static atlas and consumer revision `1e5022885525ab56f056aa2c24100e18c4bc67ed` are decorative-only: they have no reviewed mapping from AvatarConfig to frame/item/tone, so this feature does not substitute them as saved/draft preview. Required input is a reviewed current manifest/binding that maps supported configurations to admitted source-derived artwork through the existing owner/compositor. No synthetic geometry, legacy image, JPEG crop or newly generated character is admitted as product art.

Root embedding revision `7201b875f9ed6a07a0a77b9ae4aeac2f3f2fbd57` was communicated by the root owner but is not in this worker base. Its PanelFrame supplies ConcertWindowEmbedding automatically, hides only the first top-level window's visual frame/titlebar while preserving its semantic heading, and owns mobile 6px/desktop 8px content spacing. This feature retains the existing semantic ConcertWindow wrapper and requires no opt-in. Captures here prove standalone feature geometry; combined embedded `/me` plus approved artwork remains the integrator's separate gate. Do not interpret these captures as that root proof.

## Validation and evidence

All paths below are repository-relative. Detailed exit codes, source/dependency hashes and evidence hashes are in `frontend/src/features/avatar/__tests__/reference-target/evidence/validation.json`.

| Check | Result | Boundary |
| --- | --- | --- |
| Seven focused Vitest files | 54/54, exit 0 | Controller, preview lifetime, UI, compact/category regression, option previews, new reference actions/palette/reconciliation |
| Strict project TypeScript | exit 0 | `--noEmit --incremental false --pretty false` |
| Scoped oxlint | exit 0 | Three product modules/new presentation/new fixture and granted existing test files |
| Vite production build | exit 0 | Private temporary outDir removed; existing large-chunk warnings remain |
| Native Chrome | See native report | 360/390/430 touch/keyboard, actual same-owner canvas RGBA, 44px controls, no horizontal overflow, no implicit PUT, explicit Save, unknown-result Reset/rebase, bounded ignored-abort churn, privacy teardown, zero-read missing-art fallback |
| Production artwork identity | NOT_RUN | No approved AvatarConfig binding; geometric screenshots visibly labeled TEST ONLY |
| Combined root shell / real backend / multiuser | NOT_RUN | Separate integrator gates |

Reproduction uses retained Node 24.22.2 and the existing locked dependency tree; no installation or manifest/lockfile edits. From `frontend/`, run Node on `node_modules/vitest/vitest.mjs run` with `src/features/avatar/__tests__/{ui.test.tsx,compact-layout.test.tsx,visual-adoption/presentation.test.tsx,controller.test.ts,preview.test.ts,option-previews.test.tsx,reference-target/presentation.test.tsx}` and `--configLoader runner --no-cache`. Types use `node_modules/typescript/bin/tsc --noEmit --incremental false --pretty false`; lint uses `node_modules/oxlint/bin/oxlint` on assigned source/test paths. Native reproduction from repository root is `node frontend/src/features/avatar/__tests__/reference-target/native.mjs`; the exact clipboard target must remain available at the source path in that script. The script guards >8GiB free disk and cleans its own temporary Vite cache/profile/server after execution. Build invokes Vite `build({configLoader: 'runner', build: {outDir: ownTemporaryDirectory, emptyOutDir: true}})` and removes that directory in finally.

Six bounded PNGs under `reference-target/evidence/native/`: target-360, target-390, target-430, uncertain-reset-390, production-unavailable-390, comparison-390. No historical capture matrices were restored. The comparison explicitly disposes its original fixture owner before opening its isolated comparison iframe.

The initial legacy assertions failed only for old DRESS UP/Cancel labels (43/47 pass). Hair-first then exposed four setup assumptions in option-previews (50/54 pass). Frontendking explicitly granted heading/Reset-label and equivalent Hair-first navigation/setup changes in ui.test.tsx, compact-layout.test.tsx, visual-adoption/presentation.test.tsx, and option-previews.test.tsx. All prior behavior assertions remain; setup now selects Presets explicitly when required and keyboard/Home assertions follow the genuine seven-category order. The final native candidate check initially observed reconciling before the candidate loaded; the harness now waits for Keep my draft, without weakening its candidate/reconciliation assertions. `initial-failures.json` retains that history.

## Provenance, ownership and cleanup

New production presentation and UI glyphs were authored here; no legacy rendering was copied. Existing headless controller, HTTP adapter, preview owner and runtime compositor are unchanged at base. New native fixture scaffolding is adapted from this same base's `__tests__/compact-layout/{preview.tsx,native.mjs,preview.html}` and reuses read-only `__tests__/option-previews/data.ts`, `__tests__/fixtures/harness.ts`, `world/avatar/__tests__/fixtures/png.mjs`, and committed contract fixtures. Synthetic assets are confined to test paths. Retained dependencies include React, Vite, Vitest, testing-library and Playwright; no dependency change.

Changed paths comprise AvatarEditor.tsx, OptionStrip.tsx, avatar-editor.css, new reference-presentation/**, new __tests__/reference-target/**, the four exact granted existing tests, and this document. No backend, root, shared-kit, entity, original art, source archive, manifest, lockfile or contract registry writes. Source/evidence hashes identify the exact delivered bytes. Integrate source then evidence serially, compare the full changed-path list, and run combined root/art checks after genuine binding/wiring arrives. No worker push occurred.

Only this checkout was sparsified with explicit authorization to omit backend and old PNGs; selected public originals remain. The worker's temporary dependency symlink is removed before clean STOPPED handoff. Only verified worker-owned private cache/profile/build directories are removed. Disk guard and final free space are recorded in the validation artifact; no other worker's checkout/cache is deleted.

Final disk audit: **7.542 GiB**, below the required 8 GiB floor after concurrent external growth. Browser/build/capture work stopped immediately; all worker-owned temporary runtime artifacts and the dependency symlink had already been removed. Coordinators were notified. Existing source and evidence are preserved in Git; future artifact work requires restored headroom.
