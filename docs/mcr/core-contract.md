# C03 core HTTP target contract

State: **CONTRACT_READY**, version 0.1.0. The integrator freezes the email/core
surface and the exact inherited operation mapping after consumer, schema and
producer reviews plus 33 golden fixtures and 23 core tests. This is contract
acceptance, not runtime route registration. OAuth exposure remains disabled;
enabling an SPA handoff requires a separately reviewed contract revision.

## Source and generator

`backend/scripts/docs/generate-mcr-core.py` calls the inherited **final**
`generate-openapi.py.build_document()`, selects exact inspected auth operation IDs,
and assembles authored core/asset schemas into one OpenAPI 3.1 target document.
The inherited 92-operation runtime contract remains unchanged and separately
checked. Each MCR operation identifies NOT_IMPLEMENTED or ADAPTATION_REQUIRED.
Never feed the target contract into a runtime-coverage gate as proof of routes.

Core definitions live in `contracts/core/v1/core.schema.json`, with operations in
`operations.json`. Asset definitions are owned once in C06; the assembler resolves
their pinned local IDs without network retrieval. New MCR DTOs use snake_case;
inherited Auth DTOs keep exact camelCase. Canonical `user_id` is Auth's `userId`
UUID; private query identity is `(user_id,event_id)` plus a session generation.

## Auth consumer decisions

- Browser `/api/auth/*` maps exactly to inherited Gateway `/auth/*`; existing
  content `/api/*` is not double-prefixed. The edge/adapter is still to implement.
  The browser review lists all 17 inspected operations. Public email availability
  is deliberately excluded from the MCR browser contract: neutral resend/reset
  flows do not need an account-enumeration endpoint. The other 16 IDs are retained.
- Use memory-only bearer tokens and `credentials: same-origin`, `cache: no-store`.
  AuthResponse's required `refreshToken` stays the empty string; only the HttpOnly
  cookie holds the refresh credential. Require no-store response headers on all
  secret-bearing responses. No auth data in browser persistence, URLs or logs.
- Browser mutation requests require exact allowed `Origin` AND the non-simple
  `X-MCR-Intent: browser` header. Reject absent, `null`, wildcard, suffix-matched or
  unconfigured Origin before dispatch. This applies to login/signup/verify/reset,
  refresh/logout and private mutations. Origin validation, strict CORS and the
  header are complementary; a frontend header alone is no protection. Enforce
  this in BE03, with negative producer/browser probes. OAuth callbacks use their
  dedicated one-use state-cookie/provider validation; do not apply this header
  requirement to Apple's cross-site form callback.
  Guard rejection is HTTP 403 with `OriginErrorResponse` and reason
  `ORIGIN_NOT_ALLOWED` or `BROWSER_INTENT_REQUIRED`, before downstream dispatch.
  Resend now explicitly declares 403. Every guarded operation carries the same
  machine-readable origin/rejection metadata. Other inherited auth 403 errors
  may retain null details through `AuthForbiddenResponse`.
- Production refresh/state cookies are Secure, HttpOnly, Path=/, SameSite=Lax;
  exact domain configuration must pass the inherited production validator. Do not
  weaken the domain policy silently. Native/local HTTP development remains a
  separate explicitly configured environment.
- Password signup/reset validation is **8–128 UTF-8 bytes**, matching the source.
  Unicode strings are neither trimmed nor normalized. JSON Schema's maxLength=128
  is a permissive character ceiling; `x-mcr-utf8-bytes` records the stricter rule
  for client/server tests. An eight-byte multibyte string may have fewer than
  eight code points; the generator removes the inaccurate character minimum.
- Error `traceId` is a bounded opaque nullable string, not necessarily a UUID.
  Preserve top-level code/message/traceId/timestamp/details; machine decisions use
  `details.reason`. English message text is not an enum. Password-reset status is
  not a delivery receipt; show neutral acceptance.
  New core operations use `CoreErrorResponse` with the inherited envelope and a
  required registered reason plus only safe, reason-specific detail fields.
  Generic inherited auth errors retain `ErrorResponse`, where details may be null.
  Core operation/status schemas further constrain valid reasons for that status.
  For inherited auth errors without a recognized reason, consumers use HTTP
  status and the bounded code for generic recovery; never match English text or
  assume a refresh conflict detail exists. 401 login remains failed credentials,
  429 is rate limiting and uncertain 409/5xx is not automatic replay permission.
- Logout allows a missing cookie and still clears matching cookie attributes.
  Origin/intent checks remain mandatory. Clear local identity immediately, cancel
  private requests/subscriptions and invalidate the session generation. A failed
  server revocation must remain distinguishable from confirmed logout.
