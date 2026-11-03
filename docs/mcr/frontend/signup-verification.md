# FE10 signup and email verification

Status: **MOCK_READY** for the bounded FE10/A03/A04 mapped form increment. Production signup remains unavailable. This is not acceptance of the whole FE10 task, real multiuser flow, project or release. Real Auth, account creation, mail delivery, production eligibility/consent persistence and physical devices are **NOT_RUN**.

## Scope and source review

Assigned base: `e725ae4e1c511b299619de8c0f5e343e3daf640e`, branch `codex/mcr-fe-FE10`, isolated checkout `/Users/lhs/Desktop/mcr-worktrees/fe-signup`. The implementation plan, project assessment and execution plan were read before implementation. A03 maps `/signup`; A04 maps `/verify-email`. Plan requirements for eligibility, consent, expired challenges and resend cooldown remain in scope; missing contracts are recorded below rather than replaced with browser assumptions.

No legacy rendering or assets were copied. New UI and feature commands use the existing frontend raw UTF-8 password validator, canonical UUID check, session, HTTP and privacy ports. No additional dependencies or lock resolutions changed. No backend implementation import, shared contract modification, second session/client/store, or `docs/source/` change is included.

| Reviewed authority at the base | Revision / behavior |
| --- | --- |
| `contracts/generated/mcr-core.openapi.json` | SHA-256 `c8de71d5f3dfc125fd3a5206bc404a76df39367a45fa6c3f67ed6323de91c15d` |
| `contracts/core/v1/operations.json` | SHA-256 `2f5b972dbccea09340e1519a1a2c522e3c42f63690fa26b9e58d721e9927ba62` |
| `backend/crates/auth-application/src/use_cases/signup.rs` | SHA-256 `f5dd917be183d0b0ce6e5ac801a6eee6b2d2c7223e08c7e12cd46f535c70c08a`; raw 8–128 password bytes; signup pending; verify VERIFIED; resend opaque ACCEPTED |
| `backend/services/auth-service/src/adapters/out/persistence/auth_transactions.rs` | SHA-256 `d00d6b97b0ead1942060de3e3e75f95be04d2624d290cec1c7bfa2e9215d72d2`; verification checks database clock expiry/attempt limit and deletes consumed challenges atomically |
| `backend/services/auth-service/src/adapters/out/persistence/onboarding_transactions.rs` | SHA-256 `cf4daf601c9113696258f436eec1a424cced27365aab817e419c881844f69156`; resend creates a one-hour challenge only when eligible/admitted; one-minute/20-per-day quota remains opaque |
| Gateway/Auth verification HTTP routes and `onboarding.rs` | Resend 202 `{status:"ACCEPTED"}`; admission, existence and email delivery are not returned |
| FE08 `features/public/model/{documents.ts,document-provider.tsx}` | Default unavailable; presentation port explicitly does not authorize publication, policy or retention |

The backend observations above are static source evidence. Browser rejection fixtures do not prove a fresh backend expiry transaction.

## Behavior and limitations

- `PublicBoundary.tsx` adds only imports and dispatch for the existing two mapped routes. No world, protected bootstrap or private API is entered by these views.
- `session.ts` exposes one feature command owner using its existing HTTP/session/privacy ports. Provider availability follows canonical capability discovery independently of native cookie-writer qualification. Verification exposes an explicit “Check email availability” action through that same single-flight capability request, including null/malformed concert event configurations; FE09 login/emailEnabled and world readiness still require a valid event. No JWT, user ID or cookie authority is installed by any enrollment response. Signup/verify use HTTP200 `SignupResponse`; status remains a string contract. Only the source's matching known operation outcome gives completion; a future/unexpected status gives uncertainty and is never rendered. Resend requires exactly HTTP202 `{status:"ACCEPTED"}`.
- Signup sends only `{email,password}`; verify sends only `{email,code}`; resend sends only `{email}`. There is no check-email discovery request, credentials in URLs/storage/logging, automatic application replay, idempotency claim, or delivery confirmation.
- Passwords are validated as well-formed, raw 8–128 UTF-8 bytes with no trimming. Codes require six ASCII digits. Password/code values are masked while editing and cleared immediately on submission, privacy invalidation, route replacement or unmount. Email and synthetic choices also clear on route/privacy changes. No enrollment input is retained in the command owner's state, query cache or mutation cache.
- Synchronous operation reservations prevent duplicate submissions and competition with login and recovery; logout invalidates pending enrollment immediately. Abort plus session-generation fences reject late completions even when a transport ignores cancellation. The existing HTTP transport supplies a bounded deadline (production default 15 seconds; short deterministic fixture deadline).
- Dirty query/hash/history state is discarded with replacement before rendering forms; no incoming email/password/code is imported. Initial requests containing a user-provided query necessarily arrive before React can scrub the URL. Existing early no-referrer policy protects later requests, not that original inbound request.
- Verification failure can be invalid, expired or consumed; the view offers a new request without guessing which account/challenge exists. Resend acceptance and uncertainty both remain neutral about existence and delivery. Signup conflicts show a neutral sign-in/recovery path and never reflect raw server text.
- Only a bounded server `Retry-After` hint on 429/503 creates a wait/countdown. The command owner also fences other enrollment commands during that hint, including route changes. The countdown is a UI hint; no request runs when it expires. Polling stops at the deadline and is cleaned up on route exit; deterministic timer coverage verifies zero remaining interval timers after expiry or exit. Accepted resend and errors without metadata never fabricate a one-minute cooldown, challenge deadline or remaining attempts. Server validation remains authoritative.

