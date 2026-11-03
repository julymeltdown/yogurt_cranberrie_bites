# FE09 session controller and sign-in

## Delivery and status

FE09 is **MOCK_READY**. The browser contract adapter and deterministic tests are implemented; real BE03 cookie journeys, a production event, actual world resources and IN01 are **NOT_RUN** by this frontend worker. Integration review is pending. This handoff does not mark FE09 or a multiuser flow ACCEPTED.

Assignment base: `2a5c8e6a1f15742bad722220b87850f6e26fa270`. Branch: `codex/mcr-fe-FE09`. Implementation: `983dfb2` (`Implement FE09 session controller and sign-in against C03`). The documentation commit follows it. Only the integrator publishes reviewed commits to master.

The integrator supplied prerequisite `153a3dc9d1ba11facead968e4980adbd659aaf89`, cherry-picked separately as `0e2874fd882ed814b2a1f45495bbb6c4c772d0af`. It publishes the canonical public contract and its single-assembler check. Its contract/backend-generator paths are integrator changes, not FE09 ownership. Cherry-pick the FE09 implementation and documentation only when integrating into master, where the original prerequisite already exists.

## Provenance and changed paths

C03 core version `0.1.0`, C06 assets version `1.0.0`, FE01 transport, FE02 privacy and FE07 shell are the authorities. The decoder consumes `contracts/generated/mcr-core.openapi.json` read-only, per `docs/mcr/browser-contract.md`; it imports no backend implementation. No independent wire schemas or endpoint discovery are introduced. Semantic bundle checks follow the frozen C03/C06 invariants and the repository's offline validators. No Bigyeting rendering or new dependency was copied.

| Paths | Behavior |
| --- | --- |
| `frontend/src/features/auth/model/contract.ts` | Narrow compiled validator of the reachable canonical definitions, exact auth allowlist, canonical identities, strict fields/formats, provider completeness, login UTF-8 bounds. Unknown assertion keywords, unsupported formats and recursive references fail closed. |
| `frontend/src/features/auth/model/bootstrap.ts` | Full bootstrap context/policy checks, C06 canonical SHA-256, avatar/layer/preset/atlas bindings, geometry dimensions/collision/reachability, place provenance and all theme cross-bindings before the non-secret shell projection. |
| `frontend/src/features/auth/model/session.ts` | One memory SessionPort, one QueryClient/privacy coordinator, login/logout/expiry, generation and intent fences, bounded qualified single-flight read recovery and cleanup registry. |
| `frontend/src/features/auth/model/cookie-policy.ts` | Native origin Web Lock for cookie writers; credential-free BroadcastChannel revocation; bounded lock wait, uncertainty latch and capability failure handling. |
| `frontend/src/features/auth/ui/SignIn.tsx`, `sign-in.css` | New labelled/autofill form, pending/double-submit protection, neutral credentials and throttling feedback, safe return, disabled frozen OAuth, truthful local/server logout states. Quiet typography and symmetric controls, with keyboard focus and no decorative left accents. |
| `frontend/src/app/ports.ts`, `PublicBoundary.tsx`, `router.tsx`, `frontend/src/main.tsx` | Exact delegated composition: optional auth port preserving FE07 fixtures; login page; single QueryClientProvider; explicit validated event input and one application-lifetime controller outside StrictMode. |
| `frontend/src/features/auth/__tests__/**` | 78 focused cases and isolated native-Chrome HTTP/cookie fixtures, screenshots and reproducible browser checks. Fixtures are excluded from the production entry. |

FE01/FE02/shared styles, shell lifetime, route registry, assets, manifests, lockfiles, configuration, backend, generated inputs and `docs/source/` were not modified by FE09. No retained dependency resolutions changed. Runtime imports were reviewed: feature model to public contract/shared browser facilities/app port types; UI to existing shell context/navigation; no cross-lane imports.

## Runtime policy and integration seam

`VITE_MCR_EVENT_ID` is read only in `main.tsx`, checked as a canonical lowercase UUID and passed explicitly to composition. Missing/invalid input means event entry unavailable and no auth/bootstrap/discovery requests. No production value was supplied, fixture event default is absent, and query parameters/storage cannot select an event.