- One refresh may be in flight per active session. Do not automatically retry
  login, refresh, reset, callback, link or mutations. An authenticated read may
  replay once after successful refresh only in the same session generation.
  Generic 409 is not a retry instruction. Cross-tab refresh coordination and
  uncertain rotation recovery must be qualified before enabling automatic flows.

## Core DTO decisions

- Bootstrap is authenticated and may idempotently create a default participant
  keyed by `(event_id,user_id)` without manufacturing optional downstream data.
  Required identity/policy dependencies failing returns 503; optional features
  report unavailable capabilities. Server time is UTC with `Z`; display uses KST.
  Bootstrap also delivers all three `theme_manifests` using the exact C06 schema;
  each must bind the same map/avatar content hashes, geometry, places and
  interactions. This is the authoritative discovery path for FE06/FE14. No
  undocumented theme URL convention or production fixture import is needed.
- PublicProfile is a strict allowlist: UUID, display name, optional bio and avatar.
  It cannot contain email, exact age, provider claims, handles, preferences or
  private meetings. PrivateParticipant additionally includes own preferences and
  a coarse age status; unknown is never verified. Block/suspension/read eligibility
  is checked by the producer, not encoded as exposed private policy reasons.
- Participant/profile writes and avatar writes include `expected_revision`.
  A stale revision returns 409 with VERSION_CONFLICT; preserve the user's draft,
  fetch current state and require an explicit retry. AvatarConfig references only
  C06 item IDs/manifest version, never arbitrary client URLs.
- Places reuse the C06 Place schema and provenance. Virtual concepts cannot be
  marked verified physical places. Physical guidance requires reviewed source
  provenance and current closure state. Neither a virtual coordinate nor a golden
  fixture establishes real arrival or safe venue access.
- Event READ_ONLY/ARCHIVED disables mutation capability. Capabilities report
  availability with bounded reasons; they are UX hints, never authorization.
  All writes recheck event state, membership, policy, version and item validity.
  New core transport failures retain 413/415/422/500/502/504 as applicable. The
  MCR core HTTP adapter must normalize all core errors to safe `CoreErrorResponse`
  reasons (including extractor, admission and deadline errors); it cannot reuse
  the inherited null-details error wrapper unchanged. This is an explicit
  producer obligation, not evidence of an existing route. Auth remains separately
  compatible with its inherited generic error envelope.
- Public auth capabilities list EMAIL/GOOGLE/APPLE/X truthfully. Enabled means
  configured and the complete browser handoff qualified. Disabled optional OAuth
  providers remain disabled; enum membership cannot enable a button. No secrets
  or client credentials are returned. Naver/Instagram are not auth providers here.

## Frozen OAuth exposure policy and remaining BE03 work

Retain the exact inherited OAuth schemas as mapped references, with
`DISABLED_UNTIL_HANDOFF_QUALIFIED` exposure. These operations must not be offered
by the MCR browser or enabled by capabilities in v0.1. No provider enum or secret
configuration may override this policy. This allows the required email flow and
core consumers to implement against a frozen contract without inventing OAuth
behavior. It is not a permanent removal of optional configured providers.

OAuth navigation currently returns JSON, and Apple's intermediate form posts to
absolute `/auth/oauth/apple/callback/complete`. A same-origin edge must preserve
that callback route or adapt its form action. Choose and qualify the SPA callback
handoff, explicit allowed return targets, account-link binding and error recovery
before enabling providers. Never put bearer tokens in a redirect URL. The draft
retains exact source callback schemas and marks their browser exposure disabled.

The new schemas are target obligations. Source auth response caching, origin
checks, actual MCR ingress and producer schema corrections remain unimplemented.
Fixture validation proves shapes and relationships only; real HTTP/cookie/CSRF,
provider, concurrent refresh and multiuser privacy acceptance remain NOT_RUN.

## Current validation

`scripts/mcr/check.sh contracts` checks both deterministic generators, the C06
bundle, all 33 indexed C03 examples, 21 asset test cases and 23 core test cases.
Core failures include leaked private profile fields, cross-user/event responses,
unknown avatar items, inconsistent bootstrap avatars, writes during read-only
events, duplicate provider/place IDs, leaked refresh credentials, stale/empty
update revisions and invalid UTF-8 password lengths. Place/public-profile fixtures
bind requested event/place/target-user context, and operation/status validation
rejects inappropriate error reasons. The inherited 22 generator regressions pass
separately. Schema-review R01–R04 and producer contract requests PR01/PR05/PR06
are resolved in their review records. PR02–PR04, actual Origin enforcement, core
error normalization and real Auth journeys remain BE03/runtime obligations.
