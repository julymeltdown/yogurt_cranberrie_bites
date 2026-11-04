// @vitest-environment jsdom
import { afterEach, expect, it, vi } from 'vitest'
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import '@testing-library/jest-dom/vitest'
import { createMemoryRouter, RouterProvider } from 'react-router-dom'
import { createSessionController } from '../../../features/auth/model/session'
import { isolatedCookiePolicy, deferred } from '../../../features/auth/__tests__/fixtures/data'
import { createAuthorityTimeline } from '../../community-authority/timeline'
import { DOMAIN_OPERATIONS } from '../../../shared/api/domain-operations.generated'
import registration from '../../operations-entry/__tests__/operation.fixture.json'
import { createProductionPorts } from '../../ports'
import { createAppRoutes } from '../../router'
import { createDefaultAdmin } from '../owner'
import { createControlledServer, wire } from './server'
import { createFixturePorts } from '../../__tests__/fixtures/ports'

// Frozen 3a089a3b transport registration, TEST ONLY. Production generated registry remains unchanged.
vi.mock('../../../shared/api/domain-operations.generated', async original => {
  const actual = await original<typeof import('../../../shared/api/domain-operations.generated')>()
  const fixture = await import('../../operations-entry/__tests__/operation.fixture.json')
  return { ...actual, DOMAIN_OPERATIONS: { ...actual.DOMAIN_OPERATIONS, [fixture.default.operationId]: fixture.default.definition } }
})
const owners: { dispose(): void }[] = []
function setup() {
  const server = createControlledServer()
  const auth = createSessionController({ eventId: wire.event, cookiePolicy: isolatedCookiePolicy(), timeoutMs: 2000,
    fetch: input => server.handle(input as Request) })
  auth.session.replace(0, { userId: wire.actor, eventId: wire.event, accessToken: wire.auth.accessToken })
  let elapsed = 0, interrupt = () => {}
  const admin = createDefaultAdmin(auth, { timeline: signal => createAuthorityTimeline({ observe: () => ({ nowMs: elapsed, origin: 100 }), maxObservationGapMs: 120_000,
    onInterruption(invalidate) { interrupt = invalidate; signal.addEventListener('abort', invalidate, { once: true }); return () => signal.removeEventListener('abort', invalidate) } }) })
  const scope = { ...auth.session.read(), userId: wire.actor, eventId: wire.event }
  const entry = admin.open(scope)!
  const h = { auth, admin, server, scope, entry, advance(n: number) { elapsed = n }, interrupt: () => interrupt(), dispose() { admin.dispose(); auth.dispose() } }
  owners.push(h); return h
}
afterEach(() => { cleanup(); owners.splice(0).forEach(o => o.dispose()); vi.restoreAllMocks(); Reflect.set(DOMAIN_OPERATIONS, registration.operationId, registration.definition) })

