# FE-REF-INTEGRATED-QA

Status: INTEGRATED baseline QA evidence; visual acceptance BLOCKED. Selected-character default-production verification remains IN_PROGRESS as the next assigned checkpoint. Deterministic composed-root evidence only. Full visual acceptance, live-service journeys and physical-device acceptance remain NOT_RUN.

## Scope and provenance

Exclusive ownership: new `frontend/e2e/reference-integrated/**` and this receipt. No application, contract, manifest, lockfile or approved artwork changes. Initial assignment base: `e6b55d57e0155016bd81e9cf3ad45887362b55b8`; later published ancestry is reconciled before capture. The harness adapts `frontend/src/app/__tests__/default-shell/illustrated-overview/fixture.tsx` into its own preview. It calls actual `createAppRoutes`, `createProductionPorts`, canonical Auth, resolver and Phaser. No panel overrides or replacement page frames. **The world `load` option is overridden by the fixture**: it uses `createWorldComposition`, a custom resolver and `loadPhaserEngine`, bypassing the default production world loader/renderer. This is actual root-route composition with a fixture world, not proof of the default production renderer or genuine selected-character integration.

The original ten-phone target remains byte-identical: `docs/mcr/design/reference-target-2026-11-04.png`, 1536×1024, SHA-256 `d9a8fae2f96a82253542019ab673589b3938627956eace15c8594c702b5c4617`. `cases.mjs` records manually measured bounds for each unequal phone. Each capture viewport uses that individual aspect, rounded by at most half a pixel; comparison HTML crops the original without stretching. Comparison labels sit outside the crop; opacity control supports overlays.

Fixture clock: `2026-11-07T07:00:00.000Z`. Fixture responses are isolated in `data.ts`; actual DTO fixture generators provide rows with root event/user scope rebound. Real HTTP services and real WebSockets are not contacted. C05 MockSocket supplies only authentication/room snapshots, not authoritative movement simulation. Synthetic C06 metadata and test-only review wrappers remain explicitly non-product evidence.

## Authority boundaries

AR03 selected static release `static-release.json` SHA-256 `cc53c6852742e9cbb9858a2d0961411e1a9e131d1e30c0f2b73b265e03f91677` references the six-front-idle atlas `e124c88815b22220df2f2dc59e2f26b3ceb845e57b77674fd8e55383116d7efe` (36,030 bytes). Original photo01 is `67448360482a0dd68172d6089378a002190142bb8bd804818c89a86370154311` (168,045 bytes). The runner verifies these source bytes and hashes actual image Blob bytes used by visible gallery spans. Static character IDs are not translated into AvatarConfig IDs. Decorative gallery use does not establish identity, layered editing or walking support.

Current production ports do not provide an approved account-assets binding by default. Canonical avatar previews, draft/randomize/save sequences and identity portraits must remain unavailable or NOT_RUN unless that actual authority arrives. The harness does not fabricate mappings, owner/member portraits, gift media or mural anchors. Virtual coordinates do not certify physical arrival.

## Historical attempts

- First attempt on `06c8008f`: PARTIAL. Harness captured public lazy fallbacks and required room state on hidden panels. Those were harness defects, not visual product findings. Corrected waits now require actual public content, published composition on protected routes, room only for World, and ready selected-static art.
- Debug attempt on `f4ed446ae7821984ed30f950c4b338cc3407adb4` above published `33506639`: all six attempted routes failed before QA bridge initialization. Published C06 `oneOf` reached the Gift eager compiler, which rejected the keyword (`contract.ts:49`, `Invalid giveaway response`). The admission owner owns the repair. No QA app patch or contract downgrade. Earlier 617-check evidence predates this regression and is not current green.

## Reproduction

Use retained dependencies and the pinned Node 24.22.2 runtime. A temporary owner-created `frontend/node_modules` symlink may point to the integration checkout's retained dependency tree; remove it after validation. No install, dependency copy or new full checkout is required.

