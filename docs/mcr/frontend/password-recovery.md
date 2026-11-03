# FE11 password recovery and callback presentation

Status: **MOCK_READY**. This bounded increment implements FE11/A05/A06 against the frozen C03 contract and integrated FE09 canonical session. Real Auth/mail delivery, production reset-link configuration, provider/cookie handoff, physical devices and release acceptance remain **NOT_RUN**. It does not enable OAuth or accept the whole project.

## Revision, provenance and scope

- Assigned worktree/branch: `mcr-worktrees/fe-recovery`, `codex/mcr-fe-FE11`; initial clean base `b39d5e54e835485fa114794d2c9330dbb78abcaa`.
- Integrator prerequisite: `fa2e5404257f12f6b83c1439ed28be602a186305`, cherry-picked independently as `99dbe78f68b946af85c5f95dd6c15036405ec2b4`. It changes only `frontend/index.html`, adding an early `no-referrer` meta before every URL-bearing asset. The worker reported the need and did not author that shared composition change.
- Implementation: `c7c70d66f22e7611aa064ff5c42f958a86017ea1`, directly following the prerequisite; parent copy correction: `1bb71ccc424042ade3225e240afcfe4b32d856f0`. The documentation commit containing this file follows those serially. Only the integrator publishes master. No worker push occurred.
- Source foundation: port-rust `704f42149d5e8e35e1d578676d8ae2dac4f0f9c2`, imported by `909e2bf4e1c5bdf752954c94bf71a3b64871732d`, as recorded in `docs/mcr/source-map.md` and the backend import manifest. Current source was read at the assigned base, including subsequent MCR Gateway adaptations. No source code or old rendering was copied into this increment; all UI and recovery orchestration are newly authored.
- Read specification FE11/A05/A06, root/frontend agreements, assessment/execution plan, browser-contract, auth-contract-review and session-sign-in. Specification scope remains authoritative. `docs/source/`, generated contracts and retained dependency resolutions are unchanged.

Current evidence hashes (SHA-256; backend paths are read-only evidence, never frontend implementation imports):

| Input | SHA-256 |
| --- | --- |
| `contracts/generated/mcr-core.openapi.json` | `c8de71d5f3dfc125fd3a5206bc404a76df39367a45fa6c3f67ed6323de91c15d` |
| `contracts/core/v1/operations.json` | `2f5b972dbccea09340e1519a1a2c522e3c42f63690fa26b9e58d721e9927ba62` |
| `backend/crates/auth-application/src/use_cases/password_reset.rs` | `8fc64170664dcd8d94832b2cf0a78b8e0b862c1d6092f6d61ea693c034695e11` |
| `backend/services/auth-service/src/adapters/out/security/password_reset.rs` | `dacdc9d322e6fd11d52aed30b4a1d8577702c058ffe4d31f58b2039f92b3065d` |
| `backend/services/auth-service/src/adapters/out/mail/reset_mailer.rs` | `a1be9486325189e9b6396ba937049e307fc7e569fd8ea98c260ae720f6f40b79` |
| `backend/services/auth-service/src/adapters/out/persistence/auth_transactions.rs` | `d00d6b97b0ead1942060de3e3e75f95be04d2624d290cec1c7bfa2e9215d72d2` |
| `backend/services/api-gateway/src/adapters/in/http/auth_routes.rs` | `a0c4939deb09908dd5093578396fc21775992f8aeaf449b4dfe9654a313b5e4d` |
| `backend/services/api-gateway/src/adapters/in/http/oauth_routes.rs` | `81c3fcd65f521e62374c5042559f3fbc5d3b0f7637c8aeca1bc9c31e7e5dc7e3` |
| `backend/crates/gateway-application/src/dto/auth_dto.rs` | `cd1d78ee98cc32ad95a5a5ab522ee2f82791663c0382c62c80367b0afb742ae3` |
| Existing `frontend/src/shared/api/http.ts` | `e658f8da8e97c5e6801762c5fc552732c2e84a058d4deebdd7b08a1d6b2b57bb` |

## Behavior

Request and confirmation use the existing canonical controller, SessionPort, QueryClient/privacy owner and FE01 HTTP client. They call only POST `/api/auth/password/reset/request` with `{email}` and `/api/auth/password/reset/confirm` with `{token,newPassword}`. No second session, HTTP client, protected-request bypass, credential cache or discovery endpoint is created. These operations are anonymous and carry no event input; explicit recovery therefore does not invent an event prerequisite. Entering either public view performs no automatic recovery command.

`PasswordResetResponse.status` is a string, not an enum. Valid HTTP 200 plus the frozen response shape acknowledges the operation; its status text is discarded, never reflected or interpreted as delivery proof. Request feedback is neutral about account existence and delivery. The actual source returns `SENT` even for unknown email and queues delivery asynchronously; confirmation returns `RESET`. The browser does not turn those implementation strings or the `ACCEPTED` example into a closed schema.

