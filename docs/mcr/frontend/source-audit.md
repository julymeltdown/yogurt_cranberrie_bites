# C01 frontend source audit

Status: **IN_PROGRESS — frontend inventory ready for integrator review**. This is
the frontend contribution to C01, not acceptance of C01, C02, or any FE task. No
Bigyeting application code, JSX, CSS, assets, or visual baselines are copied by
this patch. The proposed extracts remain **NOT_STARTED** and their regression
tests are **NOT_RUN**.

## Authority and pinned inputs

- Worker checkout: `/Users/lhs/Desktop/mcr-worktrees/fe-foundation`.
- Branch: `codex/mcr-fe-foundation`; base:
  `69c17cc0e85eb7209e854a56da0f1388072e00b6`.
- Read-only source: `/Users/lhs/Desktop/mcr-sources/bigyeting`, repository
  `julymeltdown/bigyeting`, commit
  `5eecdcaffd1a9d505b9ab2a974f8d5623217e90a`.
- Source HEAD equals the pin and the source working tree is clean. Inventory
  hashes verify tracked checkout bytes against the pinned Git blob IDs.
- Requirements: root and frontend `AGENTS.md`, implementation plan §§1–4,
  5.1–5.4, 7, and C01/C02/C08/FE01–FE04/FE07–FE11; project assessment and
  execution plan at the worker base. No frozen HTTP/realtime/asset contract is
  claimed at this checkpoint.
- Delivery remains `julymeltdown/mcr/master`, through the integrator. This worker
  commits assigned paths only and does not push or modify the integration tree.

The accompanying [reuse-inventory.json](reuse-inventory.json) is the machine
record: source paths, exact SHA-256/Git object IDs, direct imports, candidate
symbols, dependencies, destination/test paths, adaptations, and source regression
references. Its empty blanket-copy allowlist is deliberate: a candidate file is
not approved for wholesale import. Source paths in this document are relative to
the pinned Bigyeting repository; destinations are relative to MCR.

## Package and lockfile findings for C02

The source declares `pnpm@11.7.0`; `frontend/pnpm-lock.yaml` uses lockfile version
`9.0`, with `autoInstallPeers: true` and `excludeLinksFromLockfile: false`. Preserve
the selected dependency resolutions and integrity records, including peer
contexts, when the integrator reduces the importer. Do not run a general update.

| Retained concern | Manifest declaration | Locked package version |
|---|---|---|
| React / React DOM | `^19.2.7` | `19.2.7` / `19.2.7` |
| TypeScript | `~6.0.2` | **`6.0.3`** |
| Vite / React plugin | `^8.1.1` / `^6.0.3` | **`8.1.3`** / `6.0.3` |
| React Router DOM | `^7.18.1` | `7.18.1` |
| TanStack Query / Zustand | `^5.101.2` / `^5.0.14` | `5.101.2` / `5.0.14` |
| ky / Zod | `^2.0.2` / `^4.4.3` | `2.0.2` / `4.4.3` |
| React Hook Form / resolvers | `^7.80.0` / `^5.4.0` | `7.80.0` / `5.4.0` |
| Tailwind / Vite integration | `^4.3.2` / `^4.3.2` | `4.3.2` / `4.3.2` |
| browser-image-compression | `^2.0.2` | `2.0.2` |
| MSW / Vitest | `^2.14.6` / `^4.1.9` | `2.14.6` / `4.1.9` |
| Testing Library React / jest-dom | `^16.3.2` / `^6.9.1` | `16.3.2` / `6.9.1` |
| Playwright / jsdom | `^1.61.1` / `^29.1.1` | `1.61.1` / `29.1.1` |
| Oxlint | `^1.71.0` | **`1.72.0`** |
| Node / React / React DOM types | `^24.13.2` / `^19.2.17` / `^19.2.3` | `24.13.2` / `19.2.17` / `19.2.3` |
| Phaser | absent | add exact `3.90.0`, integrator-owned |