```sh
node --test frontend/e2e/reference-integrated/metrics.test.mjs
node frontend/node_modules/oxlint/bin/oxlint frontend/e2e/reference-integrated
node frontend/node_modules/typescript/bin/tsc --noEmit --strict --noUncheckedIndexedAccess --exactOptionalPropertyTypes --noUnusedLocals --noUnusedParameters --module ESNext --moduleResolution Bundler --target ES2022 --lib ES2022,DOM,DOM.Iterable --jsx react-jsx --resolveJsonModule --allowImportingTsExtensions --skipLibCheck frontend/node_modules/vite/client.d.ts frontend/e2e/reference-integrated/preview.tsx frontend/e2e/reference-integrated/data.ts
MCR_REFERENCE_INTEGRATED_OUTPUT=/absolute/new/owned/output node frontend/e2e/reference-integrated/native.mjs
python3 scripts/mcr/check-plan.py
```

The runner requires a new absolute output directory and ≥8 GiB free space before starting and before every browser experiment. Native Chrome runs headlessly with service workers blocked and actual timers; Date is pinned. `QA_CASE` and `QA_WIDTH` permit bounded diagnosis and always label the report PARTIAL. Default runs cover all ten routes at 360/390/430 plus three warm experiments.

## Measurement semantics

Loader calls count resolver invocations, including synthetic fixture paths. Fixture HTTP calls count intercepted Request objects separately. CDP counts actual browser network requests and encoded transferred bytes, including development modules; these are not production bundle budgets. Browser memory/disk/service-worker cache flags are independent of loader reuse. `HTMLImageElement.decode()` instrumentation records explicit API calls/resolutions/rejections; it does not count all CSS/GPU decoding. Blob hashes verify bytes and do not certify decode reuse. Warm steps preserve cumulative counters and per-path counts so deltas can be computed without conflating request categories.

Warm navigation follows World → Me → Customize → World → Customize, then actual Parade/Danger/Revenge controls, then supported avatar actions. Assertions check one persistent Phaser instance and peak one active connection. Covered full panels intentionally suspend the connection; returning reconnects, so total connection creations can exceed one. Camera object and geometry remain stable; outfit equality is checked when replication is published, zero draft PUTs and one canonical Save PUT only when enabled by the current production composition. Unavailable avatar authority yields explicit NOT_RUN rather than a passing save claim.

## Visual review and functional limits

All ten 390px side-by-side comparisons were inspected, with the original phone aspect preserved. No full visual acceptance is claimed. Differences are material and recorded per screen:

| Screen | Observed difference or limitation | Classification |
| --- | --- | --- |
| Landing | Approved source-pack characters render; wordmark, spacing, moon/skyline and texture differ from target | Visual gap |
| Login | Working canonical email sign-in; different paper/heading geometry; no Remember me; provider controls unavailable | Visual and capability gap |
| World | Different scenery/framing, no approved self/other avatar image, extra tools/status band | Visual and authority gap |
| Map | Latest published map fills panel with landmark plaques/zoom; supplied scenery, framing and labels still differ | Visual gap; independent map action evidence belongs to Map owner |
| Customize | Preview unavailable; category/item/tone editing and Random/Reset/Save gated | Authority gap; draft/save NOT_RUN |
| Feed | One fixture post, initials and no media vs target's illustrated multi-post layout; compose/footer position differs | Fixture coverage and visual gap, not evidence of server data loss |
| Meet | Three rows/capacity counts render, JOIN opens details without applying; portraits/member strip missing, Create disabled | Authority/composition gap; no physical arrival claim |
| Gifts | Three rows with honest No photo; original undersized controls/density finding retained historically and checked against published repair below | Touch-target finding plus target-density difference; below-fold alone is not a functional failure |
| Wall | Unavailable message, no qualified anchor, grid, palette or editing toolbar | Authority gap; mural edit/sync NOT_RUN |
| Me | Refreshed dark profile header and activity tabs render; approved avatar and recent activity remain unavailable, with remaining spacing/social identity differences | Authority and visual gap |

Gifts touch targets were handed to the existing Gifts owner through the frontend coordinator. QA owns no application fixes. JOIN and RESERVE remain navigation into the actual detail/reservation journey and do not create application/reservation writes. Detail data/authorization and real command outcomes are outside these fixture assertions; a successful route transition is not server-flow acceptance.

The target-density finding must remain distinct from functional reachability: scrolling or browser focus can reveal a row outside the initial crop. The native test activates the first row and checks focus restoration; it does not certify every offscreen row reachable. Newly authored figures and labels are QA evidence, not replacement application UI.

## Baseline delivery evidence

