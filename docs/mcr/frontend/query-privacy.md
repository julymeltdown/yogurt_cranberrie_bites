# FE02 query privacy handoff

State: **MOCK_READY**. This provides headless QueryClient integration, not FE09
session UI, a server authorization policy or an accepted real multiuser flow.

## Candidate and ownership

- Worktree/branch: `mcr-worktrees/fe-transport`, `codex/mcr-fe-transport`.
- Base: `d76cbb5ca7ff419cf8c4e6ac1d587e4c69c42644` (FE01 handoff).
- Implementation candidate: `f12e476a902ee946ea149bbb47dd58083b3e2a53`.
- Ancestry: base → FE02 implementation → documentation-only handoff. Identify the
  final handoff SHA with `git log -1 --format=%H -- docs/mcr/frontend/query-privacy.md`.
- Changed paths: `frontend/src/shared/query/keys.ts`,
  `frontend/src/shared/query/privacy.ts`,
  `frontend/src/shared/query/__tests__/keys.test.ts`,
  `frontend/src/shared/query/__tests__/privacy.test.ts`, and this document.
- Newly authored; no copied Bigyeting module, legacy identity/provider scope or
  headless extraction is introduced. No FE01 edits or requested FE01 fixes.
- Contract provenance: C03 email/core v0.1.0 at
  `2d9198a684ec363c476ae75835ddf4289e47762c`; consumed FE01 SessionPort/SessionScope
  at the worker base. C03/C06 UUID user/event/place IDs are used as identifiers,
  not synthetic asset item IDs or evidence of physical arrival. C04 feature DTOs,
  grants, cursors and privacy-event wire frames are not invented by this patch.
- Runtime dependency: retained `@tanstack/react-query` and `@tanstack/query-core`
  **5.101.2**; no new dependency or manifest/lock edit. Native Query, QueryObserver,
  MutationObserver and QueryClient behavior was inspected in the installed pinned
  source. All private data remains in the existing QueryClient, not another cache.
- Source images, catalog revision 3, original bytes, source contracts, backend,
  routers, configuration, styles and the integration checkout were not modified.
  This task is headless and adds no visual decoration.

## Keys and classification

`privateQueryKey(scope,resource,entityIds?)` produces:

```text
['mcr','private',userId,eventId,sessionGeneration,policyGeneration,resource,...UUIDs]
```

Scope comes from canonical Auth identity and FE01 generation; event is nullable
for account-level work before event selection. An event change still requires
SessionPort.replace and therefore a new session generation. Keys are frozen,
contain no bearer/provider IDs/handles/DTOs/drafts, reject noncanonical identifiers,
unknown resource names and unsafe generation values. Profile/place/reveal keys
require exactly one UUID target; self/collection keys accept none, preventing
target omission from silently deduplicating two different resource reads.
Only currently required resource labels are registered. Future C04 filters/cursors
need an explicit extension; arbitrary payload objects must not become key parts.

`publicQueryKey()` is exactly `['mcr','public','auth-capabilities']`. That is the
only current public cache classification. A public-profile DTO is still an
authenticated, policy-dependent read and belongs in private scope. Everything
unclassified is conservatively purged on privacy changes. Private/policy-dependent
projections must never be placed under the capabilities key.

Every policy change advances a separate policy generation, including consent
revocation/expiry, handle replacement, block/unblock revalidation, suspension and
event closure. Reveal and non-reveal projections use that generation; a revoked
handle cannot survive as a stale cached profile, feed projection or reveal entry.
The approach deliberately purges all private projections rather than guessing
which C04 resources depend on a relationship that has changed.

## Coordinator ports and read integration

Create one `createQueryPrivacy({client,session})` per active shell. Use the same
SessionPort and actual QueryClient as the transport/application. The coordinator
listens to the current generation AbortSignal and reattaches after identity
replacement. Same-identity `commitRefresh` does not abort it, change key generation,
purge cache or notify owners. No token is copied into the coordinator's scopes.

`capture()` returns a frozen token-free scope with gate and policy generation.
`isCurrent(scope)` requires current identity/generation/policy, an open gate and
an authenticated user. `options({resource,entityIds?,read})` creates native TanStack
query options; the read receives `{scope,signal}` and returns validated domain data.

```ts
const options = privacy.options({
  resource: 'participant',
  read: async ({ signal }) => {
    const result = await http.request({
      path: `/api/concert/events/${eventId}/me`, method: 'GET',
      access: 'protected', errorFamily: 'core', signal,
      decode: participantDecoder,
    })
    return result.data
  },
})
// Feed these intact to useQuery, QueryObserver or client.fetchQuery.
```