Full direct dependency declarations and peer-qualified lock selections are in the
JSON inventory. Locked engine declarations include ky `>=22`, Vite/Oxlint
`^20.19.0 || >=22.12.0`, jsdom `^20.19.0 || ^22.13.0 || >=24.0.0`, and Vitest
`^20.0.0 || ^22.0.0 || >=24.0.0`. These permit Node 24 at the declaration level;
they do not establish an installed or executable tree. The initial shell reports
Node `v26.7.0`, which is not the selected project runtime. The parent has announced
an isolated Node `24.22.2` / pnpm `11.7.0` installation under
`/Users/lhs/Desktop/mcr-tools`; this audit has not qualified that environment.
The exact image digest, frozen install and clean frontend/backend builds remain
C02 obligations.

Exclude from the foundation: Mantine, Phosphor icons, old drag/drop and onboarding
packages, `@chenglou/pretext`, Three.js and its types, GSAP, string-tune, motion,
FFmpeg and wasm-webp. None is needed by the proposed minimal extracts. `clsx`,
`tailwind-merge`, and `fake-indexeddb` are conditional on a concrete new-code/test
need; they are not required by this scaffold. Avoid importing the old utility or
media barrel merely to justify retaining these packages.

### Scripts and configuration cutover

The inventory preserves the exact source script strings. They must be reviewed
and replaced, not copied as a functioning MCR toolchain:

| Source setup | Finding / MCR action owned by integrator |
|---|---|
| `build` | `tsc -b && vite build` then two parent-level Bigyeting copy/bundle gates. Retain the compiler/build stages in the new environment and author scoped MCR gates. |
| `lint`, `check:architecture`, `copy:*` | Depend on old FSD/design/product-copy/age-fixture scripts. Do not import old product or database checks. |
| `brand:generate` | Uses old brand asset generator; retire. |
| `e2e`, `e2e:visual`, `e2e:real*`, `container:smoke` | Invoke parent scripts and old full-stack/fixture environments. Do not execute these against source or existing services. |
| `e2e:media*` | Old image/video pipeline and production config; rewrite for image-only MCR. |
| `dev`, `preview`, `typecheck`, `test` | Generic command shapes are useful; they still need MCR config, paths and isolated ports. |
| `vite.config.ts` | React/Tailwind plugins and `@` alias are relevant concepts. Discard old manual chunk rules for Mantine, Three, motion, old features and query/account paths. |
| `tsconfig.app.json` | Strict mode, `noUncheckedIndexedAccess`, `exactOptionalPropertyTypes`, bundler resolution, automatic JSX, and no-emit are useful. Parent owns new settings; do not reuse old app entrypoints. |
| `playwright*.ts` | Old project names, selectors, `.omo` report paths, provider/full-stack flows and screenshots are obsolete. Author separate MCR mock/real/mobile projects and new baselines. |
| `src/test/setup.ts` | Imports old API mock server, branches by filename, and replaces canvas/storage APIs. Rewrite minimal deterministic setup; do not let an old mock or canvas stub certify the world. |

## Minimal extraction decisions

Every row below has exact file hashes, source regression paths and proposed test
paths in the JSON. Test names describe obligations, not executed evidence. Retain
only the selected algorithms; rewrite endpoint/identity/presentation adapters.