Reset passwords use **8–128 UTF-8 bytes**, without trimming or normalization; malformed Unicode is rejected. Existing login remains **1–128 bytes**. Explicit token entry uses a masked field with autocomplete disabled, source-compatible base64url characters and a local 8192-character resource cap. This does not amend the wire token schema. Forms clear tokens/passwords on submit, unmount and privacy/session invalidation, including invalid submissions. No credential is stored, logged, cached, put into a return URL, or sent across tabs. The active request retains only the transient input needed for its operation.

The inspected source builds `AUTH_PASSWORD_RESET_BASE_URL#token=<token>`; its production crypto generates 32 random bytes as 43 unpadded base64url characters. Intake accepts exactly that fragment profile, with no extra/duplicate fields, and immediately replaces the current URL before displaying a form. The incoming token stays in memory, never a prefilled DOM input. Query tokens and malformed/oversized URL material are rejected and scrubbed. A clean subsequent navigation to the same route clears the prior token and pending operation. Explicit token entry remains available independently of a configured mail link. Actual HTTPS BASE_URL, external mail adapter/content and delivered-link behavior still require deployment qualification.

One recovery command may run at a time. Duplicate submits and competing local login are guarded synchronously. Each command captures the canonical generation, uses transport cancellation/deadline and a privacy cleanup, and checks scope before final success. Logout/account replacement/peer invalidation/disposal/route exit suppress stale completion, even when an injected fetch ignores abort. Cancellation after dispatch does not claim the server rolled back. Known errors are rendered through fixed copy; raw server messages, token text and exception details are not shown. Invalid/expired/used token feedback offers an explicit new request. Network/timeout/5xx/malformed success remains uncertain, never automatically replayed by application code.

The source reset transaction consumes a one-time token and revokes the owner's refresh tokens. Confirmation therefore clears local canonical access/private state before dispatch and uses the existing cookie-writer Web Lock, credential-free peer revocation and uncertainty latch. The exact authorized cookie-policy extension is the typed `recovery` kind plus an optional `AbortSignal`: a route exit can cancel a queued native lock before peer notification or HTTP dispatch. Existing login/logout/refresh calls retain their behavior. No cookie or cross-tab identity/credential is invented, and production automatic refresh remains disabled.

OAuth callback presentation is denial, invalid state or unavailable only. The source endpoint returns Auth JSON and has no reviewed SPA redirect/return-target handoff. This view does not parse/import credentials, replay a code, validate server state locally, authenticate from a URL, revoke an existing valid identity, or enable any provider. It removes query/fragment/arbitrary location state, renders fixed copy, and offers only the existing sanitized participant destination or a sign-in link. Duplicate state/return targets, public/auth/admin redirect chains, traversal, escaped separators, external URLs and secret-bearing queries fall back safely. Fresh clean callback navigation resets earlier presentation.

The early integrator meta protects onward subresource referrers before React loads. React replacement protects later URL/DOM behavior. A manually supplied callback query URL still reaches the initial HTTP server request and may already exist in incoming access logs; no meta or React effect can erase that history. Source-supported reset fragments are not sent in that document request.

## Paths and assignment review

| Changed paths | Responsibility |
| --- | --- |
| `frontend/src/features/auth/model/recovery-contract.ts`, `recovery.ts`, `callback.ts` | Frozen-artifact-backed reset validation/commands and safe URL presentation |
| `frontend/src/features/auth/ui/ResetPassword.tsx`, `Callback.tsx`, `recovery.css` | Newly authored accessible forms/status/focus and callback return actions |
| `frontend/src/features/auth/model/session.ts` | Exact exception: canonical recovery methods, competing-operation guard and lifecycle disposal |
| `frontend/src/features/auth/model/cookie-policy.ts` | Exact exception: typed recovery coordination and optional waiting cancellation signal |
| `frontend/src/app/PublicBoundary.tsx` | Exact exception: imports/dispatches only the two existing routes |
| `frontend/src/app/__tests__/recovery.test.tsx` | 22 DOM/application-route regressions |
| `frontend/src/features/auth/__tests__/recovery.test.ts`, `callback.test.ts` | 46 command/lifecycle and 33 URL/redirect tests |
| `frontend/src/features/auth/__tests__/recovery-browser-smoke.mjs`, `fixtures/recovery-preview.html`, `fixtures/recovery-preview.tsx` | Explicit synthetic native-browser qualification, excluded from production graph |
| `docs/mcr/frontend/password-recovery.md` | This evidence/handoff record |

The separate prerequisite alone changes `frontend/index.html`. No other existing tests, root router, manifests/locks, generated inputs, shared HTTP/query utility, backend, world/runtime or artwork changed. Runtime imports stay within existing public contracts/shared browser facilities and canonical auth composition. No backend implementation is imported. No decorative left accent, colored border rail, pseudo left bar or generic accented-card decoration is used; normal symmetric controls and keyboard outlines remain. No art is needed.

## Verification on 2026-11-03