it('joins independent role/event revisions through registered canonical settings and keeps one entry across panel borrowers', async () => {
  const h = setup(); await h.entry.start(); const state = h.entry.read()
  expect(state.phase).toBe('ready'); expect(state.decision?.roleRevision).toBe(7); expect(state.settings?.event.revision).toBe(8)
  expect(state.muralClosure?.closure.prepare('Reviewed event')).toBe(true)
  expect(state.muralClosure?.closure.read().draft?.expected_revision).toBe(8)
  expect(h.admin.open(h.scope)).toBe(h.entry); await h.entry.start(); expect(h.server.requests).toHaveLength(2)
  expect(h.server.requests.every(r => r.headers.get('Authorization') === `Bearer ${wire.auth.accessToken}`)).toBe(true)
  expect(h.server.requests.every(r => r.method === 'GET' && r.cache === 'no-store')).toBe(true)
})
it('does not grant controls or dispatch a fallback when actual registration is absent', async () => {
  Reflect.deleteProperty(DOMAIN_OPERATIONS, registration.operationId)
  const h = setup(); await h.entry.start(); expect(h.entry.read().phase).toBe('unavailable'); expect(h.server.requests).toHaveLength(0)
})
it.each(['mismatch', 'foreign', '403', '503'] as const)('rejects %s settings instead of deriving event context from role', async cause => {
  const h = setup()
  if (cause === 'mismatch') h.server.state.settingsOverride = { ...wire.settings, revision: 9 }
  if (cause === 'foreign') h.server.state.settingsOverride = { ...wire.settings, event: { ...wire.settings.event, event_id: wire.snapshot } }
  if (cause === '403' || cause === '503') h.server.state.settingsStatus = Number(cause)
  await h.entry.start(); expect(h.entry.read()).toMatchObject({ phase: 'unavailable', settings: null, operations: null, muralClosure: null })
})
it('does not turn MODERATOR into event/place/evidence authority', async () => {
  const h = setup(); h.server.state.role = 'MODERATOR'; await h.entry.start()
  expect(h.entry.read()).toMatchObject({ phase: 'unavailable', decision: { role: 'MODERATOR' }, operations: null, muralClosure: null })
  expect(h.server.requests).toHaveLength(1)
})
it.each(['READ_ONLY', 'ARCHIVED'])('retains authorized closure observation in %s without a participant write grant', async phase => {
  const h = setup(); h.server.state.settings.event.phase = phase; await h.entry.start()
  const closure = h.entry.read().muralClosure!; await closure.start('closure')
  expect(closure.isCurrent()).toBe(true); expect(closure.closure.prepare('No write')).toBe(false)
  expect(await closure.closure.refresh(wire.job)).toBe(true)
  expect(closure.closure.canResume()).toBe(phase === 'READ_ONLY')
  expect(h.auth.privacy.capture().gate).toBe('open')
})
it('publishes null before equal-field/new-decision replacement and disposes the old owners', async () => {
  const h = setup(); await h.entry.start(); const old = h.entry.read(); const observed: (string | null)[] = []
  h.entry.subscribe(() => observed.push(h.entry.read().decision?.decisionId ?? null)); h.server.replaceDecision()
  const refreshing = h.entry.refresh(); expect(h.entry.read().operations).toBeNull()
  expect(old.muralClosure?.isDisposed()).toBe(true); expect(old.operations?.read().access).toBe('withdrawn')
  await refreshing; expect(observed[0]).toBeNull(); expect(h.entry.read().decision?.decisionId).toBe(h.server.state.decision)
  expect(h.entry.read().operations).not.toBe(old.operations)
})
it('updates event context from authorized settings refresh without replacing the role or retained entry', async () => {
  const h = setup(); await h.entry.start(); const state = h.entry.read(), entry = state.muralClosure!
  await entry.start('closure')
  h.server.state.settings = { ...h.server.state.settings, revision: 9,
    event: { ...h.server.state.settings.event, revision: 9, phase: 'READ_ONLY' } }
  await state.operations!.loadEvent()
  expect(h.entry.read().muralClosure).toBe(entry); expect(h.entry.read().decision?.roleRevision).toBe(7)
  expect(h.entry.read().settings?.event).toMatchObject({ revision: 9, phase: 'READ_ONLY' })
  entry.closure.revalidate(); expect(await entry.closure.refresh(wire.job)).toBe(true); expect(entry.closure.canResume()).toBe(true)
  expect(state.operations!.mapRevision()).toBeNull()
})
it.each(['actor', 'event', 'policy', 'clock', 'expiry', 'dispose'] as const)('withdraws all retained children on %s and fences future commands', async cause => {
  const h = setup(); await h.entry.start(); const old = h.entry.read().muralClosure!; await old.start('mural'); await old.start('closure')
  if (cause === 'actor' || cause === 'event') h.auth.session.replace(h.auth.session.read().generation, { userId: cause === 'actor' ? wire.snapshot : wire.actor, eventId: cause === 'event' ? wire.snapshot : wire.event, accessToken: 'TEST_ONLY_replacement' })
  if (cause === 'policy') h.auth.privacy.changePolicy(h.auth.privacy.capture(), 'block')
  if (cause === 'clock') h.interrupt()
  if (cause === 'expiry') h.advance(60_000)
  if (cause === 'dispose') h.admin.dispose()
  expect(h.entry.read().operations).toBeNull(); expect(old.isDisposed()).toBe(true)
  expect(old.wall.read()).toMatchObject({ phase: 'disposed', mural: null }); expect(old.closure.read()).toMatchObject({ phase: 'disposed', job: null })
})
it('aborts pending settings and ignores its late completion after canonical policy replacement', async () => {
  const h = setup(), delayed = deferred<Response>()
  h.server.state.intercept = request => new URL(request.url).pathname.endsWith('/settings') ? delayed.promise : null
  const pending = h.entry.start(); await vi.waitFor(() => expect(h.server.requests).toHaveLength(2))
  h.auth.privacy.changePolicy(h.auth.privacy.capture(), 'block'); expect(h.server.requests[1]!.signal.aborted).toBe(true)
  delayed.resolve(Response.json(wire.settings)); await pending; expect(h.entry.read().phase).toBe('disposed')
})
it('mounts the actual default Admin route through production ports and retains an unknown command across navigation', async () => {
  const h = setup(); h.admin.dispose()
  // Production binding uses the real browser timeline; this controlled jsdom document is visible.
  const ports = createProductionPorts(h.auth); owners.push({ dispose: () => ports.dispose?.() })
  const stopReady = h.auth.readiness.subscribe(() => {}); stopReady()
  // Login through canonical Auth establishes readiness; no substitute shell/bootstrap authority.
  const ready = new Promise<void>(resolve => { const stop = h.auth.subscribe(() => { if (h.auth.read().capabilities !== 'loading') { stop(); resolve() } }) })
  h.auth.start(); await ready; expect(await h.auth.login('operator@example.com', 'TEST_ONLY_password')).toBe(true)
  const router = createMemoryRouter(createAppRoutes(ports), { initialEntries: ['/admin/mural'] }); owners.push({ dispose: () => router.dispose() })
  render(<RouterProvider router={router} />)
  await screen.findByRole('heading', { name: 'WALL CONTROL' }); await waitFor(() => expect(screen.getByRole('status', { name: 'Wall control status' })).toHaveTextContent('Server wall state loaded'))
  fireEvent.change(screen.getByLabelText('Reason', { exact: true }), { target: { value: 'Reviewed lost pause' } })
  fireEvent.click(screen.getByRole('button', { name: 'Review wall action' })); h.server.state.lost = true
  fireEvent.click(screen.getByRole('button', { name: 'Confirm wall action' }))
  await waitFor(() => expect(screen.getByRole('status', { name: 'Wall control status' })).toHaveTextContent('unresolved'))
  const command = ports.admin.open!(h.auth.session.read() as typeof h.scope)!.read().muralClosure!.wall.read().command!
  fireEvent.click(screen.getByRole('link', { name: 'Event' })); await screen.findByRole('heading', { name: 'Event settings' })
  fireEvent.click(screen.getByRole('link', { name: 'Places' })); await screen.findByRole('heading', { name: 'Venue and places' })
  fireEvent.click(screen.getByRole('link', { name: 'Closure' })); await screen.findByRole('heading', { name: 'CLOSE THE EVENT' })
  fireEvent.click(screen.getByRole('link', { name: 'Wall' })); await screen.findByRole('button', { name: 'Check original wall command' })
  expect(ports.admin.open!(h.auth.session.read() as typeof h.scope)!.read().muralClosure!.wall.read().command?.key).toBe(command.key)
  expect(h.server.requests.filter(r => r.method === 'POST' && new URL(r.url).pathname.endsWith('/control'))).toHaveLength(1)
  h.server.state.role = null; fireEvent.click(screen.getByRole('button', { name: 'Check current access' }))
  await screen.findByRole('heading', { name: 'Administration unavailable' }); expect(screen.queryByRole('heading', { name: 'WALL CONTROL' })).toBeNull()
  h.auth.privacy.changePolicy(h.auth.privacy.capture(), 'suspended')
  fireEvent.click(screen.getByRole('button', { name: 'Check current access' }))
  await screen.findByRole('heading', { name: 'Administration unavailable' })
  expect(screen.getByRole('button', { name: 'Check current access' })).toBeEnabled()
})
it.each([false, true])('keeps legacy boolean authorization=%s compatible without granting a control factory', async value => {
  const fixture = createFixturePorts(), authorize = vi.fn(async () => value)
  const router = createMemoryRouter(createAppRoutes({ ...fixture.ports, admin: { authorize } }), { initialEntries: ['/admin'] })
  owners.push({ dispose: () => router.dispose() }); render(<RouterProvider router={router} />)
  await screen.findByRole('heading', { name: value ? 'Control room is not available yet' : 'Access denied' })
  expect(authorize).toHaveBeenCalledOnce(); expect(screen.queryByRole('heading', { name: 'WALL CONTROL' })).toBeNull()
})