| Candidate / owning task | Narrow source | Proposed MCR destination | Required adaptation and regression |
|---|---|---|---|
| Transport / FE01 | `src/lib/api/real/runtime-client.ts` | `frontend/src/shared/api/http.ts`; `shared/media/presigned-upload.ts` | Preserve cancellation and normalized failures; replace hardcoded API host, `/v1`, cookies, CSRF/session response hooks and account context. Explicit retry policy. Test 401/409/429, abort races, XHR cleanup, and no API credentials on signed PUT. |
| Error classification / FE01 | `src/lib/api/api-error.ts` | `frontend/src/shared/api/errors.ts` | Extract error class and network/timeout/abort/schema classification; reconcile C03 envelope. Exclude Korean presentation table and product-specific codes. Test malformed response, unknown code, safe message and request ID behavior. |
| Auth / FE09–FE11 | `real/canonical-auth-client.ts`, `real/client.ts`, `security-contract.ts`, `csrf-token-cache.ts`, `real/runtime-context.ts` | `frontend/src/features/auth/model/session-controller.ts`; `shared/validation/return-target.ts` | Reference only: rewrite around actual Gateway operations, canonical UUID, in-memory bearer, single-flight refresh and approved cookie/origin policy. Do not copy provider availability, 43-character completion token, X/Naver prerequisite, old cookie names or route allowlist. Test unavailable/expired/denied states, concurrent refresh and safe redirects. |
| Queries / FE02 | `src/lib/query/query-keys.ts`, `active-x-account-transition.ts`, `direct-reveal-cache.ts`, `intro-visibility-cache.ts` | `frontend/src/shared/query/{keys,privacy}.ts` | Preserve cancel/remove-before-invalidate discipline, rebuild keys around user/event. Old public-looking link/reveal roots are not uniformly viewer-scoped. Guard late results with identity/policy generation and clear secrets on revoke/block/expiry/logout. Test A→B and event switches with deliberately delayed results. |
| Validation / FE03 | `canonical-social-intro-schema-core.ts`, `...interactions.ts`, `canonical-{contract,response-contract}.ts`; `social-intro-model.ts` | `frontend/src/shared/validation/{text,age-range}.ts`; `entities/intro/model/validation.ts` | Select code-point/tag/range/disclosure checks and `parseTagInput` only. Replace old IDs and copy. Source range requires both-null or both-bounded, half-open upper bound; C04/BE25 decides supported limits. Contact heuristics require policy review rather than a blanket link ban. Test Unicode, null/range boundaries, unknown claims and no public secrets. |
| Image preparation / FE04 | `static-image-compressor.ts`, image portions of `media-policy.ts` and `media-preparation.ts`; optional `webp-animation-metadata.ts` | `frontend/src/shared/media/{compress-image,image-policy,prepare-image}.ts` | Keep same-origin worker URL, EXIF removal and abort. Split image-only types to avoid mixed preparer import. Source limits are not MCR policy. Do not import FFmpeg composition or video/animation codecs. Test final size/type, empty/video rejection and cancellation. |
| Upload lifecycle / FE04 | `canonical-social-intro-media-client.ts`, `...schema-media.ts`, `media-attachment.ts`, `publish-intro-with-media.ts`, `use-social-intro-media-state.ts` | `frontend/src/features/media/model/upload-controller.ts`; `shared/media/preview.ts` | Inject initiate/PUT/finalize/status ports; rewrite old endpoints and social-account identity. Cleanup and receipt reconciliation must distinguish uncertain finalization from failure. Own object-URL revocation explicitly. Test no false completion, no duplicate retry, cancellation, URL release and late callback suppression. |

### Important limits exposed by the inspection

1. `runtime-client.ts` uses included credentials and member/admin CSRF caches,
   hardcodes session-response paths, and reexports X-account context. Its general
   shape is useful, but unchanged transport is incompatible with the chosen auth.
   It also relies on ky defaults; command retry semantics must be explicit.
2. `api-error.ts` is headless but contains substantial product presentation copy.
   Headless does not mean eligible for whole-file reuse.
3. `canonical-contract.test.ts` imports source filesystem/backend DTO fixtures and
   old mock infrastructure. Preserve semantic cases through C07 shared synthetic
   fixtures; never make frontend tests depend on backend implementation files.
4. `canonical-social-intro-schema-core.ts` has useful code-point and half-open
   range checks, but also prefixed IDs, provider and gender/age policy, video MIME
   types, disclosure policy and Korean copy. The parent must freeze those policy
   choices; the source cannot silently amend MCR eligibility.
5. Static image compression imports `MediaProgress` from the mixed preparer.
   `media-processors.ts` directly imports FFmpeg. Extract a tiny image-only type
   instead of carrying this dependency chain into the new build.
6. `media-attachment.ts` creates a blob URL but does not release it. Cleanup lives
   in callers. New preview ownership must cover remove, unmount and identity
   change. Old private IndexedDB draft recovery is outside minimal extraction.
7. Upload catch-and-delete and old draft retry behavior do not establish safe
   uncertain-outcome reconciliation for MCR commands. Server receipts and
   idempotency contracts are required before adapting them.

## Legacy UI exclusion and entry retirement

The pinned frontend has **710 tracked files**. The conservative hash inventory
explicitly rejects **422** files:

