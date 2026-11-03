# C03 core producer review

2026-11-03. Scope: read-only comparison of the C03 draft with the actual inherited
Gateway/Auth producers. This review changes only this document. It does not
implement BE03, freeze C03, review asset constraints, run a build, start a
service or accept a browser authentication flow. The parent owns the freeze.

## Snapshot and assessment

Worker base: `520a0df80da62b1a2a52c3ab1ee7f0d7219620a1`. The worker checkout does
not contain the new C03 draft, so draft inputs and current producer sources were
read directly, without modification, in `/Users/lhs/Desktop/mcr`. Its observed
HEAD was `a9746c6042ee25b68ccada307375fb6221c9c52b`; the draft files were untracked,
so that HEAD alone does not identify them. The draft changed during review; the
final inspected snapshot below includes the parent's explicit legacy-auth/core
error distinction and 25 fixtures (23 initially). Exact SHA-256 snapshot:

| Input | SHA-256 |
|---|---|
| `contracts/core/v1/core.schema.json` | `f7f0ef7ce4554301a21775eab80df817420794ebb3494f0d18346f612070f079` |
| `contracts/core/v1/operations.json` | `2f5b972dbccea09340e1519a1a2c522e3c42f63690fa26b9e58d721e9927ba62` |
| `backend/scripts/docs/generate-mcr-core.py` | `12d0ea30ccc3e7599eec5f9148db682d1c90eab84a6f4da8f1cd9d0431617cd5` |
| `backend/docs/api/mcr-core.openapi.json` | `9a7fb8046298a65266122f652f6a9918c1d724932d085f33b48ef2c390975284` |
| `docs/mcr/core-contract.md` | `f27b4c06fa50ad136e33220be8b8095324ff39edda4efc2ac2797130c09be5ed` |
| `contracts/fixtures/core/index.json` | `e178cbc42ed02bc81d3780ddab1373463b74dbf0783d645f9c636afa3d291777` |

The final inspected index names **25 fixtures**. Their set digest is
`a3618815f9eb486f777643c4f19861882ab3af53face47938845ffbd781df7f2`, computed as
SHA-256 of newline-joined `filename:sha256(contents)` entries, sorted by filename,
without a trailing newline. Fixtures were inspected as examples, not served by
a runtime. Builds, generators, tests, HTTP requests and database checks were
**NOT_RUN** in this review; commands were Git metadata, file reads/searches and
read-only Python JSON/hash analysis. Existing tests below are evidence pointers,
not fresh passing test claims.

The source-aware decisions for password bytes, nullable opaque trace IDs,
cookie-less logout and disabled OAuth exposure are appropriate. The draft
correctly labels inherited operations ADAPTATION_REQUIRED and new endpoints
NOT_IMPLEMENTED. The actionable contract correction is to freeze the status and
envelope for mandatory Origin/intent failures, including resend. Implementation
gaps below belong to BE03 and must remain explicit until executed producer tests.

## Findings and minimum changes

### PR01 — Freeze the Origin/intent rejection response for every browser mutation

The generator adds required `X-MCR-Intent: browser` to inherited mutations at
`backend/scripts/docs/generate-mcr-core.py:112`, and the document requires an
exact configured Origin before dispatch. However, the inherited resend operation
has no **403** response: its statuses are 202, 400, 415, 422, 429, 500, 503, 504,
413 and 502. Other inherited auth mutations already declare 403. A conventional
`GatewayError::forbidden` guard would therefore produce an undocumented status
on `POST /api/auth/email/resend`.

**Before freeze:** choose the rejection status and safe machine reason(s) for
missing/disallowed Origin and missing/wrong intent. If 403 is selected, add its
ErrorResponse to resend and make the rule consistent across the mutation set.
Record the inherited operations' origin policy in machine-readable metadata as
well as prose; currently only newly authored core writes receive
`x-mcr-origin-policy` (`generate-mcr-core.py:172`). Do not use different failure
statuses merely to avoid updating an inherited response table.

