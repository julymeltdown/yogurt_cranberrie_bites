# FE-AVATAR-UNION-ADMISSION

Status: **CONTRACT_READY** for canonical frontend admission. This document records scoped checkpoints; it does not accept rendering, server persistence, live bootstrap, IN01 or release behavior.

## Ownership and provenance

The reused exclusive checkout is on `codex/mcr-fe-avatar-union`, initially based on freshly fetched `a4ae8555af8d43b1cddf8b07d71f48d14ad20940`. Previous FEED work is already published independently and is not republished here. A new unbudgeted persistent goal was created after `get_goal` returned no unfinished goal.

Original ownership: generated `frontend/src/world/network/transport/wire-types.ts` through the existing invocation-only generator, avatar `runtime/contract.ts` and `runtime/schema.ts`, new `whole-admission/**`, and this document. Frontendking then explicitly transferred the Auth model contract/bootstrap; Gift, Feed, Party discovery/authoring, Meet Beacon and admin event model contracts; and subsequently Me/People model contracts to this owner. No runtime index/resources/lease files, whole-character renderer, old avatar-editor model, UI, shared canonical contracts, manifests or lockfiles are owned here.

Actual contract checkpoint `3398d8331863775ecc0180e5eccebddd179fedd5` is published in master. Its canonical config/manifest schemas and `contracts/fixtures/assets/selected-static/` fixtures define the disjoint 1.0.0 layered and 1.1.0 whole-character branches. Whole manifest ID/version: `mcr_selected_static_20261103`/1; content hash `sha256:0627c7d178b22535a561e1270db6d8bf9e2383ba84d03a63962e48e565c59b2b`. No schema or artwork was authored or copied by this worker.

## Compatibility repair checkpoint

The new canonical union exposed missing `oneOf` support in five generic public contract compilers and the layered asset evaluator. Gift eagerly compiles reachable owner schemas at module import, so this rejected even retained layered fixtures and prevented root admission before feature initialization. The repair supports exactly one matching branch and retains unknown-keyword/ref rejection. It does not reinterpret `oneOf` as `anyOf` or remove validation.

Changed runtime contract types explicitly restrict `AvatarConfig` to `Extract<WireAvatarConfig,{contract_version:'1.0.0'}>` and tag its manifest as 1.0.0. Entry guards reject a valid whole config/manifest with a version error before any layered access or approved-layered-fallback logic. Asset schema admission supports canonical `oneOf` and array bounds. Meet Beacon and admin event use explicit closed Zod schemas rather than the affected compiler and need no repair edit.

Affected baseline validation at contract-containing master `335066396502782426c4b86ae8aacb9704def988`: 328 tests in 23 files passed, exit 0, covering Auth contract/bootstrap, Gift, Feed, Party discovery/authoring, Places, places-entry and reference-root. Command with pinned Node24.22.2: `node frontend/node_modules/vitest/vitest.mjs run --root frontend --no-cache --maxWorkers 1 src/features/auth/__tests__/contract.test.ts src/features/gift-discovery src/features/event-feed src/features/party-discovery src/features/party-authoring src/features/places src/app/places-entry src/app/__tests__/reference-root/root.test.tsx`.

The added `whole-admission/__tests__/compiler-compatibility.test.ts` imports the real eager Gift decoder, complete Auth bootstrap decoder and public schema consumers. It checks both valid canonical config branches and rejection of mixed/unknown values. Broader canonical manifest/model admission is a followup; the repair alone does not claim whole-character bootstrap semantics or render acceptance.

Repair delivery: source `58824c4f8ece5a32afba5fd63c6272cb95e569f5`, published non-force through reconciliation commit `90f44fd99fdfe17f3ee3e596ceff0583f6db2c47`, whose remote ancestry was fetched and verified. The standalone candidate passed 413 tests in 33 files, strict full frontend TypeScript, scoped lint and whitespace checks, all exit 0. Map subsequently reported its actual production-root native 360/390/430 checks and 111 targeted tests passing; integrated QA and Auth/Profile also reported their retained root admission restored. Those are peer-reported repair results, not a new whole-character end-to-end claim by this author.

## Canonical union followup

The generator-only wire update uses `frontend/src/world/network/__tests__/generate-types.mjs`; no generator source or canonical schema is edited. `AvatarConfig` is now the actual generated `LayeredAvatarConfig | WholeCharacterAvatarConfig`. C05 snapshot and delta tests verify both variants coexist, retain exact fields/revisions and round-trip without conversion; invalid or mixed variants are rejected. The old layered runtime and editor continue receiving only the explicit 1.0.0 extraction.

The new pure entrypoint `frontend/src/entities/avatar/whole-admission/index.ts` exports:

- `AvatarConfig`, `LayeredAvatarConfig`, `WholeCharacterConfig`, `WholeCharacterPresetId`, `AvatarState`, `AvatarManifest`, `LayeredAvatarManifest`, `WholeCharacterManifest`, `ManifestPin` and `AvatarError`.
- `parseConfig(value)`: validates the canonical closed config schema and returns an independent immutable union value. Profile reads without a manifest use this shape admission; it does not certify resource availability or layered item membership.
- `decodeAvatarState(value)`: requires exactly `revision` and `config`, a positive safe integer revision, and canonical config admission. No field stripping or fallback.
- `validateConfig(value, validatedManifest)`: additionally checks contract version, manifest identity/version and actual preset/layer membership. Callers supply the already verified manifest.
- `decodeConfig(text, validatedManifest)`: applies the existing strict C06 parser before config admission, rejecting duplicate/escaped-duplicate keys, lexical floats/exponents, oversized input and trailing data.
- `validateManifest(text, pin)`: strict text/schema admission, then dispatch to the retained layered validator or the published whole-character pure contract validator. This reuses the reviewed whole manifest hash/source/texture/frame policy; it imports no renderer, lease pool or resource loader and performs no fetch.

