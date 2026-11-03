# C03 browser-consumer review of pinned Gateway authentication

Status: **IN_PROGRESS — review delivered for C03 decisions; not a contract freeze
or runtime acceptance**. This is a read-only review of source and the parent's
generated contract. Only this document is changed. FE01/FE09 implementation,
browser requests, provider flows, databases and hardware are **NOT_RUN**.

## Baselines and scope

- Worker branch: `codex/mcr-fe-foundation`; review base
  `a6cb4f4adb8b24c1ca3a56ecead900790765bf02`.
- Gateway source: `julymeltdown/port-rust` at
  `704f42149d5e8e35e1d578676d8ae2dac4f0f9c2`, clean read-only checkout
  `/Users/lhs/Desktop/mcr-sources/port-rust`. Paths below are relative to its
  `boilerplate-grpc-rust/` workspace.
- Bigyeting comparison: `5eecdcaffd1a9d505b9ab2a974f8d5623217e90a`, principally
  `frontend/src/lib/api/real/canonical-auth-client.ts` and `runtime-client.ts`,
  already recorded in the C01 frontend inventory. No source code is copied.
- Supplied final OpenAPI: `/Users/lhs/Desktop/mcr-build/upstream-openapi.json`,
  title `Port Rust API Gateway`, version `0.5.0`, SHA-256
  `ba1319c823948458f8fa66765e2f1468e109504c8dc30734ff2731a9e10970ea`.
  Reviewed the auth operation slices, referenced DTOs, security schemes, headers,
  and source-backed semantic notes. This worker did not regenerate the bundle.
- Parent reports integrated audit/scaffold commits `d50427c` / `6e1754a` and
  frozen install, strict build and Oxlint PASS. Those checks do not establish
  authentication behavior, and this worker has not rerun them.

The source Gateway exposes `/auth/*` directly. The MCR specification selects
same-origin ingress under `/api/*`; C03 must record the external path and any edge
rewrite per operation. Do not assume `/api/auth/*` already exists in the source,
or prepend `/api` indiscriminately to inherited routes that already use `/api`.
The bundle's localhost server and production placeholder are not MCR deployment
addresses.

## Exact operation map

These are the bundled `operationId` values and the actual source route families.
JSON field names are case-sensitive; all listed request fields are required.
`None` means no documented request body, not `{refreshToken: ...}`. Standard
success JSON is `application/json` unless stated otherwise.

| operationId | Gateway method/path | Request | Success and authentication |
|---|---|---|---|
| `signupWithEmail` | POST `/auth/email/signup` | JSON `{email, password}` | 200 `SignupResponse`; anonymous. Creates pending account, not a signed-in session. |
| `verifyEmail` | POST `/auth/email/verify` | JSON `{email, code}` | 200 `SignupResponse`; anonymous. Verification is not login. |
| `checkEmailAvailability` | POST `/auth/email/check` | JSON `{email}` | 200 `EmailAvailabilityResponse`; anonymous; source explicitly exposes availability. |
| `resendVerificationEmail` | POST `/auth/email/resend` | JSON `{email}` only | 202 `{status:"ACCEPTED"}`; anonymous, no browser service token/Bearer/cookie required. |
| `login` | POST `/auth/login` | JSON `{email, password}` | 200 `AuthResponse`, refresh `Set-Cookie`; anonymous request. |
| `refreshAccessToken` | POST `/auth/refresh` | None | 200 `AuthResponse`, rotated refresh `Set-Cookie`; `RefreshCookie`. |
| `logout` | POST `/auth/logout` | None | 204, clears refresh cookie; bundle declares `RefreshCookie`. Runtime also succeeds without a cookie. |
| `requestPasswordReset` | POST `/auth/password/reset/request` | JSON `{email}` | 200 `PasswordResetResponse`; anonymous, neutral public result. |
| `confirmPasswordReset` | POST `/auth/password/reset/confirm` | JSON `{token, newPassword}` | 200 `PasswordResetResponse`; anonymous; one-time reset token, no login tokens. |
| `authorizeOAuth` | GET `/auth/oauth/{provider}/authorize` | No body | **302**, `Location` and state `Set-Cookie`; anonymous; creates one-time state. |
| `completeOAuthCallback` | GET `/auth/oauth/{provider}/callback` | Query `code`, `state` | 200 `AuthResponse`, refresh cookie set/state cookie cleared; `OAuthStateCookie`. |
| `receiveAppleOAuthFormPost` | POST `/auth/oauth/{provider}/callback` | URL-encoded provider form, below | 200 **intermediate HTML**, not authentication; no cookie required on this first cross-site POST. Apple only. |
| `completeAppleOAuthFormPost` | POST `/auth/oauth/{provider}/callback/complete` | URL-encoded `{code, state}` only | 200 `AuthResponse`, refresh cookie set/Apple state cleared; original `OAuthStateCookie`. Apple only. |
| `linkOAuthIdentity` | POST `/auth/oauth/{provider}/link` | JSON `{code, state}` | 200 `LinkedProvidersResponse`; **Bearer AND OAuth state cookie**, not alternatives. |
| `deleteAccount` | DELETE `/auth/account` | None | 202 `DeleteAccountResponse`, refresh cookie cleared; Bearer. Durable deletion initiation, not completed erasure. |
| `getAccountDeletion` | GET `/auth/account` | None | 200 `UserDeletionSaga`; Bearer; current caller's saga. |
| `getAccountDeletionStatus` | GET `/auth/account/deletion` | None | 200 same saga; Bearer; route alias, not session introspection. |