**BE03 producer work:** add the actual boundary guard. No Origin/intent check is
present in the inspected `auth_routes.rs`, `refresh_routes.rs` or current router
layers (`services/api-gateway/src/adapters/in/http/mod.rs:256`). Use exact,
validated allowlist entries; reject absent/null/unconfigured/ambiguous values
and wrong or missing intent before body handling or downstream dispatch. Preserve
the explicit callback exceptions; do not demand this browser header from an
OAuth provider's cross-site callback.

Both CORS compositions need coordination:
`services/api-gateway/src/config/mod.rs:1828` and
`services/api-gateway/src/adapters/in/http/server.rs:40` omit `x-mcr-intent`,
and the transport composition replaces the builder's header list. Changing only
the first list leaves configured cross-origin preflight broken. Same-origin
requests do not need CORS permission; they still need the server guard.

Required probes: each missing/invalid header case, explicit allowed Origin,
resend's chosen status/envelope, zero downstream calls on rejection, and the
complete transport wrapper rather than only a bare Router test.

### PR02 — Apply no-store to the actual auth response boundary

`x-mcr-cache: NO_STORE` is a target annotation, not an HTTP response header.
Login (`…/http/auth_routes.rs:116`), refresh (`…/http/refresh_routes.rs:30`) and
the GET OAuth callback (`…/http/oauth_routes.rs:116`) construct cookie/token
responses without inserting Cache-Control. Existing no-store helpers are
path-specific: resend (`…/http/verification_routes.rs:80`) and Apple POST
callbacks (`…/http/oauth_routes.rs:21`). The outer transport helper sets no-store
for idempotency replays, not all auth responses (`…/http/server.rs:138`).

**BE03:** wrap the MCR auth surface so both successful credential responses and
errors from guard/extraction/admission/deadline/transport layers get no-store.
Preserve the existing Apple no-referrer/nosniff/CSP behavior. Do not rely on the
frontend's fetch cache option to protect an intermediary or navigation response.
Make the chosen `/api/auth` versus upstream `/auth` path handling work for these
helpers; their current literal path matches must be covered at the real ingress.

Required probes: login/refresh success, invalid JSON/field types, Origin denial,
rate limit, timeout and unavailable Auth, all through the final composed router.
The draft can freeze this as an implementation obligation; it must not claim the
source already provides it universally.

### PR03 — Opaque traceId is correct; correlation identity still diverges on errors

The new string-or-null traceId with maxLength 128 is compatible with the normal
source path. `crates/platform/src/correlation.rs:54` accepts bounded trimmed
opaque identifiers, and `…/http/mod.rs:195` uses the current correlation value
for body and header. A UUID-only schema would reject legitimate `corr-123`.
The source's bound is 128 UTF-8 bytes; a 128-character schema is a permissive
outer bound rather than the producer's exact normalization algorithm.

Two concrete producer paths still split the identifiers:

- Resend's `failure` creates a new body UUID (`…/http/verification_routes.rs:70`)
  while its outer response wrapper preserves the incoming/generated header
  (`verification_routes.rs:84`). For example, malformed resend JSON with
  `X-Correlation-Id: producer-review-123` returns that header but a different
  body traceId by the inspected control flow.
- The transport rejection for an unsupported Idempotency-Key first builds a
  GatewayError response, then independently replaces its correlation header
  (`…/http/server.rs:124`). An auth request carrying that key takes this branch
  outside the ordinary telemetry layer.

**BE03:** normalize one identifier before rejection-capable layers and reuse it
in both body and response header, including these two paths. Preserve nullable
trace compatibility; do not coerce client correlation values into UUIDs.
Add body/header equality assertions to
`services/api-gateway/tests/verification_http.rs` and the transport-level auth
error probes. Existing resend tests check header presence/bounds, which does
not establish equality. These are static findings, not executed HTTP results.