## Eligibility, consent and publication gap

The plan requires eligibility and consent. The frozen signup request has only email/password: there is no approved eligibility policy, legal document publication authority, revision acknowledgement, consent persistence or atomic signup/consent admission contract. The plan's 14–100 claim boundary is not adopted as a legal signup age. FE08's read-only document presentation port cannot grant missing consent authority; the existing Terms/Privacy routes remain linked without inventing approved text or operator contact. Central policy/publication/persistence contracts are needed before a production adapter can be qualified.

The parent accepted this boundary during the source review. Production `syntheticEnrollment` is absent and signup stays unavailable even if email capability is enabled and a caller submits true acknowledgements. The test-only session option enables explicitly labelled synthetic eligibility/consent choices; these are fixture inputs, not recorded consent, and are never added to the wire. The fixture is under `__tests__` and is not reachable through production composition. Existing-account verify/resend do not require synthetic signup qualification. Default production composition has no configured event/capability: signup stays disabled with zero API requests. Verification initially makes no automatic discovery request, but its explicit availability action performs the legitimate public capability GET without requiring a concert event. On an enabled response, existing-account verify/resend work; login/world readiness stay unavailable. Discovery errors can be retried explicitly.

Whole FE10 acceptance therefore still needs approved policy/legal publication, persisted and enforced eligibility/consent, live Auth/mail verification including expiry/quota behavior, real vertical signup→verify→sign-in, and integration/release evidence. No scope cut is claimed.

## Files and ownership

Application paths: `frontend/src/features/auth/ui/{SignUp.tsx,VerifyEmail.tsx,enrollment.css}`, `frontend/src/features/auth/model/{enrollment.ts,enrollment-contract.ts}`. Exact composition exceptions: `frontend/src/app/PublicBoundary.tsx` and `frontend/src/features/auth/model/session.ts`.

Verification paths: `frontend/src/features/auth/__tests__/{enrollment.test.ts,enrollment-browser-smoke.mjs,fixtures/enrollment-preview.html,fixtures/enrollment-preview.tsx}` and `frontend/src/app/__tests__/enrollment.test.tsx`. This document is the only lane documentation change. All 13 assigned paths were compared manually; imports are frontend/public-contract only. Shared HTTP, cookie policy, manifests, locks, routers, contract artifacts and backend files remain untouched.

## Verification

Node24.22.2 and pnpm11.7.0; installed Chrome156.0.8078.80; commands run from the assigned checkout, browser scripts from `frontend/`. Every command listed below completed with exit code **0**.

