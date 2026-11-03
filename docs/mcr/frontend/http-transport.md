# FE01 HTTP transport handoff

State: **MOCK_READY**. Prepared for integrator review; this is not FE09 session UI,
real-server identity acceptance, C07 generated SDK delivery or an integration gate.

## Candidate, ownership and provenance

- Worktree: `mcr-worktrees/fe-transport`; branch: `codex/mcr-fe-transport`.
- Base: `2d9198a684ec363c476ae75835ddf4289e47762c` (reviewed C03 email/core v0.1.0).
- Implementation candidate: `60a457b76373d56ceb39fa110e6321b0883aa00e`.
- Delivered ancestry: base → implementation candidate → documentation-only handoff.
  The final documentation commit is identified by `git log -1 --format=%H --
  docs/mcr/frontend/http-transport.md` on the delivered worker branch. No push or
  master merge is performed by this worker.
- Assigned and changed paths: `frontend/src/shared/api/http.ts`,
  `frontend/src/shared/api/errors.ts`,
  `frontend/src/shared/api/__tests__/http.test.ts`,
  `frontend/src/shared/api/__tests__/errors.test.ts`, and this document.
- The predecessor's two untracked drafts were preserved, reviewed and completed.
  All transport and test code is newly authored. No Bigyeting module was copied;
  no source extraction or UI allowlist exception is asserted. `docs/source/`,
  manifests, locks, backend, contracts, routers and styles have no worker edits.
- Contract inputs: `docs/mcr/core-contract.md`, `contracts/core/v1/core.schema.json`,
  `contracts/core/v1/operations.json`, mapped inherited auth operations and golden
  examples under `contracts/fixtures/core/`, all at the base revision. Tests import
  only public contract JSON, never backend implementation. The origin requirement
  and disabled OAuth exposure follow frozen C03; source auth is not assumed to
  have implemented those producer obligations.
- Runtime dependency: retained Ky **2.0.2**. No dependency was added or upgraded.
  The lockfile foundation is `605b858b3baa2bb93ce4a01a6620cd25bc648c14`.
  Qualification uses Node **24.22.2**, pnpm **11.7.0**, TS **6.0.3**, Vitest
  **4.1.9**, Vite **8.1.3** and Oxlint **1.72.0** from the retained lockfile.

## Public ports for FE02 and FE09

`createMemorySession()` owns the single canonical identity and memory-only bearer
credential. Share one `SessionPort` across all HTTP clients for an active shell.
`read()` includes `(userId,eventId,generation)`, token version and a generation
AbortSignal. `replace(expectedGeneration, credentials|null)` is compare-and-set:
login, logout, account/event changes and revocation increment generation, reset
its token version and abort the previous generation. Failed CAS returns null.
User/event identifiers are canonical lower-case UUIDs; anonymous identity has no
user, event or token. `commitRefresh(snapshot,credentials)` requires the same
user/event/generation and token version; it cannot switch identity or overwrite a
newer token. It changes token version without invalidating current private reads.

`createHttpClient({session, fetch?, timeoutMs?, refresh?})` exposes:

- `request<T>(HttpRequest<T>): Promise<{data:T,scope:SessionScope}>`.
- `refreshSession(signal?): Promise<{data:RefreshCredentials,scope:SessionScope}>`.

Each request declares method, path, public/protected access, `auth`/`core` error
family and a synchronous, side-effect-free `Decoder<T>`. The decoder receives
unknown JSON and `{scope,status,path}`; it must validate the complete DTO and
request-bound event/user/place relationships. Throwing rejects the response as
`invalid-response`. A decoder is not a type assertion. No untyped global API
barrel or backend import is introduced. C07 must supply/check operation-specific
DTO adapters against the authoritative schemas; this patch does not independently
claim to replace their generated SDK.

Consumer example (the feature owns `authResponseDecoder` and input validation):

```ts
const result = await http.request({
  path: '/api/auth/login', method: 'POST', access: 'public', errorFamily: 'auth',
  body: { json: { email, password } }, decode: authResponseDecoder,
})
const installed = session.replace(result.scope.generation, {
  userId: result.data.userId, accessToken: result.data.accessToken,
  eventId: selectedEventId,
})
if (installed === null) return // A newer login/logout/scope decision won.
```

Login requests never install credentials as a side effect. Anonymous refresh also
returns an uninstalled candidate; FE09 installs it with the same generation CAS.
A cancelled anonymous waiter receives no candidate and cannot restore a session.
For an already active identity, refresh may update only that identity's token.
The refresh decoder must validate the full inherited AuthResponse, including
`refreshToken === ''`, `tokenType === 'Bearer'`, expiry and canonical user ID,
before returning `{userId,accessToken}`. The transport additionally validates
canonical identity and bounded bearer syntax; it does not validate JWT signatures.