### PR04 — Keep UTF-8-byte signup/reset behavior; correct the producer message

`crates/auth-application/src/use_cases/signup.rs:142` validates Rust `str::len()`
in the inclusive range 8–128, so it counts UTF-8 bytes. Reset confirmation calls
the same helper (`…/use_cases/password_reset.rs:110`). Neither path trims or
normalizes the password. `signup-unicode.json` contains `가나다` (9 bytes, three
code points), which is valid for this rule. The generator's removal of the
eight-character minimum and its explicit `x-mcr-utf8-bytes` extension correctly
preserve producer behavior.

The current producer error text says **“8-128 characters”**, contrary to its byte
check. **BE03:** correct that text and add producer boundary tests for both
signup and reset; do not change hashing semantics, normalize input, or reinstate
minLength=8. Test 7/8/128/129 bytes, multibyte values, unchanged whitespace and
password round-trip verification. The inspected core validator handles the byte
extension, but a generic JSON Schema/OpenAPI tool need not enforce extensions.
Keep authoritative server validation. The login password schema is not a
signup/reset rule; do not silently apply their minimum to login.

### PR05 — Legacy auth/core reason distinction was resolved during this review

The initial draft did not distinguish its general machine-reason guidance from
legacy auth errors. The inherited ordinary GatewayError body deliberately
uses `details: None` (`…/http/mod.rs:203`); resend and extractor errors also use
null details. The latest inspected draft explicitly retains inherited
`ErrorResponse` with nullable details for auth, uses `CoreErrorResponse` with
registered reasons for new core routes, and adds auth unauthenticated/rate-limit
fixtures. This resolves the documentary ambiguity; it is not an outstanding
request to require reasons on every inherited auth error. Consumers must retain
that distinction and never match English messages. Future core producers must
actually supply the required reason (see PR06). Domain-error fixtures do not
require party/reservation/mural implementation during BE03.

### PR06 — Reconcile new core status tables with the inherited Gateway layers

New operations are given only 400/401/403/404/409/429/503 errors at
`backend/scripts/docs/generate-mcr-core.py:148`. Their two JSON writes will
typically use the same Axum JSON boundary as existing routes, whose normalization
preserves **415** for missing JSON Content-Type and **422** for wrong field types
(`services/api-gateway/src/adapters/in/http/extractor_rejections.rs:84`). The
shared GatewayError and deadline paths also emit **500/504**
(`…/http/mod.rs:175` and `:854`). The existing normalizer wraps recognized errors;
it does not collapse those statuses to 400 or 503.

In addition, the new operation error bodies require registered reasons, while
the shared GatewayError/extractor bodies have null details. An unmodified
Gateway error is therefore invalid for a new core operation even when its HTTP
status is declared. This also affects the new BE03 auth-capabilities endpoint;
legacy inherited auth operations intentionally keep their older envelope.

These new routes are explicitly NOT_IMPLEMENTED, so this is a contract/producer
integration decision rather than a claim of an already failing endpoint.
**Before freeze:** include the applicable transport statuses and ErrorResponse
forms, or specify and later implement an intentional normalization at the MCR
boundary. Assign safe registered core reasons without parsing English messages.
Test representative malformed JSON, missing media type, invalid DTO,
internal failure and downstream deadline against the chosen contract. Do not
make feature workers discover this mismatch independently or parse plain text.

## Checked mappings and intentionally deferred behavior

The 16 retained inherited operation IDs preserve source methods and successful
statuses; the excluded availability endpoint is the seventeenth source operation.
The following is a producer comparison, not runtime qualification:

