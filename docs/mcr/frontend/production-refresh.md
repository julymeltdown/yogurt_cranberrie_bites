# FE09 production refresh

Original FE09 frontend implementation criteria: **SATISFIED, task-local**. This
delivery closes the remaining production refresh caller and browser-policy
qualification. The retained session/sign-in implementation and the fresh 624
Auth/shared/shell regressions cover truthful login/provider states, safe bootstrap,
canonical memory authority and private-state cleanup. This declaration does not
edit the central task ledger or claim whole-product/release acceptance.

Assignment base: `cb9abce48d78227bbdedf7ea59ce95e9e0270c4a`. Branch:
`codex/mcr-fe-FE09-production-refresh`, in the preserved isolated root checkout.
Exclusive writes: `frontend/src/main.tsx`, NEW
`frontend/src/app/__tests__/production-refresh/**`, and this document. No Auth
model, existing test, shared HTTP, World, contract, dependency or lockfile changed.

## Result and contract

The actual application entry now passes `qualifiedRefresh: true` to its existing
single `createSessionController`. The same controller, memory session and privacy
owner still feed production ports/router. Only protected GET recovery can enter
its coordinated refresh path; sensitive secret reads and mutations keep their
existing non-replay policy. Startup does not restore cookies or renew in the
background. Access expiry still removes memory authority.

This activation supersedes the historical disabled-caller statement in
`session-sign-in.md` and the old test-only description of this constructor flag
in `SessionOptions`; neither historical file is part of this write lease.

The actual producer prerequisite is BE03 revision
`0a8c422645458c9397af9289e15089ee3f9cb0ad`, qualified by published closing
`86cf6bcb280c002becb74ca54d2d2f94e04d1637` through
`8d2834592eee51385f048f7ca613bff18c8d0ec7`:

- [Browser contract](../../../backend/docs/api/browser-refresh-contract.md).
- [Backend qualification and frozen receipt](../backend/browser-refresh-qualification.md),
  native receipt SHA-256
  `e46cac5739809af1ead93d707efa699b2790a180d85346e771d82911fc439957`.

That producer receipt covers six native services, actual Chrome cookies, signed
expiry/leeway, atomic rotation/family revocation, migration/restart, native tab
locking and committed response loss. Its 12 phases passed at that exact pin.
This supersedes older backend NOT_RUN transport statements only at that pin.

The frontend fixes URLs to `location.origin` and uses `credentials: 'same-origin'`.
Owner232 confirmed this sends/adopts the same cookies as `include` for this
composition; no cross-origin Gateway configuration is required or introduced.
Native Chrome verifies automatic Secure/HttpOnly/Lax cookie participation, exact
Origin and `X-MCR-Intent: browser`, empty refresh bearer, hidden Set-Cookie and
empty persistent storage. Refresh remains one application fetch. A 409 cannot
recover another tab's bearer; failed or uncertain renewal removes private
authority. There is no application refresh retry.

## Frontend runtime evidence

The NEW `native.mjs` starts an owned ephemeral HTTPS Vite server and installed
Chrome with trust restricted to its temporary certificate's SPKI. It loads the
real `/login` and `/src/main.tsx`. An append-only test transform exposes the
entry's actual controller/ports/router and equivalent teardown; it does not
replace their factories, options or cookie policy. Source and transformed-entry
SHA-256 values are recorded. The inspection bridge is absent from production
output. Cookies, Web Locks and BroadcastChannel are native browser facilities.
HTTP responses, credentials and protected-read bodies in this frontend runner
are explicitly synthetic public-contract fixtures.

| Actual frontend scenario | Result |
| --- | --- |
| Three old-token GETs, two immediate 401s and one delayed until after renewal | One refresh application fetch, one replay per read, stable identity generation, token version 1, current private observer retained |
| Mutation 401 | One mutation dispatch, zero refresh/replay, immediate private cleanup |
| Refresh 401 | Concurrent reads reject; authority, observer/cache, draft and subscription removed; later caller cannot retry |
| Refresh 409 | Same cleanup; uncertain cohort stays closed; a newly opened peer receives the latch via hello; explicit logout remains allowed |
| Malformed successful refresh | No credential adoption or read replay; private cleanup; no application retry |
| Pre-header EOF | No application retry or successful read replay; private cleanup; explicit logout allowed |
| Logout while server holds refresh | Local authority/signal/query/observer/draft/subscription cleared before server release; stale result cannot restore identity; zero read replay |
| Two actual root controllers in one browser context | Native cookie-policy lock queues writers, peak dispatched cookie-writer overlap 1; peer writer revokes prior private authority |