The final baseline application source is candidate `e5a578e9028fa5a4c5bc8feb083087a111d3d7e6`, based on published `8430cd57ebb628fc093cb43f5ab547f042440830`. Default capture: **30/30 CAPTURED**, **3/3 warm experiments PASS**, exit 0. The corrected visible-only geometry instrumentation records public frames/title bars, world header/era strip, panel/body/tab/footer/control/image bounds. World navigation geometry was subsequently refreshed at `0c98ca85cf76b50c03e387424fee03e36c5da4f6`: 3/3 captured, no errors or undersized controls; scoped-run exit 1 deliberately means PARTIAL selection, not a failing route. `report.json` preserves the baseline, per-case candidate and reconciliation pins.

Earlier Map refresh at `5c7c49533f5000ffd129c78f2867c6f4e39fad3f` above published `f4b40808` captured 3/3 after the actual Map implementation landed. Its raw receipt and prior combined baseline are in `evidence/history/`; the final 30-screen baseline already includes that Map. Historical screenshots/findings retain historical pins; they are not relabeled current.

Baseline findings: zero horizontal overflows/page errors across 30 cases; all ten route actions PASS at each width. Focus outlines are present on exercised protected controls; Meet/Gifts detail/back restores the originating link. The first GIFTS Reserve is route-only/read-only; no application/reservation mutation occurs. Gifts title links measure 26px high; filter/My gifts controls 36px at all widths. The 390px row heights are approximately 147.5px, with the third row extending past the initial viewport. That is a density difference, distinct from the touch-target defect and from scroll reachability.

All three warm experiments agree: cold World resolves 5 loader calls/4 explicit successful image decode calls. Me/Customize/return navigation adds no asset loader calls or decodes. First Parade and Danger each add 4 loader calls, 4 browser asset requests and 4 successful explicit decodes. Return to Revenge adds **0 loader calls, 0 browser requests, 0 transferred bytes, 0 HTTP cache hits, but 4 explicit decode calls**. Byte reuse does not establish decoded-image reuse. Phaser mounts/peak remain 1; socket peak remains 1, with 0 active during covered panels and 1 after return (3 total creations). Same camera object and geometry persist; replicated outfit matches on published world snapshots. Canonical avatar draft/save remains NOT_RUN because approved account authority is absent in this fixture composition.

Validation on the captured/reconciled QA sources: 3 helper tests PASS (exit 0), strict E2E TypeScript command above PASS (exit 0), scoped lint PASS (exit 0), planning check PASS (exit 0), native full capture PASS (exit 0), ten comparison artifact renders PASS (exit 0). An intermediate lint accidentally traversed its own Vite cache and reported dependency warnings; the owned cache was removed and scoped lint rerun cleanly. Build/live/database/device/release checks are not inferred from these results.

Comparison artifact rendering is a separate static-file browser check: 30 HTML cases exist, target image is 1536×1024, each actual image matches its captured viewport, and ten 390px pairs render with labels outside the image crop. This verifies the comparison artifact, not another application run. `comparison-render.json` records that distinction.

## Next assigned checkpoint

The frontend root owner is integrating the published selected whole-character producer, canonical registry/provider, default production World adapter and whole-character editor. Keep this QA goal active for the exact published root/union/editor/World pins and configured fixture. Exercise the **default production loader** (or explicitly disclose any remaining override), then prove canonical Save → World → Me → Customize propagation, no draft PUTs, one Save PUT and actual approved bytes without synthetic C06 approval or ID fabrication. Preserve this bounded baseline; do not replace it with a claim of genuine whole integration. No full visual/live/device acceptance follows from the baseline.

## Affected-source followups

Published profile changes through `6ccb4e8f` and World adapter `677f958e` were reconciled without repeating unrelated screens. Candidate `0569a9b10fd7070ce1c8c7759f52a28727cfdf6d` captured World and Me at all three widths (6/6, zero automated findings). Me now has the dark header and activity tabs; its avatar preview/recent activity remain unavailable. Whole World rendering is not certified here because the fixture still selects legacy metadata and overrides the production loader.

Gifts owner published the exact touch/density repair `9055af1e03a5418457e32b1a8d125c5d0e49d823`. QA refresh uses the same owned three-row fixture and actual root routes, not the author's separate native harness. Historical 26px/36px measurements stay in `history/initial-report.json` and the original comparison. New per-case candidates and raw `refresh-gifts.json` distinguish the repaired source. Product photos, canonical identity and live reservation outcomes remain unqualified.