The whole branch delegates to `whole-character/contract.ts` from published producer `9fbc89426c6d0781c6f7a1f1a78ef722656407e5`. There is no dependency cycle; that pure producer contract depends on canonical schemas and C06 JSON utilities. Tests pass each admitted config through the real producer validator unchanged and reject even a schema-valid rehashed alternate default whose release hash differs from the reviewed pin. Layered manifests retain their original membership, geometry and hash checks. The selected runtime itself remains separately owned.

Auth `validateBundle` now branches by avatar manifest version before layered atlas/tone access. Whole admission uses the exact reviewed manifest validator and current avatar membership, while preserving the common immutable snapshot, Auth user/event/profile identity, profile-avatar equality, capability/phase rules, map geometry/reachability and every theme/map/avatar hash binding. A real-shaped whole bootstrap and adversarial identity, branch, frame, hash, theme-pin and read-only cases exercise the actual public decoder.

Me and People now decode the true canonical `AvatarState`; their previous layered-only types/transforms were unsound or rejected whole public profiles. Feed/Gift public author/owner types also reference this union. Party discovery already exposed an honest `unknown` avatar behind canonical response validation, so its type is retained. No old layered editor/controller is widened. The existing replication and account-owner tests each add an explicit 1.0.0 narrowing before their original item/accessory assertions; these two exact test paths were separately granted by Frontendking, with original assertions retained.

The display guards are supplied by the separate published lease checkpoint `c987a20b60ae9dc39fb16bd8a7d180f22c4991d2`. This followup changes no runtime index/resources, thumbnail/profile UI, root composition, whole-character renderer or art. Layered display components safely report unsupported whole values until the root provides its whole-character renderer; no 1.1.0-as-1.0.0 cast is used.

## Pins and final qualification

Reconciled validation base: `8430cd57ebb628fc093cb43f5ab547f042440830`. Canonical contract source remains `3398d8331863775ecc0180e5eccebddd179fedd5`. SHA-256 values:

| Input | SHA-256 |
| --- | --- |
| `contracts/assets/v1/avatar-config.schema.json` | `615eba53a0cc4810c7fd1a1c71235a18e6a237bf40f5bd931070b8b1b3c35bb4` |
| `contracts/assets/v1/avatar-manifest.schema.json` | `88a3f3899eb29aada6b05ef5c8f0eca2f4148a33dfca51db9c254a6a687753f8` |
| `contracts/assets/v1/common.schema.json` | `b7be87ce387dd286bcd2ea895da04f1a8b886c1ea3dc35494f761ac05aad75b4` |
| `contracts/realtime/v1/world.schema.json` | `1732e627037fe91533ac6f7fe74e36683ce9b846bfbfc020b3cce298ae84df36` |
| `contracts/core/v1/core.schema.json` | `9ddf3cd3c853ef6a8258a6215c546bd84eccde620ee73b636888d868cfca1c88` |
| selected whole manifest file | `04b0815c5ad66729c578236b010974e44274e6562a9e9dd4599a861e2eed485e` |
| retained `frontend/pnpm-lock.yaml` | `47ca44459819ad0e887d2f4f93394fdf3b1053b8f45edc9ceb0236981feee1f6` |

The manifest file hash differs from its C06 `content_hash`, which is computed from canonical content excluding the hash field. Generated wire headers preserve their own exact four schema hashes. Source-file hashes and final check results are retained in `whole-admission/__tests__/evidence/qualification.json`.

Validation uses retained Node24.22.2 dependencies through an authorized temporary symlink; no installation, lockfile resolution, extra worktree or dependency copy. Python oracle tests use the existing `/tmp/mcr-fe14-format-venv` on PATH. The initial People Unicode oracle attempt ran from the repository root and could not resolve its frontend-relative script; the corrected frontend-cwd run passed all 481 then-selected tests. No production change was made to that oracle. A subsequent 882-test combined run passed before the final reviewed-manifest dispatch; 361 affected tests and strict TypeScript passed after that dispatch. Final combined results are recorded below and in the receipt.

The product-root Vite8.1.3 bundle passed with `publicDir:false` and output `/tmp/mcr-fe-avatar-union-build`, compiling actual application imports while avoiding public asset copying. The pre-build disk guard recorded 35,960,115,200 bytes available, above 8 GiB. Existing >500 kB chunk advisories remain. This is a production compilation check, not a deployed or native-root whole-character flow.

Remaining gates: live backend save/reload/default and real API/bootstrap/World integration, root whole-character binding, multiuser/device validation, full AR03 layered/directional animation and IN01/IN05 remain **NOT_RUN** here. No browser screenshots or new art are produced by this data-admission task. Compiler/schema/unit evidence does not certify those boundaries.

Final combined qualification at the recorded base: **909 tests in 61 files passed**, exit 0. Strict full frontend TypeScript, generator `--check`, scoped lint, root production build, planning provenance and whitespace checks also returned 0. The receipt records commands, source hashes and precise limits. Publication uses explicit staged paths, a fresh remote-history reconciliation and a non-force push; delivered SHA/remote ancestry are reported directly to the assigned owners.

Union implementation source: `e614997db5dc54bd688c86df1bb94d96d80c4fc4`. Fresh remote reconciliation produced `496aa2d5810b5054dfca8792d72c0e2ca9ce5661`, preserving the concurrently published Auth/Profile UI ports. All qualified source hashes remained unchanged; strict TypeScript and generator drift checks passed again, and 349 affected admission/Auth/Me/People/Profile/account/root tests in 28 files passed after that reconciliation, all exit 0.