| Class | Files | Boundary |
|---|---:|---|
| Legacy JSX/TSX and markup tests | 316 | No entire source components, controls, layouts, pages or markup expectations copied. |
| Entry/router/provider/theme | 9 | Includes old `index.html`, `main.tsx`, `src/app/` route composition and theme providers. |
| CSS | 1 | `src/styles/globals.css` and its tokens are excluded. |
| Assets | 57 | Public branding/icons/taste fixtures, source/script assets and tracked image/font/media files are excluded. |
| E2E/visual/route fixtures | 36 | Old selectors, visual captures and route assumptions are excluded. |
| Video/animation runtime | 3 | `ffmpeg-compressor.ts`, `media-processors.ts`, `animated-webp-input.ts` are excluded. |

The deny policy extends beyond enumerated paths: `.ts` theme objects, animation
choreography, generated brand output and untracked old assets are not reusable
because an extension-based inventory missed them. All other source files remain
unapproved unless a specific extract is reviewed. Pure assertions from TSX tests
can inform new tests; that does not permit copying rendering fixtures or old UI.

Retire the old entry graph rather than importing then hiding it. MCR currently
has no old app runtime to remove. The independently authorized C02 preparatory
scaffold creates only `frontend/index.html`, `frontend/src/main.tsx`,
`frontend/src/app/FoundationApp.tsx`, and `frontend/src/shared/styles/base.css`,
all newly authored. It provides a static landing frame, honest availability copy,
native landmarks and a skip link. It contains no router, auth, queries, mocks,
Phaser, socket, service call or copied asset. It is not FE07 or FE08. The parent
owns manifest/lock/config composition and will qualify build reproducibility.

## Provenance, licensing and data boundary

There is no tracked LICENSE/COPYING/NOTICE file in the inspected source tree and
the private frontend package has no `license` field. Record code provenance even
when the same user controls the repositories; this audit does not establish
source or third-party redistribution rights. Installed dependency license/notice
collection is **NOT_RUN**, as installation belongs to the parent. No imported code
needs a notice added by this patch because no source code is copied.

Only tracked source code/configuration/test fixtures were inspected. No live
database, hardware, credentials, source user data or running service was accessed.
No source script, package install, migration, Docker or provider flow was run.
Do not reuse source API base URLs, database/volume names, or full-stack scripts.
The integrator/backend lane must establish isolated MCR services/databases and
record the process/port map; this frontend inventory does not certify it.

## Evidence and handoff

| Check | Result |
|---|---|
| Source `git rev-parse HEAD` | Exit 0; exact pinned source revision. |
| Source `git status --short` | Exit 0; empty. |
| Pinned blob comparison / inventory generation | Exit 0; every tracked frontend file's bytes match its pinned Git object; candidate/test/exclusion hashes recorded. |
| `python3 scripts/mcr/check-plan.py` | Exit 0, PASS; planning/provenance/exporter checks only. |
| JSON parse, cross-reference and hash validation | Exit 0; seven candidates, 478 hash references, 422 exclusions; source/destination/test references valid. |
| `git diff --check`; protected-source diff | Exit 0; no whitespace errors or changes to `docs/source/`. |
| Source unit/browser suites | NOT_RUN; old scripts/test setup are not an MCR acceptance environment. |
| MCR extraction tests / frozen install / application build | NOT_RUN in this worker; no extracts or local package/configuration. Parent reports its isolated frozen install succeeded with Node 24.22.2, pnpm 11.7.0, retained source resolutions and Phaser 3.90.0. Parent owns build verification after integration. |
| Device / real multiuser / release | NOT_RUN. |

The integrator must review these frontend decisions alongside the backend C01
inventory, consolidate `docs/mcr/source-map.md`, approve symbol-level reuse under
C08, and freeze C03/C04/C07 before feature extraction. C02 must record actual
runtime versions, install/build commands and exit codes. Each future extraction
must add its delivered commit ancestry and executed regression evidence; this
audit does not pre-accept any listed behavior. Audit and scaffold are separate
worker commits descended from the base; final handoff supplies their SHAs and
changed-path lists for serial integration. Only the integrator publishes master.