Feature decoders validate full C03 DTOs and requested identity/event/target context;
the privacy layer checks scope, not the content of arbitrary domain objects.
Identity can change even between HTTP completion and query completion, so the
query wrapper performs its own scope checks and cancellation.

Reads disable TanStack retries to preserve FE01's sole authorized HTTP retry
policy. They have no placeholder/initial data carryover, zero stale/GC times,
no focus/reconnect/interval refetch, and explicit observer data-change notification.
A client-wide per-query persister is overridden by a memory-only direct queryFn
call. Queries must use the standard key hash and these options intact; do not
replace the guard, commit fence, persister, retry or placeholder settings.

`social-reveal` options are disabled on mount and do not automatically reveal
again on rebind. Fetch/reveal only after an explicit authorized action and current
grant/expiry checks. `fetchQuery` or `observer.refetch` is an explicit request and
is not prevented by enabled:false. A new policy generation is not a new consent
grant. FE31 must supply actual grant/expiry handling from its frozen contract.

`run(scope,work,signal?)` is the small generic guard used by the queryFn and
available to mutations/other async owners. It checks before and after work,
composes query/policy/session cancellation, detaches listeners on completion and
rejects promptly even if work ignores AbortSignal. Original safe adapter errors
propagate while current; privacy failures use a bounded PrivacyError without raw
payloads. Direct reads must not have cache/UI side effects before the guard returns.

TanStack commits after its queryFn promise settles. The options' structuralSharing
callback therefore performs a final synchronous scope check at Query.setData,
closing the microtask gap between successful queryFn settlement and cache commit.
This is a scope fence, not a second data cache or a schema decoder.

## Privacy changes, active observers and recovery

`changePolicy(expectedScope,reason)` binds revocation to the canonical session.
Messages from a different user/event/session generation return false without
purging the new user's data. Successive revocations from one live session remain
valid after earlier policy-generation bumps, so a stream cannot silently lose its
second revocation. Reopening with revalidated requires a full current-policy CAS;
a stale recovery result cannot undo a newer revocation. Server/WebSocket consumers
must retain their originating session scope when delivering such messages. Accepted reasons are block, consent-revoked,
consent-expired, handle-replaced, suspended, event-closed and revalidated.

A change first invalidates old leases and aborts work, then cancels private queries
without reverting to previous data, runs owner cleanup, scrubs active observer
results/errors, removes active and inactive private cache entries, removes cached
mutations and finally notifies subscribers. Public capabilities survive. Unknown
query keys are private by default. A throwing cleanup/observer is isolated; other
owners still clear their state and cleanup failures are reported as a count,
without retaining or logging underlying errors.

`removeQueries` alone is insufficient: an active observer can retain its previous
result. The scrub is intentionally separate. `resetQueries` is not used because
it can restore initial data and automatically refetch old scoped options. An old
observer's later refetch cannot dispatch private work or return old data; native
TanStack may recreate an empty/error query entry for that obsolete key. Rebind to
fresh options or unmount the old observer; do not use keepPreviousData across scopes.
Subscriber callbacks run after eviction and can create current options to refetch
normal authorized reads. Reveals remain explicit. Call capture() when reacting to
an event; another synchronous subscriber could already have superseded its scope.

Suspension/event closure closes the private-read gate. FE09/feature owners must
unmount private consumers until authoritative revalidation permits reads and
calls changePolicy(currentScope,'revalidated'). That operation advances generation
again, purges prior representations and opens the local read gate. Bootstrap/
policy recovery may use an explicit validated HTTP call outside private caching
while the gate is closed. This does not override server authorization, allow event
writes or authorize a cached reveal. Read-only/archive display remains a separate
capability decision after server revalidation.

## Mutation, draft and subscription ownership

`registerCleanup(callback)` registers a synchronous owner callback and returns an
unsubscribe function. Clear sensitive drafts, reset MutationObserver instances,
abort feature controllers, clear optimistic/local state and close private sockets/
subscriptions here. Callback snapshots prevent new registrations from running
inside the same delivery. `subscribe(callback)` is for subsequent rebind/unmount
work and also returns an unsubscribe function. Release both on owner disposal.

A safe mutation integration uses a captured scope and native MutationObserver:

```ts
const scope = privacy.capture()
const observer = new MutationObserver(client, {
  mutationKey: privateQueryKey({ ...scope, userId }, 'participant'),
  retry: false,
  mutationFn: input => privacy.run(scope, ({ signal }) => updateParticipant(input, signal)),
  onSuccess: data => {
    if (!privacy.isCurrent(scope)) return
    // Synchronous scoped cache/UI update; never compute the key from a later user.
    client.setQueryData(participantKeyCapturedForThisScope, data)
  },
})
const unregister = privacy.registerCleanup(() => {
  observer.reset()
  clearPrivateDraft()
  closePrivateSubscription()
})
```