FE02 should use `(userId,eventId,generation)` for private cache ownership and
`session.isCurrent(result.scope)` immediately before consuming delayed results.
Drop old queries, private drafts and subscriptions when identity/policy changes;
transport cancellation does not itself evict a query cache or close a world socket.
Bind a decoder to the requested public-profile target rather than assuming every
returned `user_id` must equal the signed-in user.

For logout, clear local identity with `replace(currentGeneration,null)` **before**
dispatching `/api/auth/logout`, evict private state, then send its public POST with
an explicit decoder. Keep failed/uncertain server revocation distinct from a
confirmed response. A successful response must never reinstall old credentials.
Use the generation signal for related pending work. Browser cookie effects of a
late logout still need FE09/BE03 qualification alongside new login/refresh flows.

## Routing, dispatch and cancellation

Only absolute browser paths starting `/api/` with plain alphanumeric, underscore
or hyphen segments are accepted. Absolute URLs, protocol-relative URLs, encoded
segments, backslashes, traversal, empty segments, query/fragment-in-path, trailing
slashes and old `v1` segments fail before fetch. Queries are separate scalar
values; secret/token/password/auth/code/state keys are rejected. Event-scoped
concert paths require a protected request bound to the current event UUID.
The URL resolves against the browser's current HTTP(S) origin; no backend origin,
secret, old `/v1` adapter or persistent credential store is configured.

Frozen email auth paths/methods/access are checked. Capabilities is public GET
with core errors; account/deletion reads are protected and never auto-refresh.
Generic requests cannot directly call refresh; use the dedicated flight port.
OAuth routes and email availability are rejected under the v0.1 exposure policy.

Ky has `retry:0`, `throwHttpErrors:false`, `timeout:false` (the transport owns one
whole-request deadline), `credentials:'same-origin'`, `cache:'no-store'`,
`redirect:'error'` and `referrerPolicy:'no-referrer'`. The transport supplies JSON
Accept, protected bearer and exact `X-MCR-Intent: browser` for every non-GET.
Caller headers are limited to bounded printable X-Correlation-Id, X-Request-Id and
X-Client-Id values. Authorization, cookie, Origin, content/intent, infrastructure
and arbitrary headers cannot be overridden. Public requests never inject bearer.
Origin enforcement remains a server obligation; this header does not prove CSRF
protection.

JSON is serialized once. `FormData` uses the native multipart boundary; a test
reads the received file bytes and name through Request.formData(). No upload
progress, image compression, presigned external PUT or finalize receipt is claimed.
FE04 owns those separate media ports; external signed URLs do not belong in this
same-origin API client.

The default deadline is 15 seconds; integer per-client/per-request overrides are
1–120,000 ms. It covers fetch, refresh waiting, one optional replay, body streaming
and decoding. Pre-abort does not dispatch. User cancellation, timeout and session
replacement reject promptly even if an injected fetch ignores its signal. Stream
collection caps JSON at 4 MiB and cancels/releases its reader; non-JSON, malformed
UTF-8/JSON, oversize and redirected responses never reach the decoder. Late fetch
responses are disposed. A synchronous decoder cannot be interrupted mid-execution,
but elapsed monotonic deadline and cancellation/current-generation checks run again
before returning its value.

## Refresh and uncertainty rules

Refresh recovery requires both the client `refresh.decode` option and per-request
`refresh:'once'` on a **protected GET**. Neither is product-enabled by this patch.
Auth operations, capabilities, public reads and mutations cannot opt in. Login,
refresh, OAuth, resets, account operations and all mutations are never replayed.
A second 401 after the sole read replay is returned as a failure.

One refresh promise is shared per SessionPort/generation/token version, including
explicit refresh calls and separate clients using that port. Parallel 401s join
it; staggered old-token 401s reuse the newer token. Failed rotation remains latched
for that token version so a subsequent request cannot blindly retry an uncertain
cookie mutation. A new identity generation or an explicitly newer token permits a
new flight. There is no automatic loop, Retry-After scheduler, network retry or
conflict replay. A token CAS loser returns the winning token, never the stale grant.

Cancelling one waiter does not cancel the shared refresh for another waiter.
An active identity may receive a same-generation token rotation after its reader
is cancelled; it cannot resurrect an identity cleared or replaced by logout,
account/event change or revocation. The shared rotation retains its own bounded
deadline. FE09 must arbitrate user-visible recovery after failed/uncertain refresh
rather than bypassing the latch with repeated new sessions.

