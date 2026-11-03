# FE-AVATAR-UNION-ADMISSION

Status: **IN_PROGRESS**. This document records scoped checkpoints; it does not accept rendering, server persistence, live bootstrap, IN01 or release behavior.

## Ownership and provenance

The reused exclusive checkout is on `codex/mcr-fe-avatar-union`, initially based on freshly fetched `a4ae8555af8d43b1cddf8b07d71f48d14ad20940`. Previous FEED work is already published independently and is not republished here. A new unbudgeted persistent goal was created after `get_goal` returned no unfinished goal.

Original ownership: generated `frontend/src/world/network/transport/wire-types.ts` through the existing invocation-only generator, avatar `runtime/contract.ts` and `runtime/schema.ts`, new `whole-admission/**`, and this document. Frontendking then explicitly transferred the Auth model contract/bootstrap; Gift, Feed, Party discovery/authoring, Meet Beacon and admin event model contracts; and subsequently Me/People model contracts to this owner. No runtime index/resources/lease files, whole-character renderer, old avatar-editor model, UI, shared canonical contracts, manifests or lockfiles are owned here.

Actual contract checkpoint `3398d8331863775ecc0180e5eccebddd179fedd5` is published in master. Its canonical config/manifest schemas and `contracts/fixtures/assets/selected-static/` fixtures define the disjoint 1.0.0 layered and 1.1.0 whole-character branches. Whole manifest ID/version: `mcr_selected_static_20261103`/1; content hash `sha256:0627c7d178b22535a561e1270db6d8bf9e2383ba84d03a63962e48e565c59b2b`. No schema or artwork was authored or copied by this worker.

## Compatibility repair checkpoint

The new canonical union exposed missing `oneOf` support in five generic public contract compilers and the layered asset evaluator. Gift eagerly compiles reachable owner schemas at module import, so this rejected even retained layered fixtures and prevented root admission before feature initialization. The repair supports exactly one matching branch and retains unknown-keyword/ref rejection. It does not reinterpret `oneOf` as `anyOf` or remove validation.

Changed runtime contract types explicitly restrict `AvatarConfig` to `Extract<WireAvatarConfig,{contract_version:'1.0.0'}>` and tag its manifest as 1.0.0. Entry guards reject a valid whole config/manifest with a version error before any layered access or approved-layered-fallback logic. Asset schema admission supports canonical `oneOf` and array bounds. Meet Beacon and admin event use explicit closed Zod schemas rather than the affected compiler and need no repair edit.

Affected baseline validation at contract-containing master `335066396502782426c4b86ae8aacb9704def988`: 328 tests in 23 files passed, exit 0, covering Auth contract/bootstrap, Gift, Feed, Party discovery/authoring, Places, places-entry and reference-root. Command with pinned Node24.22.2: `node frontend/node_modules/vitest/vitest.mjs run --root frontend --no-cache --maxWorkers 1 src/features/auth/__tests__/contract.test.ts src/features/gift-discovery src/features/event-feed src/features/party-discovery src/features/party-authoring src/features/places src/app/places-entry src/app/__tests__/reference-root/root.test.tsx`.

The added `whole-admission/__tests__/compiler-compatibility.test.ts` imports the real eager Gift decoder, complete Auth bootstrap decoder and public schema consumers. It checks both valid canonical config branches and rejection of mixed/unknown values. Broader canonical manifest/model admission is a followup; the repair alone does not claim whole-character bootstrap semantics or render acceptance.