Guard onError/onSettled/optimistic rollback callbacks too; a stale failure must not
restore account A's draft into account B's UI. Avoid awaits between the current
scope check and a local write, or check again afterward. Native global mutation
callbacks must obey the same rule. The generic run guard is not a lock around
TanStack's later mutation callbacks; success callback guards remain required.

Removing a mutation from MutationCache does **not** cancel it, suppress its
callbacks, reset its observer or roll back the server. A deliberate test records
this limitation. Work that ignores abort is fenced locally through run; the owner
still must reset/clear its UI and treat a dispatched mutation as potentially
completed. Do not automatically retry a cancelled or uncertain consent/stock/
profile write. FE01 uncertainty and operation-specific receipt/state reconciliation
remain authoritative; FE02 PrivacyError is not proof that the server did nothing.
No cross-user draft transfer, offline mutation queue or persistence is provided.

## Memory lifetime and persistence boundary

`dispose()` is idempotent: it removes the generation listener, rejects pending
work, scrubs/evicts private state, runs final cleanup, notifies then releases owners.
Disposed coordinators cannot register work or respond to a later shell's session
changes. Query/policy listeners and per-run abort listeners are explicitly disposed.
The patch has no localStorage/sessionStorage/IndexedDB use or private module-global
state and does not add a persistence plugin.

Exported `publicOnlyDehydrateOptions` must be passed to any future dehydration or
persistence boundary. It admits only successful exact public capabilities entries
and excludes every mutation, pending query and unclassified/private entry. Default
TanStack dehydration includes successful private queries and is **not safe** for
this application. Do not hydrate old private snapshots or attach global cache
subscribers that persist/log private data. This helper cannot police arbitrary
external writes, browser extensions, screenshots, user-held references or a
caller deliberately bypassing the ports.

## Evidence and limits

Toolchain: Node 24.22.2 via
`/Users/lhs/Desktop/mcr-tools/node-v24.22.2-darwin-arm64/bin`, pnpm 11.7.0, retained
TypeScript 6.0.3, Vitest 4.1.9, Vite 8.1.3 and Oxlint 1.72.0.

| Command | Exit | Evidence |
|---|---:|---|
| `corepack pnpm@11.7.0 --dir frontend test src/shared/query/__tests__` | 0 | 45 tests in two suites using native QueryClient/QueryObserver/MutationObserver and FE01 SessionPort; one mock-fetch HTTP composition. |
| `corepack pnpm@11.7.0 --dir frontend build` | 0 | Strict TypeScript plus Vite production build; modules are not wired to product UI yet. |
| `corepack pnpm@11.7.0 --dir frontend exec oxlint src/shared/query` | 0 | Assigned query/test paths; zero final warnings. |
| `python3 scripts/mcr/check-plan.py` | 0 | Preserved planning/source baseline, not runtime acceptance. |
| `node scripts/mcr/check-ui-origin.mjs --root <worktree>` | 0 | Static origin/import guard; no extracted legacy code. |
| `python3 scripts/mcr/check-boundaries.py --base d76cbb5ca7ff419cf8c4e6ac1d587e4c69c42644 --lane frontend --allow-doc docs/mcr/frontend/query-privacy.md` | 0 | Exact lane/doc assignment; changed paths and imports inspected. |
| `git diff --cached --check` | 0 | Exact staged patch before commits. |

Races cover ignored cancellation, logout/account/event switches, active/inactive
cache and observer scrubbing, obsolete observer refetch, fresh option rebinding,
post-queryFn/pre-commit revocation, all six privacy revocation/closure reasons,
same-identity refresh, stale policy events, mutation/draft cleanup, late mutation
results, deliberately unguarded mutation behavior, failing cleanup/observers,
per-query persister/default bypass prevention, public-only dehydration, anonymous
work rejection, safe keys and listener disposal. Intermediate strict-type errors
in test helpers and intentional sparse-array lint were corrected; final commands
are the acceptance evidence, not those earlier attempts.

No server privacy stream, real consent grant/reveal, expiry timer wiring, React
shell, cross-tab coordination, browser storage plugin, database or multiuser flow
was exercised. C04/C05 event adapters, FE09 owner wiring, FE31 grant expiry/reveal
handling, UI subscriptions/drafts, browser acceptance and IN03 remain **NOT_RUN**.
An observed revocation fences subsequent local results; it cannot erase information
already seen or copied or compensate for a missing/delayed server policy event.