Call the controller's `request` for protected feature requests; it returns FE01's scoped result and fences its final promise boundary. It rejects direct auth paths and uncoordinated `refresh` options. The controller owns login/logout. Private queries use `auth.queryClient` with `auth.privacy.options`; drafts, mutation abort/reset callbacks and private subscriptions register with `auth.privacy.registerCleanup`. Identity replacement immediately aborts old scope, clears private observer/cache/mutation state and runs cleanups. The existing FE07 scope-abort boundary releases the world lease. Bearer credentials are never placed in query data, DOM, URL, browser storage or cross-tab messages.

Login passwords are measured without trimming/normalizing: **1–128 UTF-8 bytes** for inherited LoginRequest. Signup/reset's 8-byte minimum is a different operation and is not imposed on existing login credentials. Server authorization remains authoritative.

Automatic cookie restoration and 401 refresh are **disabled in production**. The `qualifiedRefresh` constructor seam is used only in deterministic qualification fixtures. Production main does not enable it; enabling it requires a separate reviewed decision and real cookie-race evidence. The qualified path joins concurrent/stale-token 401s into one FE01 refresh, retains generation/event identity, retries only a protected GET once, and never replays auth or mutations. Failed refresh clears private authority. Access expiry clears memory; it does not rotate a cookie.

Explicit login/logout require native Web Locks plus BroadcastChannel. Every dispatched cookie writer first notifies peers to revoke memory access, then holds the common lock through its bounded request. A peer notification never grants access. An unknown dispatched outcome latches the live cohort unavailable for login/refresh, announces uncertainty to peers/newcomers, and permits explicit server logout only. A 204 can acknowledge that explicit logout but does not reopen/qualify an uncertain cohort. The protocol persists no credential, identity, URL or historical fence; closed-tab/new-cohort and production-cookie behavior remain unqualified, which is why automatic flows stay off. Unsupported/denied coordination disables email sign-in honestly.

Logout clears local access before lock/network work. Only the expected 204/no-content response produces confirmed server sign-out. Failure or interruption is shown as local access cleared with server sign-out unconfirmed; retry is explicit. Provider capabilities are decoded exactly, but GOOGLE/APPLE/X remain disabled regardless of advertised enablement until a reviewed OAuth handoff revision exists.

## Verification

Commands ran with retained Node `24.22.2` and `corepack pnpm@11.7.0`, in this worker's checkout, on 2026-11-03. All final commands below exited **0**.

| Command | Result |
| --- | --- |
| `corepack pnpm@11.7.0 --dir frontend install --frozen-lockfile` | Retained installation; no lock/manifests changed. |
| `corepack pnpm@11.7.0 --dir frontend test` | **354/354**, 12 files; includes **78 FE09** cases. |
| `corepack pnpm@11.7.0 --dir frontend test src/features/auth/__tests__/sign-in.test.tsx` | **8/8**, including advertised-enabled frozen OAuth and mounted-world 403/401 regression cases. |
| `corepack pnpm@11.7.0 --dir frontend build` | Strict TypeScript and production build pass. Main chunk approximately 538 kB / 143 kB gzip; Vite emits its >500 kB advisory. No build/config threshold was changed. |
| `corepack pnpm@11.7.0 --dir frontend lint` | No errors or warnings. |
| `node src/features/auth/__tests__/browser-smoke.mjs` from `frontend/` | Installed Chrome **156.0.8078.80**, owned ephemeral servers/browser contexts, 360×800, 390×844, 430×932, 1280×800; focus, enabled-text contrast, 44px controls, pending/error/throttling, login return, immediate world release, unconfirmed logout/retry, reduced-height viewport and no overflow pass. |
| Same Chrome command, shared-cookie two-tab scenario | Native Web Locks/BroadcastChannel and HttpOnly fixture cookie; peak cookie writers **1**, automatic refreshes **0**; peer world access revoked. Production without event performs **0 API requests**; no fixture global/identity/credential in output. |
| `python3 backend/scripts/docs/generate-mcr-core.py --check` | Canonical browser/backend artifacts reproduce from the single assembler. |
| `node scripts/mcr/check-ui-origin.mjs --root <worker-root>` | C08 PASS, 84 files, no origin/import violations; static evidence only. |
| `python3 scripts/mcr/check-plan.py` | Planning/provenance PASS, original archives/checksums preserved; not application acceptance. |
| `python3 scripts/mcr/check-boundaries.py --base 0e2874f --lane frontend --allow-doc docs/mcr/frontend/session-sign-in.md --allow-composition frontend/src/app/router.tsx --allow-composition frontend/src/main.tsx` | PASS after excluding the separate integrator prerequisite; ports/PublicBoundary are ordinary source paths under this guard, manually checked against explicit delegation. |
| `git diff --check` | PASS. |

