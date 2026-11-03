# Community shell composition review

Date: 2026-11-03. Disposition: **scoped clear; no P1/P2 findings**.
Composition remains **MOCK_READY**. This report does not grant whole-task or release acceptance.

## Assigned scope and revisions

- Writable checkout: `/Users/lhs/Desktop/mcr-worktrees/fe-community-review`.
- Branch: `codex/mcr-fe-community-review`.
- Sole delivered path: `docs/mcr/qa/community-shell-review.md`.
- Initial range: `9921759c8769323500dce4317d0d22a53fe69749..5fba2426c7166c9978227f03b6a1a5aed06054d8`.
- Assignment base and independently executed probe candidate: **`5fba2426c7166c9978227f03b6a1a5aed06054d8`**.
- Added parent delta, reviewed read-only: `5fba2426c7166c9978227f03b6a1a5aed06054d8..366cf8bb808d8a0d55124e0a73429a2f742d0fe1`.
- Party-composition candidate: **`366cf8bb808d8a0d55124e0a73429a2f742d0fe1`**.

Initial review covered `CommunityFeatures.tsx`, `WorldShell.tsx`, `WorldHost.tsx`,
`access.tsx`, `router.tsx`, composition tests and the native fixture/runner.
The additional assignment covered the three-file party composition delta in
`CommunityFeatures.tsx`, `router.tsx` and `__tests__/community.test.tsx`.
The party controller's construction/start/reset/disposal and panel hooks were
read for compatibility, together with the updated native runner and fixture.
This is not a fresh general review of FE21's contract, command engine or UI.

The added delta was read from `/Users/lhs/Desktop/mcr` using absolute paths and
immutable Git revisions. That checkout was clean when inspected. No checkout,
cherry-pick, build, test, install or write was performed there. The report branch
remains based on the initial candidate; the party delta was not copied into it.

## Review conclusions

| Concern | Evidence and conclusion |
| --- | --- |
| Bootstrap/privacy binding | `BootstrapBoundary` captures before dispatch, executes through `privacy.run`, and retains that captured scope with the frozen result. Policy generation also participates in the scoped-load attempt key, preventing a final state-commit race from restamping an old capability decision. Event, participant and all capability decisions are copied/frozen. |
| Disposed/suspended access | Disposed privacy rejects dispatch and late bootstrap completion. Disposed subscriptions reach the sanitized route fallback. The existing suspended-access test retries without dispatching or reopening the gate; source review confirms retry only changes the attempt counter. |
| Allocation reentrancy | World privacy cleanup is registered before `mount`. If allocation synchronously revokes policy, the returned lease is disposed and cleared before either initialization setter runs. This passed with and without StrictMode. |
| Owner count and navigation | Effect allocation and cleanup leave one live world owner. Public exit releases the world and gift cleanup owners; the public SignIn cleanup is separately accounted for. Re-entry uses the same canonical Auth/session and fresh feature data. |
| Late responses | Abort-resistant bootstrap and gift completions cannot populate the disposed or exited shell. The completed-read/queued-policy probe additionally covers the interval after privacy-read settlement and before shell state commit. |
| Party delta | The new controller receives the same exact lease, bootstrap capabilities and event phase as gifts. Both controllers are disposed together. Party construction registers cleanup/access listeners but dispatch starts in `start`, invoked by a mounted party panel. The shell retains controller identity across Meet/Gifts/World navigation. |
| Party command preservation | The added parent test asserts one POST despite duplicate clicks, 202 PENDING retention through tab changes, status lookup with the original key, activity reconciliation, then fresh READ_ONLY bootstrap and cancellation using `expected_revision` with an empty body. The native runner exercises pending-command recovery at both widths. These are parent-executed checks; this reviewer did not rerun them. |
| Public bundle and boundaries | Public/admin routes remain outside `WorldBoundary`; community imports remain lazy. No backend implementation, server configuration or secret import was introduced by either composition delta. The retained production-login browser evidence lists only entry/PublicBoundary scripts. |

No concrete P1/P2 defect was identified in the assigned changes. This is a
bounded review conclusion, not evidence that every future world adapter or
unreviewed domain flow is correct.

## Independently executed checks

Runtime: retained Node **24.22.2**, pnpm **11.7.0**, Vitest **4.1.9**, jsdom.
Version commands exited **0**. Dependencies came from an offline frozen-lockfile
preparation in the assigned checkout; retained manifest/lockfile bytes are unchanged.

The temporary probe below was executed only against the initial candidate.
Exact final command, from the assigned checkout:

```sh
PATH=/Users/lhs/Desktop/mcr-tools/node-v24.22.2-darwin-arm64/bin:$PATH /Users/lhs/Desktop/mcr-tools/node-v24.22.2-darwin-arm64/bin/node /Users/lhs/Desktop/mcr-tools/pnpm-11.7.0/package/bin/pnpm.cjs --dir /Users/lhs/Desktop/mcr-worktrees/fe-community-review/frontend test src/app/__tests__/community-review-probe.test.tsx
```

Exit **0**, **7/7 tests**, one file. Cases:

1. Synchronous consent revocation inside world allocation, ordinary mode: returned lease disposed exactly once, no setters, no gift dispatch.
2. The same allocation revocation under StrictMode: no duplicate live resource.
3. Privacy already disposed before bootstrap: no bootstrap dispatch, request or world mount; sanitized fallback.
4. Disposal while an abort-resistant bootstrap is pending: signal aborted, late result rejected, no new resource.
5. Policy queued after a completed privacy read but before bootstrap state commit: old snapshot never opens the shell; fresh disabled gift capability causes no gift request.
6. Mutation of a port's returned event/capability object after settlement: shell keeps the reviewed immutable snapshot through navigation.
7. StrictMode private-to-public-to-private navigation: old owner cleanups removed, pending request aborted, canonical identity retained, ignored late private payload, fresh request and peak one world/connection on re-entry; final cleanups balanced.

Initial harness assertions were corrected before the final pass: the fixture's
capabilities were already frozen, so the mutation probe now returns a mutable
clone; SignIn intentionally registers its own cleanup, so public exit verifies
removal of the two former private owners rather than a zero global count.
No application correction was required. The final seven-test source is retained
below for reproduction, and its temporary `.test.tsx` file was removed before handoff.

Independent SHA-256 checks exited **0** and matched **8/8** source entries in the
initial candidate's `docs/mcr/evidence/community-shell/report.json`, then **9/9**
entries in the party candidate's report, using `git show` at the exact latter
commit. Both reports identify Node 24.22.2 and Chrome 156.0.8078.80, with PASS at
390×844 and 1280×900. Hash agreement binds those reports to the inspected source
bytes; it is not an independent re-execution of the native runs.

Parent evidence inspected: `docs/mcr/evidence/community-shell-integration.md`,
its browser `report.json`, and the source of the composition/native assertions.
The original 105 focused tests/build/lint and subsequent 72 affected
tests/build/lint are attributed to the parent. Unchanged broad suites, builds,
lint and browser runs were not repeated by this reviewer.

Final delivery checks (assigned checkout):

```sh
python3 scripts/mcr/check-boundaries.py --base 5fba2426c7166c9978227f03b6a1a5aed06054d8 --lane frontend --allow-doc docs/mcr/qa/community-shell-review.md
git diff --cached --check
```

Both exited **0**. Manual path review confirms this Markdown report is the only
tracked change. Source archive, manifests, lockfile, contracts and application
source are unchanged. The report commit descends directly from the assignment
base; its exact SHA is supplied with the handoff. No push was performed.

## Limits and correction ownership

The seven new probes use jsdom, deterministic HTTP fixtures and an instrumented
world lease, not a real Phaser/WebSocket/backend deployment. The party delta
received source/evidence review only; no new runtime claim is made for that
candidate. Native reports are parent evidence, not reviewer-executed browser or
physical-device runs. Backend authorization, party/giveaway commands against a
real service, multiplayer, approved character artwork and release gates remain
outside this review. The existing large-chunk warnings remain release work.

The original FE24 bounded-delivery goal was completed only after its explicit
parent acceptance. The separate shell-review goal remains **ACTIVE through
correction review**. After committing this report the writer stops; no subsequent
application work or whole-task acceptance is inferred.

## Reproducible temporary probe

Save this block as `frontend/src/app/__tests__/community-review-probe.test.tsx`
in a disposable checkout at the initial candidate, run the pinned command above,
and remove the temporary file afterward. This block is the exact final passing
probe, SHA-256 `ece6aa9cd716d973e8892a1756faebf09589d8a8160eb29dbb0533b04f434dde`.