The `/shadow/auth/login` operation `shadowLogin` is parity-runtime-only and is
not a browser login alternative. No `/auth/providers`, `/v1/session`, refresh-body
token API, Naver verification API or Instagram login API is present in this
Gateway contract.

## DTO casing and parsing obligations

The Gateway service reexports DTOs through
`services/api-gateway/src/application/dto/mod.rs`; the owning definitions are in
**`crates/gateway-application/src/dto/auth_dto.rs`**, not an auth DTO file beside
the service reexport. They use Serde `rename_all = "camelCase"`.

| DTO | Public JSON shape / constraints in supplied OpenAPI |
|---|---|
| `AuthResponse` | Required `userId` UUID, `accessToken`, **`refreshToken: ""`**, `tokenType`, `expiresIn` integer seconds >=1, `linkedProviders` array of `EMAIL/GOOGLE/APPLE/X`. `expiresIn` is not an absolute expiry timestamp. `tokenType` has example `Bearer`, not an enum constraint. |
| `SignupResponse` | Required `userId` UUID, `status` string. Actual Auth use case returns `PENDING_VERIFICATION` for signup and `VERIFIED` for verify. Neither response supplies an access token. |
| `EmailAvailabilityResponse` | Required `email`, `available`, `status`; bundled status enum `AVAILABLE/TAKEN`. Runtime has an additional `INVALID` branch, below. |
| `EmailSignupRequest` | `email`, `password`; documented password 8–128 characters. Source enforcement is bytes, below. |
| `LoginRequest` | `email`, `password`; documented password length 1–128. Do not trim/change the submitted password. |
| `EmailVerifyRequest` | `email`, `code`; documented six ASCII digits. No challenge ID or completion token. |
| `PasswordResetConfirmRequest` | `token`, **`newPassword`**, documented password 8–128; not `new_password`. |
| `PasswordResetResponse` | Required `status` string; example `ACCEPTED` is not an enum. Actual Auth returns `SENT` on request and `RESET` on confirmation; route-test doubles use other strings. Do not infer delivery from any of these. |
| `LinkedProvidersResponse` | Required `userId`, **`providers`** array of `EMAIL/GOOGLE/APPLE/X`. Do not parse it as `AuthResponse.linkedProviders`; a deserialize alias in Rust does not change the emitted key. |
| `ErrorResponse` | **Top-level** `{code, message, traceId?, timestamp?, details?}`; optional fields may be null. `details` is an open object, not Bigyeting `field_errors`. Bundle marks `traceId` UUID, but runtime can echo a non-UUID correlation string, below. |

Most auth request DTOs do not deny unknown fields, and the bundle does not mark
them `additionalProperties:false`. Do not promise rejection of extra fields
unless C03 deliberately adds it. Resend does deny unknown fields; its schema has
email length 1–254 and a **4096-byte entire JSON body** limit. Its response has
only the enum `ACCEPTED`. Ordinary JSON extractor errors are normalized to JSON
400/415/422; FE01 must also handle non-JSON transport/edge failures.

