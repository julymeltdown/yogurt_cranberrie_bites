# Browser refresh contract

Scope: the existing BE03 public refresh producer consumed by FE09. This document
does not change the wire contract. Fresh execution pins and limits are recorded in
[`browser-refresh-qualification.md`](../../../docs/mcr/backend/browser-refresh-qualification.md).

## Request and response

Send `POST /api/auth/refresh` with `credentials: 'include'` and
`X-MCR-Intent: browser`. No JSON body or bearer token is required. The browser
supplies the `refresh_token` cookie and `Origin`; JavaScript must not read or copy
the cookie. Gateway requires exactly one explicitly allowlisted `Origin` and
exactly one intent header with that value, before downstream effects. CORS must
allow the actual application origin with credentials; a cross-site application
cannot rely on a `SameSite=Lax` cookie for fetch requests.

Success is HTTP 200 with `Cache-Control: no-store`, a replacement `Set-Cookie`,
and camelCase JSON containing `userId`, `accessToken`, `refreshToken: ""`,
`tokenType: "Bearer"`, `expiresIn` (seconds), and `linkedProviders`. `userId` is the
canonical Auth UUID. Keep the access token in memory. Neither bearer nor refresh
credentials belong in persistent browser storage, URLs, telemetry, or peer-tab
messages. Browsers hide `Set-Cookie` from response-header JavaScript.

## Cookie and lifetime

Gateway issues `refresh_token` with `HttpOnly; Path=/; SameSite=Lax` and
`Max-Age=GATEWAY_REFRESH_COOKIE_MAX_AGE_SECONDS` (default 1,209,600 seconds).
`GATEWAY_REFRESH_COOKIE_SECURE` adds `Secure`;
`GATEWAY_REFRESH_COOKIE_DOMAIN` adds `Domain` when nonempty. Production validation
requires Secure and a nonempty configured domain. Logout clears the same cookie
scope with `Max-Age=0`. Cookie deletion requires matching the deployed path and
domain; the qualification fixture uses a host-only cookie and ephemeral HTTPS.

Auth separately controls signed credential expiry with
`AUTH_ACCESS_TTL_SECONDS` (default 900) and `AUTH_REFRESH_TTL_SECONDS` (default
1,209,600). Access TTL is validated within 60–86,400 seconds and refresh TTL
within 300–31,536,000 seconds. Cookie lifetime alone does not extend signed or stored validity.
Gateway's pinned RS256 JWT verifier uses the library's default 60-second expiry
leeway. Qualification waits past actual signed expiry and that leeway.

## Rotation, races, and reuse

Auth locks the canonical active user before the predecessor token. One database
transaction records `ROTATED`, equal `rotated_at`/`revoked_at`, and `successor_jti`,
and inserts a real successor in the same family. Success advances the cookie;
there is one live successor. Independent logins create independent families.

| Presentation | HTTP outcome | Cookie and family effect |
| --- | --- | --- |
| Live valid credential | 200 | Replace cookie; rotate within the family |
| Same predecessor already rotated, within the default 30-second grace | 409 | No `Set-Cookie`; no successor disclosure or additional rotation |
| Rotated predecessor after grace | 401 | Revoke that family, including its live successor; unrelated families remain live |
| Missing, expired, unknown, or logout-ended credential | 401 | No successful renewal |
| Missing/unallowlisted Origin or wrong/missing intent | 403 | Reject before Auth effects |

409 is a benign concurrent-rotation outcome, not an instruction to replay an
uncertain POST. A tab that lost the race cannot recover the winner's bearer token
from the cookie. Use the frontend's cookie-writer coordination and fail-closed
session policy. A lost response may mean rotation committed while the browser
still holds the predecessor. Repeating that predecessor within grace yields 409;
after grace it can end the family. Chrome may itself resend once after a
pre-header EOF even when JavaScript calls fetch once; the observed retry receives
409 without another rotation. Do not add application retries to this uncertainty
or blindly retry mutations.
Logout presented with an ancestor still revokes the family's live descendant.

FE09 must retain one refresh flight per session/token generation, serialize
cookie writers across tabs, reject stale completion after logout, and clear
private authority on failed or uncertain renewal. Only safe protected reads may
be retried under its explicit policy; mutations must not be automatically replayed.
Enabling and proving the production frontend remains the frontend owner's task.

## Owning source

- Gateway: `services/api-gateway/src/adapters/in/http/{browser_auth,refresh_routes,auth_routes,mod}.rs` and `src/config/mod.rs`.
- Wire DTO: `crates/gateway-application/src/dto/auth_dto.rs`.
- Auth policy: `crates/auth-application/src/use_cases/login.rs`.
- Atomic persistence: `services/auth-service/src/adapters/out/persistence/auth_transactions.rs`.
- Family classification/logout: `services/auth-service/src/adapters/out/cache/session_store.rs` and migration `0008_refresh_token_families.sql`.