`TransportError` distinguishes HTTP, network, aborted, timeout, invalid request,
invalid response, session changed and authentication required. It contains bounded
status/code/envelope/retry hint/uncertainty, without Request, Response, cause or raw
proxy payloads. Unknown English messages never select machine behavior.

- Inherited `auth` errors accept bounded copied JSON details or null/missing
  details. Unknown generic detail reasons remain untyped; use status and bounded
  code for generic recovery.
- `core` errors require a registered reason and only reason-specific safe fields;
  unknown envelope/detail fields are rejected. Revision and cooldown/rate hints
  have integer bounds. Origin errors are separate strict 403 envelopes for
  ORIGIN_NOT_ALLOWED/BROWSER_INTENT_REQUIRED, including when auth normally allows
  null details. Core/origin timestamps require UTC `Z` shape.
- This generic parser validates envelope/detail shapes, **not every operation's
  allowed reason/status combination**. Feature clients must constrain those using
  generated operation contracts before choosing domain recovery. C07 checks remain
  necessary; generic reason membership alone is not permission to retry.
- Retry-After seconds/HTTP-date and core retry_after_ms are hints bounded to 24
  hours. Invalid hints are discarded. No hint authorizes an automatic replay.
- After dispatch, mutation network/abort/timeout/stale/invalid-response failures
  are `uncertain:true`; HTTP 408/5xx and inherited auth 409 are also uncertain.
  Known core 409/403/429 retains status and does not replay. Preserve a draft and
  use operation-specific state/receipt lookup or explicit user action; do not
  show success from a transport failure. Pre-dispatch validation/abort is certain.

Generation/CAS proves local stale-result suppression only. It cannot suppress
browser-applied Set-Cookie, coordinate refresh across tabs, establish server token
reuse/rotation semantics or prove revocation. Cross-tab refresh coordination,
HttpOnly/Secure/SameSite cookie behavior, no-store producer headers, CSRF/CORS,
late cookie mutation races and real refresh/revocation remain **NOT_RUN**. They
must be qualified before FE09 enables automatic product refresh.

## Regression evidence

Commands run from the isolated worktree with PATH prefixed by
`/Users/lhs/Desktop/mcr-tools/node-v24.22.2-darwin-arm64/bin`:

| Command | Exit | Result and scope |
|---|---:|---|
| `corepack pnpm@11.7.0 --dir frontend install --frozen-lockfile` | 0 | Retained installation; no lock/manifest change. |
| `corepack pnpm@11.7.0 --dir frontend test src/shared/api/__tests__` | 0 | 144 mock Request/Response tests, two suites; no unhandled errors. |
| `corepack pnpm@11.7.0 --dir frontend build` | 0 | Strict TypeScript project build and Vite production build. Transport is not wired to product UI yet. |
| `corepack pnpm@11.7.0 --dir frontend exec oxlint src/shared/api` | 0 | Scoped transport/test lint; zero warnings after final cleanup. |
| `python3 scripts/mcr/check-plan.py` | 0 | Planning/provenance baseline intact; not application acceptance. |
| `node scripts/mcr/check-ui-origin.mjs --root <worktree>` | 0 | Static C08 source/import scan; not visual/browser acceptance. |
| `python3 scripts/mcr/check-boundaries.py --base 2d9198a684ec363c476ae75835ddf4289e47762c --lane frontend --allow-doc docs/mcr/frontend/http-transport.md` | 0 | Exact documentation exception; assigned changed paths and imports manually inspected. |
| `git diff --check` (plus staged check before commits) | 0 | No whitespace errors in the delivered patch. |

The test cases execute the retained Ky path with native Node Request, Response,
FormData, AbortController and ReadableStream objects through a mock fetch. They
cover simultaneous/staggered 401 and refresh sharing, failed-rotation latches,
one replay, CAS winners, logout/account/event changes, late login/logout/refresh,
anonymous resume/cancellation, waiter cancellation, malformed/cross-user grants,
public/auth/mutation no-replay, 403/409/429/408/5xx, network/abort/deadline including
stalled bodies, null inherited errors, strict core/origin details, Retry-After,
malformed/non-JSON/oversized/redirected responses, decoder rejection, forbidden
URLs/headers, no credential persistence and multipart bytes.

Initial harness runs exposed an incorrect test-file location/import and attempts
to inspect request bodies after Ky disposal. These were corrected by placing the
suite in its assigned path and consuming bodies inside mock fetch. A strict-type
failure in optional test headers and an intentional control-character lint warning
were also resolved. Earlier failing runs are not used as acceptance evidence.

No HTTP server, provider, database, browser or physical device was started for
FE01. Real identity, cookie rotation, cross-tab coordination, multiuser privacy,
loads, retries against producer receipts, FE09 UI and IN01–IN05 remain **NOT_RUN**.