Retained Node `24.22.2`, pnpm `11.7.0`, Chrome `156.0.8078.80`; all final commands exit **0**. Frontend commands ran in the assigned checkout with the retained local Node distribution. No dependency update occurred.

| Command | Result |
| --- | --- |
| `corepack pnpm@11.7.0 --dir frontend install --frozen-lockfile` | 199 packages reused; retained lock unchanged |
| `corepack pnpm@11.7.0 --dir frontend test` | **511/511**, 17 files, including 101 new FE11 cases |
| `corepack pnpm@11.7.0 --dir frontend build` | Strict TypeScript and production build PASS |
| `corepack pnpm@11.7.0 --dir frontend lint` | No warnings/errors |
| `node src/features/auth/__tests__/recovery-browser-smoke.mjs` from `frontend/` | Installed Chrome PASS at 360×800, 390×844, 430×932, 1280×800 and reduced 420px height |
| `MCR_FE09_EVIDENCE=/tmp/mcr-fe11-evidence/sign-in-regression node src/features/auth/__tests__/browser-smoke.mjs` from `frontend/` | Existing sign-in/native cookie regression PASS at four widths, peak cookie writers 1, automatic refreshes 0 |
| `python3 backend/scripts/docs/generate-mcr-core.py --check` | Public/backend contract outputs reproduce, no drift |
| `node scripts/mcr/check-ui-origin.mjs --root <assigned-worktree>` | C08 PASS, 108 files / 101 content files, zero violations; source `5eecdcaffd1a9d505b9ab2a974f8d5623217e90a`, inventory hash `9d1202211cd136948a7afeb5588238ba9de13613980dab42616011f47762a079` |
| `python3 scripts/mcr/check-plan.py` | Planning/provenance PASS; original archive checksums preserved, not application acceptance |
| `python3 scripts/mcr/check-boundaries.py --base 99dbe78 --lane frontend --allow-doc docs/mcr/frontend/password-recovery.md` | FE11 paths PASS after separately accounting for the integrator prerequisite; manual assignment/import review also performed |
| `git diff --check` | PASS |

Native coverage includes keyboard/heading/status focus, 44px targets, enabled-text contrast, reduced-height scrolling/no horizontal overflow; neutral unknown-email and throttling; duplicate prevention; fragment/explicit token handling; invalid-token/uncertain outcomes; cross-tab revocation; queued-lock cancellation with zero dispatch; route-exit late-response suppression; no automatic refresh; no secrets in rendered HTML, active URL/history state, storage or JS-visible cookies. Production cold callback actually fetched root and lazy JS/CSS with empty Referer; reset fragments were scrubbed, no fixture globals/markers or world/Phaser/API requests were admitted on public entry. Provider sign-in remains disabled. These are synthetic HTTP/native-browser proofs, not real Auth/mail/provider acceptance.

Local evidence: `/tmp/mcr-fe11-evidence/{unit-tests.txt,build.txt,lint.txt,browser-smoke.txt,browser-results.json,c08.json,plan.json,sign-in-regression.txt}`, the `sign-in-regression/` report/PNGs, and `request-*`, `confirmed-*`, `denied-*`, `uncertain-*`, `production-reset-390.png`. Screenshots at 360/390 and 1280 were manually inspected, including production fragment intake and the corrected uncertainty copy. These temporary files are local qualification evidence; reproducible test sources are committed. Root output retains the existing >500kB advisory: approximately 541.32kB / 143.74kB gzip; public lazy JS approximately 23.55kB / 7.01kB gzip, CSS 7.32kB / 1.87kB gzip. No threshold/configuration changed.

## Review correction and transmission limit

The parent's early review accepted the generation/cancellation/fragment design and requested a concrete copy correction: “Nothing will be retried automatically” was too broad given observed browser retransmission. The follow-up uses outcome-focused reset uncertainty and separate neutral request/delivery uncertainty, with a dedicated DOM regression and fresh affected Chrome/build/lint/full-suite evidence. The optional cookie-policy signal is explicitly within the parent's granted minimal coordination exception.

An initial native fixture that dropped TCP before any response headers observed repeated server arrivals. A separate instrumented probe now records **one application fetch and five server arrivals** in the observed Chrome run, ending uncertain. The ordinary 503 scenario has one application fetch and one server arrival. Shared FE01/Ky retry remains zero; no query/mutation retry layer exists. This proves the application does not issue a replay; it cannot promise exactly one physical transmission by Chromium after an opaque connection failure. Source one-time-token enforcement remains authoritative for reset consumption. No shared transport/backend change or real-server duplicate-consumption acceptance is claimed.

Real production HTTPS/Secure cookie/domain policy, reset-mail adapter and delivery, deployed BASE_URL, real expired/replayed token transaction, production auth races, OAuth backend-to-SPA/cookie handoff, actual devices and IN01/release gates: **NOT_RUN**. At handoff, the persistent goal remains active through the integrator's bounded review and concrete fixes; whole-project acceptance remains separate.