Optional shared headers are `X-Correlation-Id`, `X-Request-Id` and `X-Client-Id`,
documented strings <=128 characters. They do not authenticate, identify a user,
or make a command replay-safe. The Gateway returns `X-Correlation-Id`; error
codes commonly become `GATEWAY_<HTTP status>`, so a browser cannot assume the
internal `AUTH_*` taxonomy survives the Gateway. Browser code must not supply
infrastructure-owned `X-Forwarded-For` or internal gRPC/service credentials.

## Cookie, CSRF, redirect and provider findings

### Source behavior

1. `login`, refresh and successful OAuth issue `refresh_token`; JSON is explicitly
   passed through `strip_refresh_token`, retaining the required empty
   `refreshToken` field. Browser code must neither treat that as an auth failure
   nor persist/use it as the refresh credential.
2. Refresh cookie attributes are `HttpOnly; Path=/; SameSite=Lax;
   Max-Age=<configured seconds>`. `Secure` and `Domain` are configuration-driven.
   Defaults are Secure=false, empty domain and 1,209,600 seconds. Production
   config requires Secure=true **and nonempty Domain**; MCR must qualify its
   actual settings rather than claim the defaults satisfy production policy.
   Clearing uses matching attributes and Max-Age=0.
3. OAuth state cookies are `oauth_state_google`, `oauth_state_apple`, and
   `oauth_state_x`, with the same HttpOnly/Path/SameSite/Secure/Domain policy and
   **600-second** lifetime. `TWITTER` aliases X and uses `oauth_state_x`. The
   response clears the state cookie after successful callback/link. Auth consumes
   the provider/state transaction before provider exchange, including PKCE/nonce
   checks; do not create or replace this cookie in JavaScript.
4. The generic parser accepts provider spelling case-insensitively. The schema
   lists `GOOGLE/APPLE/X/TWITTER`; use these declared values in generated clients.
   Although the parser also recognizes `EMAIL`, downstream OAuth rejects it.
   Response arrays contain `X`, never `TWITTER`. Naver/Instagram are not supplied
   auth providers.
5. Reviewed refresh/logout handlers require **no CSRF header, Origin or Referer
   validation**. There is no Bigyeting CSRF token endpoint/cookie/header contract.
   Refresh reads the HttpOnly cookie only; logout revokes it when present and
   clears it on success. Source CORS uses explicit allowed origins and credentials;
   it is not a substitute for a server cookie-mutation CSRF/origin policy.
   **C03 must choose and document MCR's protection and negative probes.** Adding
   a frontend `X-CSRF-Token` alone would not enforce it on this server.
6. Same-origin browser fetch can preserve cookies with `credentials:"same-origin"`
   (or explicitly included credentials). Cross-origin requests require included
   credentials and compatible origin/cookie settings; never infer that CORS alone
   guarantees cookies work. Executable CORS exposes correlation/operation/replay
   headers, but not `Retry-After`; a cross-origin consumer needs an explicit
   exposure decision to read that hint. This matters less for selected same-origin
   MCR ingress, which still needs actual browser verification.

### OAuth browser handoff still needs a C03 decision

Authorize is a state-changing **GET returning 302**, intended for explicit browser
navigation. Do not invoke it as a cached/prefetched/retried TanStack query or expect
Bigyeting's `{authorization_url, state_expires_at}` JSON. The source accepts no
`return_target` input and the callback returns JSON; it does not redirect into
MCR `/auth/callback` or restore a world/panel route. Link also needs an explicit
same-session orchestration plan rather than repurposing login to prove linking.

Apple's initial URL-encoded callback permits either `{code,state}` with optional
`id_token,user`, or `{error,state}` with optional provider fields. Extra/duplicate
fields, malformed encoding, code+error, control characters, unsupported providers
and oversized input fail. The total form limit is 32,768 bytes, code <=4096 UTF-8
bytes and state <=512 bytes. Provider profile/token fields are ignored. Successful
initial HTML neither calls Auth nor sets/clears cookies; it submits code/state to
**hardcoded `/auth/oauth/apple/callback/complete`** on the Gateway origin, with a
manual Continue button. Only completion checks exactly one original Apple state
cookie and exchanges the code. This absolute form action must remain reachable
under the chosen MCR ingress rewrite. A 200 HTML page is never signed-in success.

