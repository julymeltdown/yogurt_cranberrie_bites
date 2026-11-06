// @vitest-environment jsdom
import { afterEach, expect, it } from 'vitest'
import { act, cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import { createMemoryRouter, RouterProvider } from 'react-router-dom'
import { PortsProvider } from '../../../access'
import { deferred } from '../../../../features/auth/__tests__/fixtures/data'
import { validDraft, uid } from '../../../../features/admin/service-policy/__tests__/fixture'
import { DefaultAdminPanel } from '../../panel'
import { compositionFixture, type CompositionFixture } from './fixture'

const owners: { dispose(): void }[] = []
function setup() { const h = compositionFixture(); owners.push(h); return h }
afterEach(() => { cleanup(); owners.splice(0).forEach(owner => owner.dispose()) })
function mount(h: CompositionFixture, path = '/admin/event') {
  const router = createMemoryRouter(['/admin', '/admin/event', '/admin/reports', '/admin/places'].map(route => ({ path: route,
    element: <PortsProvider ports={h.ports}><DefaultAdminPanel key={route} scope={h.scope()} /></PortsProvider> })), { initialEntries: [path] })
  owners.push({ dispose: () => router.dispose() }); render(<RouterProvider router={router} />); return router
}

it('creates one service-policy child only after current operator settings and shares the retained Admin lifetime', async () => {
  const h = setup(); await h.initialize()
  const entry = h.entry()!, state = entry.read(), child = state.servicePolicy!
  expect(state.phase).toBe('ready'); expect(child.read().access).toBe('ready')
  expect(state.reports?.isCurrent()).toBe(true)
  expect(h.admin.open(h.scope())).toBe(entry); await entry.start()
  await child.refresh(); child.edit(validDraft); expect(child.review()).toBe(true)
  expect(h.policy.requests.filter(request => request.path.endsWith('/operations-access'))).toHaveLength(1)
  expect(h.policy.requests.filter(request => request.path.endsWith('/admin/settings'))).toHaveLength(1)
  expect(child.read().command?.input.expected_policy_revision).toBe(4)
  expect(h.policy.requests.filter(request => request.method === 'PUT')).toHaveLength(0)
})
it('leaves policy null during a delayed settings join and ignores a late result after withdrawal', async () => {
  const h = setup(), response = deferred<Response>()
  h.intercept(request => new URL(request.url).pathname.endsWith('/admin/settings') ? response.promise : null)
  const pending = h.initialize()
  await waitFor(() => expect(h.policy.requests.some(request => request.path.endsWith('/admin/settings'))).toBe(true))
  const entry = h.entry()!
  expect(entry.read().servicePolicy).toBeNull(); expect(entry.read().reports?.isCurrent()).toBe(true)
  h.auth.privacy.changePolicy(h.auth.privacy.capture(), 'block')
  response.resolve(Response.json({})); await pending
  expect(entry.read()).toMatchObject({ phase: 'disposed', servicePolicy: null, operations: null, reports: null })
})
it.each([403, 503])('preserves the separate Reports child when operator settings fail with %s', async status => {
  const h = setup(); h.settingsStatus(status); await h.initialize()
  expect(h.state()).toMatchObject({ phase: 'unavailable', servicePolicy: null, operations: null })
  expect(h.state()?.reports?.isCurrent()).toBe(true)
  expect(h.policy.requests.some(request => request.path.endsWith('/service-policy'))).toBe(false)
})
it.each(['MODERATOR', null] as const)('does not infer service-policy operator authority from role %s', async role => {
  const h = setup(); h.role(role); await h.initialize()
  expect(h.state()?.servicePolicy).toBeNull()
  expect(h.policy.requests.some(request => request.path.endsWith('/admin/settings'))).toBe(false)
  expect(h.state()?.reports?.isCurrent() ?? false).toBe(role === 'MODERATOR')
})
it.each(['actor', 'event', 'privacy', 'clock', 'expiry', 'dispose'] as const)('publishes null and scrubs retained policy before callbacks on %s', async cause => {
  const h = setup(); await h.initialize(); const entry = h.entry()!, previous = entry.read(), child = previous.servicePolicy!
  await child.refresh(); child.edit(validDraft); expect(child.review()).toBe(true)
  const observations: unknown[] = []
  child.subscribe(() => observations.push(entry.read().servicePolicy))
  if (cause === 'actor' || cause === 'event') h.auth.session.replace(h.auth.session.read().generation, {
    userId: cause === 'actor' ? uid(999) : h.scope().userId, eventId: cause === 'event' ? uid(999) : h.scope().eventId, accessToken: 'TEST_ONLY_replacement',
  })
  if (cause === 'privacy') h.auth.privacy.changePolicy(h.auth.privacy.capture(), 'block')
  if (cause === 'clock') h.interrupt()
  if (cause === 'expiry') { h.advance(60_000); entry.read() }
  if (cause === 'dispose') h.admin.dispose()
  expect(entry.read().servicePolicy).toBeNull()
  expect(child.read()).toMatchObject({ access: 'withdrawn', decision: null, command: null, draft: { reason: '' } })
  expect(previous.reports?.isDisposed()).toBe(true)
  expect(observations.length).toBeGreaterThan(0); expect(observations.every(value => value === null)).toBe(true)
  await child.confirm(); expect(h.policy.requests.filter(request => request.method === 'PUT')).toHaveLength(0)
})
it('replaces the child on a new same-role decision and keeps Reports attached to the new decision', async () => {
  const h = setup(); await h.initialize(); const entry = h.entry()!, previous = entry.read()
  await entry.refresh(); const next = entry.read()
  expect(next.servicePolicy).not.toBe(previous.servicePolicy); expect(next.servicePolicy?.read().access).toBe('ready')
  expect(previous.servicePolicy?.read().access).toBe('withdrawn'); expect(previous.reports?.isDisposed()).toBe(true)
  expect(next.reports?.decision).toBe(next.decision)
})
it('mounts actual Event policy controls, retains an uncertain command through Reports navigation and recovers without replay', async () => {
  const h = setup(); await h.initialize(); const router = mount(h)
  const panel = await screen.findByRole('region', { name: 'Service policy' })
  const child = h.state()!.servicePolicy!
  await waitFor(() => expect(child.read().decision?.policy_revision).toBe(4))
  act(() => { child.edit(validDraft) }); fireEvent.click(within(panel).getByRole('button', { name: 'Review policy change' }))
  expect(document.activeElement).toBe(screen.getByRole('heading', { name: 'Confirm service policy change' }))
  h.policy.mode('lost'); fireEvent.click(screen.getByRole('button', { name: 'Confirm policy change' }))
  await waitFor(() => expect(child.read().command?.stage).toBe('unknown'))
  const key = child.read().command!.key
  fireEvent.click(screen.getByRole('link', { name: 'Reports' }))
  await waitFor(() => expect(router.state.location.pathname).toBe('/admin/reports'))
  expect(screen.queryByRole('heading', { name: 'Service policy' })).toBeNull()
  expect(h.state()?.reports?.borrow()).not.toBeNull(); expect(h.state()?.servicePolicy).toBe(child)
  fireEvent.click(screen.getByRole('link', { name: 'Event' }))
  await screen.findByRole('button', { name: 'Check policy command status' })
  expect(child.read().command?.key).toBe(key)
  fireEvent.click(screen.getByRole('button', { name: 'Check policy command status' }))
  await waitFor(() => expect(child.read().command?.stage).toBe('succeeded'))
  expect(h.policy.requests.filter(request => request.method === 'PUT')).toHaveLength(1)
  expect(child.read().command?.receipt?.resource_revision).toBe(22)
})
it('keeps policy off the Moderator Event panel while preserving Reports', async () => {
  const h = setup(); h.role('MODERATOR'); await h.initialize(); mount(h)
  await screen.findByRole('heading', { name: 'Administration unavailable' })
  expect(screen.queryByRole('heading', { name: 'Service policy' })).toBeNull()
  expect(h.state()?.reports?.isCurrent()).toBe(true)
})
it('keeps the retained policy child off the control-room summary', async () => {
  const h = setup(); await h.initialize(); mount(h, '/admin')
  await screen.findByRole('heading', { name: 'Control room' })
  expect(h.state()?.servicePolicy?.read().access).toBe('ready')
  expect(screen.queryByRole('heading', { name: 'Service policy' })).toBeNull()
  expect(h.policy.requests.some(request => request.path.endsWith('/service-policy'))).toBe(false)
})