```tsx
// @vitest-environment jsdom
import '@testing-library/jest-dom/vitest'
import { StrictMode } from 'react'
import { act, cleanup, render, screen, waitFor } from '@testing-library/react'
import { afterEach, expect, it, vi } from 'vitest'
import { createMemoryRouter, RouterProvider } from 'react-router-dom'
import { createAppRoutes } from '../router'
import type { ActiveScope, ShellBootstrap, ShellPorts, WorldLifecyclePort } from '../ports'
import { bootstrapFor, createFixturePorts } from './fixtures/ports'
import { deferred, fixture, giveaway } from '../../features/gift-discovery/__tests__/fixtures/data'

const releases: (() => void)[] = []
afterEach(() => { cleanup(); releases.splice(0).forEach(release => release()); vi.restoreAllMocks() })
function setup(options: {
  strict?: boolean; before?: (f: ReturnType<typeof fixture>) => void;
  bootstrap?: (scope: ActiveScope, signal: AbortSignal) => Promise<ShellBootstrap>;
  mount?: (f: ReturnType<typeof fixture>, original: WorldLifecyclePort['mount']) => WorldLifecyclePort['mount'];
  handler?: (request: Request) => Response | Promise<Response>;
} = {}) {
  const f = fixture(options.handler ? { handler: options.handler } : {}), world = createFixturePorts()
  const activeCleanups = new Set<unknown>()
  const register = f.auth.privacy.registerCleanup.bind(f.auth.privacy)
  vi.spyOn(f.auth.privacy, 'registerCleanup').mockImplementation(callback => {
    const remove = register(callback); activeCleanups.add(callback)
    return () => { activeCleanups.delete(callback); remove() }
  })
  options.before?.(f)
  const mount = options.mount?.(f, world.ports.world!.mount) ?? world.ports.world!.mount
  const load = vi.fn(options.bootstrap ?? (async (scope: ActiveScope) => bootstrapFor(scope)))
  const ports: ShellPorts = { ...world.ports, auth: f.auth, session: f.auth.session, world: { mount }, bootstrap: { load } }
  const router = createMemoryRouter(createAppRoutes(ports), { initialEntries: ['/gifts'] })
  releases.push(() => { router.dispose(); f.dispose() })
  const element = <RouterProvider router={router} />
  return { ...f, ...render(options.strict ? <StrictMode>{element}</StrictMode> : element), router, load, stats: world.stats, activeCleanups }
}

it.each([false, true])('releases a lease returned after synchronous policy revocation inside mount (StrictMode=%s)', async strict => {
  const next = deferred<ShellBootstrap>(), input = vi.fn(), visible = vi.fn()
  let reads = 0
  const f = setup({ strict, bootstrap: async scope => ++reads === 1 ? bootstrapFor(scope) : next.promise,
    mount: (f, mount) => (host, context) => {
      const lease = mount(host, context)
      f.auth.privacy.changePolicy(f.auth.privacy.capture(), 'consent-revoked')
      expect(context.signal.aborted).toBe(true)
      return { dispose: lease.dispose, setInputBlocked: input, setVisible: visible }
    } })
  await waitFor(() => expect(f.load).toHaveBeenCalledTimes(2))
  expect(f.stats).toMatchObject({ mounts: 1, disposals: 1, games: 0, connections: 0, peakGames: 1 })
  expect(input).not.toHaveBeenCalled(); expect(visible).not.toHaveBeenCalled()
  expect(f.requests).toHaveLength(0)
  expect(screen.queryByRole('link', { name: 'Fan badges' })).not.toBeInTheDocument()
  expect(screen.getByRole('heading', { name: 'Opening your event' })).toBeVisible()
  expect(document.querySelector('[data-world-host]')).toBeNull()
})

it('does not dispatch or mount when privacy was disposed before the first bootstrap', async () => {
  vi.spyOn(console, 'error').mockImplementation(() => {})
  const f = setup({ before: f => f.auth.privacy.dispose() })
  await screen.findByText('This page is unavailable')
  expect(f.load).not.toHaveBeenCalled(); expect(f.requests).toHaveLength(0)
  expect(f.stats.mounts).toBe(0)
  expect(screen.queryByRole('link', { name: 'Fan badges' })).not.toBeInTheDocument()
})

it('fences an abort-resistant bootstrap after privacy disposal', async () => {
  vi.spyOn(console, 'error').mockImplementation(() => {})
  const pending = deferred<ShellBootstrap>()
  const f = setup({ bootstrap: () => pending.promise })
  await waitFor(() => expect(f.load).toHaveBeenCalledTimes(1))
  act(() => f.auth.privacy.dispose())
  expect(f.load.mock.calls[0]![1].aborted).toBe(true)
  await act(async () => { pending.resolve(bootstrapFor(f.load.mock.calls[0]![0])); await pending.promise })
  await screen.findByText('This page is unavailable')
  expect(f.load).toHaveBeenCalledTimes(1); expect(f.requests).toHaveLength(0); expect(f.stats.mounts).toBe(0)
})

it('rejects a completed privacy read when policy changes before bootstrap state can commit', async () => {
  const next = deferred<ShellBootstrap>()
  let reads = 0
  const f = setup({ before: f => {
    const run = f.auth.privacy.run.bind(f.auth.privacy)
    let bumped = false
    vi.spyOn(f.auth.privacy, 'run').mockImplementation(async (scope, work, signal) => {
      const value = await run(scope, work, signal)
      if (!bumped) {
        bumped = true
        queueMicrotask(() => f.auth.privacy.changePolicy(scope, 'block'))
      }
      return value
    })
  }, bootstrap: async scope => ++reads === 1 ? bootstrapFor(scope) : next.promise })
  await waitFor(() => expect(f.load).toHaveBeenCalledTimes(2))
  expect(f.requests).toHaveLength(0); expect(f.stats.mounts).toBe(0)
  expect(screen.getByRole('heading', { name: 'Opening your event' })).toBeVisible()
  const fresh = bootstrapFor(f.load.mock.calls[1]![0])
  next.resolve({ ...fresh, capabilities: { ...fresh.capabilities, gifts: { available: false, reason: 'FEATURE_DISABLED' } } })
  expect(await screen.findByRole('heading', { name: 'Gifts are unavailable' })).toBeVisible()
  expect(f.requests).toHaveLength(0)
  expect(f.stats).toMatchObject({ mounts: 1, games: 1, peakGames: 1 })
})

it('owns an immutable bootstrap snapshot after a port mutates its returned capabilities and event', async () => {
  let original: ShellBootstrap | null = null
  const f = setup({ bootstrap: async scope => { original = structuredClone(bootstrapFor(scope)); return original } })
  await screen.findByRole('link', { name: 'Fan badges' })
  const title = original!.event.title
  act(() => {
    Object.assign(original!.event, { title: 'Unreviewed mutated title' })
    Object.assign(original!.capabilities, { gifts: { available: false, reason: 'FEATURE_DISABLED' } })
  })
  await act(() => f.router.navigate('/world'))
  await act(() => f.router.navigate('/gifts'))
  expect(screen.getByText(title)).toBeVisible()
  expect(screen.queryByText('Unreviewed mutated title')).not.toBeInTheDocument()
  expect(await screen.findByRole('link', { name: 'Fan badges' })).toBeVisible()
  expect(f.load).toHaveBeenCalledTimes(1)
  expect(f.requests).toHaveLength(1)
})

it('releases private owners on public exit, keeps canonical Auth alive and rejects late data on re-entry', async () => {
  const pending = deferred<Response>()
  let reads = 0
  const f = setup({ strict: true, handler: () => ++reads === 1 ? pending.promise
    : Response.json({ items: [giveaway({ title: 'Fresh re-entry gifts' })], next_cursor: null }) })
  await waitFor(() => expect(f.requests).toHaveLength(1))
  const identity = f.auth.session.read(), session = f.auth.session
  expect(f.activeCleanups.size).toBe(2)
  const privateOwners = [...f.activeCleanups]
  await act(() => f.router.navigate('/login'))
  await screen.findByRole('heading', { name: 'Sign in' })
  expect(f.requests[0]!.signal.aborted).toBe(true)
  expect(f.stats).toMatchObject({ games: 0, connections: 0 })
  expect(privateOwners.every(owner => !f.activeCleanups.has(owner))).toBe(true)
  expect(f.activeCleanups.size).toBe(1) // SignIn registers its own draft cleanup.
  expect(f.auth.session).toBe(session); expect(session.read()).toBe(identity)
  expect(f.auth.privacy.capture().gate).toBe('open')
  await act(async () => { pending.resolve(Response.json({ items: [giveaway({ title: 'Obsolete private payload' })], next_cursor: null })); await pending.promise })
  expect(screen.queryByText('Obsolete private payload')).not.toBeInTheDocument()
  await act(() => f.router.navigate('/gifts'))
  expect(await screen.findByRole('link', { name: 'Fresh re-entry gifts' })).toBeVisible()
  expect(screen.queryByText('Obsolete private payload')).not.toBeInTheDocument()
  expect(f.activeCleanups.size).toBe(2)
  expect(f.requests).toHaveLength(2)
  expect(f.stats).toMatchObject({ games: 1, connections: 1, peakGames: 1, peakConnections: 1 })
  f.unmount()
  expect(f.activeCleanups.size).toBe(0)
  expect(f.stats.mounts).toBe(f.stats.disposals)
})
```
