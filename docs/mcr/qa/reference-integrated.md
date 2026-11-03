# FE-REF-INTEGRATED-QA

Status: IN_PROGRESS. Deterministic composed-root evidence only. Full visual acceptance, live-service journeys and physical-device acceptance remain NOT_RUN.

## Scope and provenance

Exclusive ownership: new `frontend/e2e/reference-integrated/**` and this receipt. No application, contract, manifest, lockfile or approved artwork changes. Initial assignment base: `e6b55d57e0155016bd81e9cf3ad45887362b55b8`; later published ancestry is reconciled before capture. The harness adapts `frontend/src/app/__tests__/default-shell/illustrated-overview/fixture.tsx` into its own preview. It calls actual `createAppRoutes`, `createProductionPorts`, canonical Auth, resolver and Phaser. No panel overrides or replacement page frames.

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

Warm navigation follows World → Me → Customize → World → Customize, then actual Parade/Danger/Revenge controls, then supported avatar actions. Assertions check a single Phaser instance/connection, stable camera object/geometry/outfit across navigation/eras, zero draft PUTs and one canonical Save PUT only when enabled by the current production composition. Unavailable avatar authority yields explicit NOT_RUN rather than a passing save claim.