No provider-discovery operation exists. `linkedProviders`/`providers` describe an
account's links, **not operational availability**. Auth config has distinct parity
and production provider clients; production construction requires Google, Apple
and X configuration, while default authorization hostnames include `.test`
fixtures. Source enum membership or configuration declarations do not establish
working live providers. C03 Bootstrap/capabilities must expose truthful supported
and enabled availability, or explicitly define another reviewed configuration
source. FE09 must not invent Naver availability or offer a successful fake login.

## Safe FE01/FE09 retry and cache policy

These are consumer recommendations derived from the inspected source, requiring
C03 fixture/producer-consumer agreement; they are not implemented by this review.

| Operation class | Browser policy |
|---|---|
| Login/signup/verify/reset/link | Explicit user command; disable generic HTTP/query/mutation retries. A timeout or abort may follow a committed side effect. Preserve drafts, surface uncertainty, and offer the operation-specific recovery rather than claim rollback. A 401 on login is invalid credentials, not a reason to recursively refresh. |
| Refresh | One shared in-flight refresh per active shell/session; exclude refresh from the general 401 handler. Update the in-memory token only if its identity generation still matches. Rotation has a grace-window conflict path and later reuse can revoke the family. Source Auth emits RefreshConflict; Gateway projects it to generic 409. Do not blindly loop/replay a stale token after timeout/409; freeze cross-tab coordination and bounded recovery before implementation. |
| Authenticated request after 401 | A refresh success permits only the specifically authorized replay, at most once and only in the same identity generation. Read-only requests can have an agreed bounded retry policy; mutations need their own pre-dispatch/replay guarantees. Never apply the rule to 403, all 409s, OAuth callbacks, or failed login. |
| OAuth authorize/callback | No prefetch, refetch-on-focus, automatic retries or cached results. On uncertain code consumption, restart only through an explicit new authorization action/recovery decision; never resubmit a one-time callback or manufacture state. GET does not imply safe replay here. |
| Email resend | No automatic retry or Idempotency-Key replay. Respect user-visible resend cooldown; 202 is neutral acceptance, not delivery or account existence. Source documents one intent/minute and 20/day including signup, while edge 429 is separate; UI timing cannot prove remaining server quota. |
| Logout | Immediately stop presenting a usable local session and clear private state. Distinguish local cleanup from confirmed server revocation; source downstream failure can prevent cookie-clearing response. Late refresh/login responses cannot resurrect logout. Do not assert every access JWT is immediately invalid on every route. |
| Rate limit / saturation | Honor available `Retry-After`, show a bounded recoverable state, and retain uncertainty. A generic 503/504 or correlation/request ID is not proof that an auth command was not dispatched. Do not attach a global Idempotency-Key: transport explicitly rejects it on unsupported routes, including auth. |

Keep access tokens only in memory, never browser storage, URLs, React Query data
or persisted mutation variables. Keep auth bodies, passwords, codes/state, reset
tokens and secret-bearing responses out of logs, analytics and service-worker
caches. Disable generic persistence/caching for auth; request `cache:"no-store"`
where appropriate. Only resend and Apple form responses have explicit no-store
handling established by this review; C03 must require consistent auth response
cache headers rather than assume all handlers supply them.

Scope private query keys by canonical `userId` plus selected event, and use an
identity generation guard with cancellation/removal. User or event changes,
logout, block/revoke/deletion and bootstrap denial must evict affected private
data and world subscriptions. A successful refresh must not switch silently to a
different UUID or let an old request populate a new user's cache. `userId` is the
Auth UUID; X/Instagram linkage and account context are not cache identity.

## Discrepancies to resolve before freezing