Every scenario closes channels and leaves zero held/pending Web Locks; owned
browser/server/certificate cleanup passes, with zero page errors. The EOF
frontend fixture observed one transport attempt. The separate actual-backend
loss probe observed **one application fetch, two Chromium transport attempts,
409, one committed rotation and no adopted cookie**. These are different runs;
the frontend fixture does not prove database rotation. Both require fail-closed
behavior without an extra application retry. Server overlap during the held
logout race can be 2 because the browser abort releases the lock while the
synthetic server still holds the abandoned response; this is not the serialized
two-tab writer scenario.

An initial fixture failure compared Origin against HTTP/1 Host on an HTTP/2
server. Capturing the actual header showed correct Origin for both referrer
policies. The fixture now compares the fixed actual listening origin. No shared
HTTP defect was found, and the requested support allocation was returned unused.
A later peer may abort in-flight capabilities discovery upon receiving uncertainty;
either ready-but-disabled or error-but-disabled is truthfully unavailable.

## Validation and reproduction

Durable evidence lives in
`frontend/src/app/__tests__/production-refresh/evidence/`:

- `native.json`: eight actual-main/controller/browser-policy scenarios and exact source pins.
- `unit.log`: **624 tests / 23 files PASS**, including all retained Auth, shared
  HTTP/privacy, shell, recovery/enrollment and public-entry regressions.
- `build.log`: strict TypeScript and production Vite build PASS; retained large
  Phaser/OpenAPI chunk advisories remain.
- `lint.log` and `checks.json`: scoped lint, source/bundle/boundary/provenance checks.
- `broad-diagnostic.log`: the broader unfiltered Vitest invocation **FAILED**,
  198 files passed / 8 failed, 3368 tests passed / 10 failed / 14 skipped. It
  imports three Node-test E2E files as Vitest suites, lacks the explicit C06 Python
  environment, and reports Meeting/Mural/Replication/Admin failures. These are
  outside this patch's edits; this delivery does not claim a green global suite
  or silently acquire their fixes. Machine checkout prefixes in logs are redacted.

Run from `frontend/`, using the already installed retained dependencies and Node
24.22.2; no installation or lockfile resolution is required:

```sh
MCR_PRODUCTION_REFRESH_OUTPUT=/absolute/new/owned-output \
  node src/app/__tests__/production-refresh/native.mjs
MCR_FE14_PYTHON=/absolute/existing/contracts-validation-venv/bin/python \
  ./node_modules/.bin/vitest run src/features/auth/__tests__ src/shared/api \
  src/shared/query src/app/__tests__/shell.test.tsx \
  src/app/__tests__/recovery.test.tsx src/app/__tests__/enrollment.test.tsx \
  src/app/__tests__/public-entry.test.tsx
./node_modules/.bin/tsc -b --pretty false
./node_modules/.bin/vite build
./node_modules/.bin/oxlint src/main.tsx src/app/__tests__/production-refresh
```

The native runner enforces an 8GiB free-disk floor, creates a fresh output
directory, installs nothing and uses no user browser profile. Its certificate,
browser contexts and Vite cache are removed after success/failure. Evidence
contains no real credentials, cookie values or private store URLs.

## Disposition and limits

Remaining original FE09 refresh behavior in this assignment: **none**. Production
caller activation and its specified concurrent401/delayed401/logout/private
cleanup/mutation criteria are delivered and qualified. Existing sign-in/provider
truthfulness is retained and freshly regression-tested. The actual backend proof
and actual frontend wrapper proof are both required inputs and remain separately
attributed; this is not a claimed integrated real-service frontend run.

Unfiltered repository test failures remain with the existing affected component
owners; they do not authorize changes here. Deployment cookie-domain/TLS, Safari,
external OAuth, real multiuser/world, physical-device and IN gates remain outside
this bounded original FE09 caller delivery. No new prerequisite was imposed on
the ready work, and no operator/source/fidelity admission was inferred.