| Command | Evidence |
| --- | --- |
| `corepack pnpm@11.7.0 --dir frontend install --frozen-lockfile --offline` | 199 packages reused; no lock change |
| `corepack pnpm@11.7.0 --dir frontend test` | **737/737**, 20 files; **68** new enrollment controller/route cases |
| `corepack pnpm@11.7.0 --dir frontend build` | Strict TypeScript and production build PASS; retained >500kB advisory, root ~544.82kB /144.67kB gzip, public lazy JS ~31.80kB /8.69kB gzip |
| `corepack pnpm@11.7.0 --dir frontend lint` | PASS |
| `node src/features/auth/__tests__/enrollment-browser-smoke.mjs` | Four widths 360/390/430/1280 and reduced 420px height; exact payloads, pending/double-submit, neutral duplicate, expired/consumed code recovery, server-only cooldown, opaque resend, uncertainty, privacy/route cancellation and late response fences, event-independent explicit discovery (null/malformed event and production public route), keyboard/heading/feedback focus, 44px targets, enabled text contrast, overflow, URL/DOM/storage hygiene and no auth installation |
| Existing `browser-smoke.mjs` with `MCR_FE09_EVIDENCE=/tmp/mcr-fe10-evidence/sign-in-regression` | FE09 native-browser regression PASS; four widths; peak cookie writers1; automatic refreshes0 |
| Existing `recovery-browser-smoke.mjs` with `MCR_FE11_EVIDENCE=/tmp/mcr-fe10-evidence/recovery-regression` | FE11 native-browser regression PASS; four widths and native lock/peer/production checks retained |
| `python3 backend/scripts/docs/generate-mcr-core.py --check` | C03 outputs reproduce |
| `python3 scripts/mcr/validate-core.py` | CONTRACT_READY;33 fixtures; producer runtime NOT_RUN |
| `node scripts/mcr/check-ui-origin.mjs --root /Users/lhs/Desktop/mcr-worktrees/fe-signup` | C08 PASS; source revision `5eecdcaffd1a9d505b9ab2a974f8d5623217e90a`; inventory `9d1202211cd136948a7afeb5588238ba9de13613980dab42616011f47762a079`; zero violations |
| `python3 scripts/mcr/check-plan.py` | Planning/provenance PASS; original source checksums preserved; not an application status update |
| `python3 scripts/mcr/check-boundaries.py --base e725ae4 --lane frontend --allow-doc docs/mcr/frontend/signup-verification.md` and `git diff --check` | PASS; exact assigned path and import review also performed |

Production preview checks confirm unavailable signup and initially unavailable verification with zero API requests before an explicit action, no world/fixture requests or canvas/world host, and no test markers/globals in the production bundle. The public production verify route then qualifies explicit capability discovery plus exact anonymous resend/verify against controlled HTTP responses, without any event or fixture global. Additional native fixture cases cover null and malformed event IDs without synthetic signup qualification. Browser HTTP is an owned deterministic synthetic server; 420px height approximates reduced viewport space and is not physical keyboard/device proof. One application dispatch/retry0 cannot certify one physical TCP arrival; no stronger transmission claim is made.

Local evidence lives in `/tmp/mcr-fe10-evidence/`: `unit-tests.txt`, `build.txt`, `lint.txt`, `browser-smoke.txt`, `report.json`, `c08.json`, `plan.json`, PNGs and the two native regression directories. The agent inspected 390px synthetic and production signup, 360px verified and 1280px uncertain views, and also inspected the corrected 390px production availability action and 1280px event-independent production verification result. Screenshots use ordinary symmetric borders/focus outlines, spacing and typography, without decorative left accents. Reproducible test sources are committed; temporary evidence is local.

## Delivery ancestry

The isolated implementation starts from the exact base above. Implementation/test candidate commit: `6a8e8270f850c9b28a61597f5ff2021e682928e2`. Its parent is the assigned base. The following documentation commit is reported in the handoff; only the parent integrator may integrate or push. Goal remains ACTIVE until the parent's bounded review acceptance and any concrete corrections are complete.

## Parent review corrections

R1 identified that an expired Retry-After interval continued polling until unmount. The interval now clears itself at the deadline, with deterministic clock coverage proving no interval timers after expiry, no replay, and cleanup on exit from enrollment.

R2 identified that initial enrollment availability inherited the unrelated concert-event discovery gate. Explicit canonical enrollment capability discovery now works with null/malformed event IDs and is shared as one flight with FE09 capability discovery. Only signup's synthetic policy fixture can qualify its form; production signup stays blocked. Existing-account verification/resend are anonymous and event-independent after discovery. FE09 login still requires a valid configured event, `emailEnabled` stays false without it, and no world authority is created. Unit/route tests and installed Chrome cover both no-event variants and production public verification with an explicit availability action. The cold-entry zero-API statement is deliberately limited to signup and verification before that action.

R3 fences a capability flight disposed synchronously before its reserved microtask dispatch, while preserving reservation before loading publication. Regressions cover start(), explicit discovery and synchronous loading-listener disposal: zero fetch calls and no late state/observer publication, preserving discovery ownership during controller teardown.