| Operation group | Inherited success | Producer observation / BE03 requirement |
|---|---|---|
| `signupWithEmail`, `verifyEmail` | POST, 200 | `auth_routes.rs:36` and `:50`; do not change to 201/204 without a contract decision. |
| `resendVerificationEmail` | POST, 202 | `verification_routes.rs:49`; body status ACCEPTED, not SENT/DELIVERED. |
| `login`, `refreshAccessToken` | POST, 200 | Refresh credential is in HttpOnly cookie; public AuthResponse.refreshToken is empty. |
| `requestPasswordReset`, `confirmPasswordReset` | POST, 200 | Status SENT is inherited; request-reset acceptance is neutral and asynchronous (`auth-application/…/password_reset.rs:99`). It is not delivery proof. |
| `logout` | POST, 204 | No cookie means no downstream logout call, but cookie clearing still happens (`auth_routes.rs:129`). The draft security override to `[]` is correct. |
| `deleteAccount` | DELETE, 202 | Authenticated asynchronous saga; do not call it immediate physical deletion. |
| `getAccountDeletion`, `getAccountDeletionStatus` | GET, 200 | Same owning handler via two aliases; missing saga yields 404. |
| `authorizeOAuth` | GET, 302 | Provider redirect plus state cookie; no browser-token handoff yet. |
| `completeOAuthCallback` | GET, 200 | JSON AuthResponse plus refresh/state cookies, not an SPA redirect. |
| Apple receive/complete POST operations | POST, 200 | Intermediate HTML is not authentication; completion is separate. |
| `linkOAuthIdentity` | POST, 200 | Bearer plus provider-specific state binding; remain disabled until qualified. |

Cookie builders (`…/http/mod.rs:507` and `:538`) already use HttpOnly, Path=/ and
SameSite=Lax. Secure and Domain come from configuration; the production validator
requires Secure and nonempty Domain (`services/api-gateway/src/config/mod.rs:723`).
Preserve matching clear-cookie attributes and qualify actual host/domain delivery
with the parent's explicit environment design. A local HTTP test is not evidence
of production Secure-cookie behavior. Cookie-less logout still requires the new
Origin/intent guard. A present-cookie revocation failure returns an error before
the current clear-cookie code; the frontend must distinguish local identity
clearing from confirmed server revocation. This is consistent with the draft's
warning; do not relabel such failure as 204 success.

`/api/auth/*` is a planned mapping to actual `/auth/*`, not existing route
registration. BE03 must implement one deliberate edge/adapter mapping and avoid
exposing an unguarded upstream alias. Verify no double-prefixing of existing
content `/api/*`. Preserve or adapt the Apple completion form's absolute action
at `…/http/oauth_routes/form_post.rs:40`:
`/auth/oauth/apple/callback/complete`.

The missing OAuth handoff is **explicitly acknowledged and disabled in the
draft**, not misrepresented implementation. Do not enable OAuth merely because
the inherited provider enum or configured upstream provider exists. BE03 needs
the new public capabilities producer, truthful configuration/handoff availability
and deliberate route exposure. Keep provider routes disabled for the browser
until SPA return targets, same-origin token acquisition, linking, state replay,
timeout recovery and callback routing are decided and tested. Tokens must not
be placed in redirect URLs. No proposed OAuth handoff architecture is silently
selected by this review.

## Suggested bounded BE03 handoff

After the parent resolves PR01/PR06 and freezes the core contract, the minimum
producer patch should comprise: deliberate ingress mapping; Origin/intent guard
and both CORS header compositions; response no-store coverage; shared error
correlation; byte-accurate password messaging/tests; truthful capabilities and
disabled OAuth exposure; preservation of existing cookie, method and status
semantics. Central router/config/generator edits require integrator coordination.

Execute the final transport composition with negative header/serialization/status
probes, then an isolated real Auth signup/verify/login/refresh/logout journey and
refresh failure/replay cases. Existing pointers include Gateway
`tests/unit/http_auth_routes.rs` (cookie-less logout and cookie stripping),
`tests/verification_http.rs`, `tests/verification_transport.rs`, and Auth
`tests/auth_lifecycle_postgres.rs` / `tests/auth_password_reset_postgres.rs`.
No passing result for these tests is claimed here. Asset schemas and constraints
are intentionally left to the parallel asset/schema reviewer.