Initial failures were resolved before the final runs: missing `uri` format support in the narrow decoder; fixture capability expectations corrected to the published disabled state; the FE07 destination link restored; screenshot-discovered contrast corrected to existing cream/ink tokens. One browser rerun was interrupted by an agent source edit/HMR; it was discarded and rerun with stable source. An overbroad boundary command listing ordinary ports/PublicBoundary as central composition was rejected; the corrected invocation above passed.

Local evidence: `/tmp/mcr-fe09-evidence/{unit-tests.txt,build.txt,lint.txt,browser-smoke.txt,browser-results.json}` and `sign-in-*`, `failed-sign-in-*`, `unconfirmed-logout-*`, `production-unavailable.png`. Screenshots at 390 and 1280 were manually inspected, including the corrected foreground contrast. These temporary artifacts are not a durable release record; the test sources are committed and reproducible.

## Limits and remaining gates

The native-browser test uses synthetic same-origin **HTTP** endpoints and a test HttpOnly cookie. It does not qualify Secure production cookies/domain/SameSite deployment, real BE03 sessions/revocation/cookie races, external OAuth, two independent real accounts, actual Phaser/socket, atlas/resource bytes or IN01/QA02. The world counters are mock lifecycle leases, not an actual game/connection. The validator operates on FE01's already-parsed JSON; raw duplicate-member spellings and integer lexical representations are not recovered after JSON.parse. Resource checks validate metadata/hash relationships and geometry, not fetched texture/tilemap bytes, art approval or physical venue truth. A 200, build, screenshot or unit fixture is not end-to-end authorization evidence.

The decoder is deliberately bounded (200,000 validation visits / depth 40) and limited to this frozen reachable vocabulary. Future contract assertion/format changes require explicit decoder compatibility review; unsupported input fails closed. Contract-facing scalar checks use retained Zod formats. Full contract exposure adds the recorded bundle advisory; no unrelated code-splitting/config changes were made.

No backend process, database, volume, build target, server or other worker checkout was touched. No push or task-ledger acceptance update was performed. Main review/integration and any concrete review fixes remain part of the active assignment.

## Bounded review R1

The integrator found that the original generic protected-request catch revoked identity for both 401 and 403. Frozen C03 bootstrap 403 explicitly permits `PERMISSION_DENIED` and `CONSENT_REQUIRED`; neither mandates identity clearing. Origin/intent 403 likewise does not mean bearer identity was revoked. The focused follow-up removes generic 403 invalidation: the original safe transport denial reaches the feature without refresh/replay or private cleanup, and bootstrap maps it to `ShellLoadError('denied')` while its scope remains current. Actual 401 still clears canonical identity/private state immediately; malformed/context-mismatched bootstrap/auth identity checks remain fail closed. A failed explicitly qualified renewal remains a failed-renewal path, not an inference from a feature permission denial.

Seven new regressions prove both frozen denial reasons preserve generation, token, signal, private query data and registered drafts; bootstrap returns denied for each; real React shell composition keeps its mounted mock world on a feature 403 but releases it and runs cleanup on 401. Self-review also tightened the request boundary from own-property-only checking to rejecting inherited `refresh` options before transport, with a no-dispatch regression. This prevents bypassing the cookie coordinator through an inherited option. All changes stay within the original model/test/document ownership. Final counts above include these review regressions; real backend and production rotation limitations are unchanged.