| Finding | Evidence / required C03 treatment |
|---|---|
| Bigyeting signup choreography is incompatible | Old `/v1/auth/email/signup/{start,verify,complete}` uses challenge/completion IDs; source Gateway instead signs up with email+password, verifies email+code, then requires login. No mechanical route rename. |
| Old transport/session assumptions are incompatible | Old `/v1/auth/email/login`, DELETE `/v1/session`, `/v1/auth/providers`, member/admin CSRF caches and X-account context headers have no matching contract here. Use the operation map, empty refresh JSON field and top-level camelCase errors. |
| Missing MCR browser contract pieces | Freeze external auth paths/rewrite, Bootstrap and canonical identity, provider availability, cookie-mutation protection, OAuth return/handoff and safe return-target allowlist. Define safe panel restoration independently of Bigyeting's old route allowlist. No speculative endpoint is asserted by this review. |
| Availability enum omits runtime value | `EmailAvailabilityResponse.status` permits only AVAILABLE/TAKEN, but Auth `check_email` returns 200 with `INVALID` for an invalid string. Rust request DTO accepts a String and format is not enforced by an OpenAPI-aware Gateway validator. Parent must reconcile schema/runtime or exclude that API deliberately, not silently broaden generated types. |
| Password length units differ | OpenAPI length is characters, while Auth signup/reset `validate_password` checks Rust `password.len()` **UTF-8 bytes** 8–128. Freeze an agreed rule and Unicode fixtures. Client validation cannot repair a producer/contract mismatch. |
| Correlation/trace format differs | Bundle marks `ErrorResponse.traceId` UUID; Gateway can echo an accepted arbitrary correlation string, and its own tests assert `corr-123`. Reconcile schema and correlation format before strict generated/Zod parsing. |
| Status examples are not enums | Reset's bundled ACCEPTED example differs from actual SENT/RESET, and route doubles use RESET_REQUESTED/RESET_CONFIRMED. Freeze source-backed semantics; do not encode example/double values as actual enum or delivered-email claims. Signup statuses are strings as well. |
| Logout security is stricter in docs than runtime | Bundle requires RefreshCookie; handler allows missing cookie and still clears it with 204. Decide whether this deliberately supports idempotent local logout, and test both missing and invalid/revoked cookies. |
| `/api` ingress can break Apple completion | Source HTML form posts to absolute `/auth/oauth/apple/callback/complete`; selected ingress cannot expose only `/api/auth/*` without an explicit compatible adaptation. Neither provider redirect nor return-target behavior is solved by generated types. |

## Evidence and immutable source references

The operation map is traced to `scripts/docs/openapi/auth.py` and
`scripts/docs/openapi/verification.py`; public behavior was compared to actual
Gateway router/DTO/cookie code and narrowly selected Auth use cases. No generated
checklist or source test is reported as an executed browser test.

Reviewed files below were byte-compared to the pinned Git revision. These SHA-256
values bind the principal route/DTO/security evidence; other referenced source
files remain bound by the repository commit above.

| Workspace-relative source | SHA-256 |
|---|---|
| `services/api-gateway/src/adapters/in/http/auth_routes.rs` | `5a57ed8c96da5d468196051c98b1f70a0fd29ab298a14193bb23ad1e1a0bfcc8` |
| `services/api-gateway/src/adapters/in/http/refresh_routes.rs` | `fce7cd3da8e27b9c3f12b5b80771e9ac17554dbf821993c29320f3ee46ae7cec` |
| `services/api-gateway/src/adapters/in/http/oauth_routes.rs` | `81c3fcd65f521e62374c5042559f3fbc5d3b0f7637c8aeca1bc9c31e7e5dc7e3` |
| `services/api-gateway/src/adapters/in/http/oauth_routes/form_post.rs` | `52c7b8646f4be9f6ce5dee6c1de211f76edd9ca50a2fc7b7b7db8fb13004393b` |
| `services/api-gateway/src/adapters/in/http/mod.rs` | `64efd260bfd69b6e3fbaa7d55c2c4ee3c497c54f8ba341b7a21519443c93d46f` |
| `services/api-gateway/src/adapters/in/http/server.rs` | `75dffc6261932b9d56ad758555d444e00661eb06e7a3b3456b6d22f329133dc3` |
| `crates/gateway-application/src/dto/auth_dto.rs` | `cd1d78ee98cc32ad95a5a5ab522ee2f82791663c0382c62c80367b0afb742ae3` |
| `crates/gateway-application/src/dto/verification_dto.rs` | `43504b64078ec47d756fcc808ac60d80bb87a4b5c113785876b39b6cd3401981` |

Validation for this review: source HEAD/status inspection and pinned-byte
comparison of 17 principal reviewed files exited 0. Final read-only Python
validation exited 0: all 17 bundled auth operations exactly match the table's
IDs/methods/paths, all eight recorded file hashes match, the supplied bundle hash
is unchanged, link security is Bearer AND state cookie, and the only new/changed
file is this document. `git diff --cached --check` is reported in the worker
handoff. No files outside this assigned document are
changed. Builds, producer/consumer tests, real HTTP/cookie/CORS/CSRF/provider
journeys, cross-tab races and live-provider acceptance remain **NOT_RUN** for
this review. C03 ownership and freeze stay with the integrator.
