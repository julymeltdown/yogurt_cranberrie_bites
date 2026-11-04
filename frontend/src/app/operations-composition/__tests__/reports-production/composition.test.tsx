// @vitest-environment jsdom
import { afterEach, expect, it, vi } from 'vitest'
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import '@testing-library/jest-dom/vitest'
import { createMemoryRouter, RouterProvider } from 'react-router-dom'
import { deferred } from '../../../../features/auth/__tests__/fixtures/data'
import { createProductionPorts } from '../../../ports'
import { createAppRoutes } from '../../../router'
import { DOMAIN_OPERATIONS } from '../../../../shared/api/domain-operations.generated'
import { createReportEvidence, registeredReportEvidenceOperation } from '../../reports/evidence'
import { setup, review, owners } from './harness'
import { id, wire } from './server'

afterEach(() => { cleanup(); owners.splice(0).forEach(h => h.dispose()); vi.restoreAllMocks() })
it('uses the actual generated discovery and Reports operations without a test registration', async () => {
  expect(DOMAIN_OPERATIONS.getOwnConcertOperationsAccess.path).toContain('/me/operations-access')
  const h = setup(), controller = await review(h)
  expect(await h.admin.authorize(h.scope, new AbortController().signal)).toBe(true)
  expect(controller.read()).toMatchObject({ phase: 'ready', access: true, detail: { text: 'Private test statement' } })
  expect(h.entry.read()).toMatchObject({ decision: { role: 'MODERATOR', roleRevision: 7 }, operations: null, muralClosure: null, settings: null })
  expect(JSON.stringify(controller.read())).not.toContain('reporter_id'); expect(JSON.stringify(controller.read())).not.toContain(id(999))
  expect(h.server.requests.some(r => new URL(r.url).pathname.endsWith('/admin/settings'))).toBe(false)
  expect(h.server.requests.filter(r => new URL(r.url).pathname.includes('/admin/reports'))).toHaveLength(2)
  expect(h.server.requests.every(r => r.headers.get('Authorization') === `Bearer ${wire.auth.accessToken}` && r.cache === 'no-store')).toBe(true)
})
it.each([403, 503])('keeps Operator Reports usable when the independent settings join returns %s', async status => {
  const h = setup('OPERATOR'); h.server.state.settingsStatus = status
  const controller = await review(h)
  expect(controller.read().phase).toBe('ready'); expect(h.entry.read().operations).toBeNull()
  expect(h.entry.read().reports?.borrow()).toBe(controller)
  expect(await h.admin.authorize(h.scope, new AbortController().signal)).toBe(true)
})
it('rejects a real no-assignment response without requesting private records', async () => {
  const h = setup(null); await h.entry.start()
  expect(h.entry.read().reports).toBeNull(); expect(await h.admin.authorize(h.scope, new AbortController().signal)).toBe(false)
  expect(h.server.requests.some(r => new URL(r.url).pathname.includes('/admin/'))).toBe(false)
})
it('leaves only the unregistered protected byte path unavailable and retains the current detail', async () => {
  const h = setup(), controller = await review(h), count = h.server.requests.length
  expect(registeredReportEvidenceOperation()).toBeNull()
  await controller.evidence(id(501))
  expect(controller.read()).toMatchObject({ phase: 'ready', detail: { text: 'Private test statement' }, evidenceUrl: null })
  expect(controller.read().message).toContain('unavailable'); expect(h.server.requests).toHaveLength(count)
  controller.draft('RESOLVED', 'Reviewed factual reason'); controller.review(); expect(controller.read().confirm).toBe(true)
})
it('the prepared evidence adapter fails without dispatch when registration is absent', async () => {
  const h = setup(); await h.entry.start(); const count = h.server.requests.length
  const evidence = createReportEvidence({ auth: h.auth, scope: h.auth.privacy.capture(), signal: new AbortController().signal, current: () => true })
  await expect(evidence.open({ scope: h.auth.privacy.capture(), reportId: id(301), mediaId: id(501), signal: new AbortController().signal })).rejects.toThrow('producer unavailable')
  expect(h.server.requests).toHaveLength(count)
})
it.each(['extra', 'foreign-event', 'duplicate', 'unsafe-revision'] as const)('rejects %s list data after one real HTTP dispatch', async cause => {
  const h = setup(); await h.entry.start(); const row = h.server.reports.report
  h.server.reports.list = { items: cause === 'duplicate' ? [row, row] : [{ ...row,
    ...(cause === 'extra' ? { reporter_id: id(999) } : cause === 'foreign-event' ? { event_id: id(999) } : { revision: Number.MAX_SAFE_INTEGER + 1 }) }], next_cursor: null }
  const controller = h.entry.read().reports!.borrow()!; await controller.load()
  expect(controller.read()).toMatchObject({ phase: 'failed', rows: [], detail: null })
  expect(h.server.requests.filter(r => new URL(r.url).pathname.endsWith('/admin/reports'))).toHaveLength(1)
})
it('sends one exact confirmed decision and accepts only the advancing Report projection', async () => {
  const h = setup(), controller = await review(h)
  controller.draft('RESOLVED', 'A factual report decision'); controller.review(); controller.cancel()
  await controller.submit(); expect(h.server.requests.filter(r => r.method === 'POST')).toHaveLength(0)
  controller.review(); await controller.submit()
  expect(controller.read()).toMatchObject({ phase: 'applied', selected: { state: 'RESOLVED', revision: 2 }, detail: null })
  const writes = h.server.requests.filter(r => r.method === 'POST'); expect(writes).toHaveLength(1)
  expect(await writes[0]!.clone().json()).toEqual({ expected_revision: 1, decision: 'RESOLVED', note: 'A factual report decision' })
  expect(writes[0]!.headers.get('Idempotency-Key')).toMatch(/^[0-9a-f-]{36}$/)
  await controller.submit(); expect(h.server.requests.filter(r => r.method === 'POST')).toHaveLength(1)
})
it.each(['pending', 'lost'] as const)('retains the %s command and original key across repeated borrowers without replay', async mode => {
  const h = setup(), controller = await review(h); h.server.reports.mode = mode
  controller.draft('DISMISSED', 'Not actionable after review'); controller.review(); await controller.submit()
  expect(controller.read().phase).toBe(mode === 'pending' ? 'pending' : 'unknown')
  expect(h.admin.open(h.scope)?.read().reports?.borrow()).toBe(controller)
  await controller.load(); await controller.submit(); expect(h.server.requests.filter(r => r.method === 'POST')).toHaveLength(1)
  const key = h.server.reports.key
  h.server.reports.report = { ...h.server.reports.report, state: 'DISMISSED', revision: 2 }; h.server.reports.status = 'SUCCEEDED'
  await controller.checkStatus(); expect(controller.read().phase).toBe('applied')
  const lookup = h.server.requests.find(r => new URL(r.url).pathname.endsWith('/commands'))!
  expect(new URL(lookup.url).searchParams.get('idempotency_key')).toBe(key)
  expect(h.server.requests.filter(r => r.method === 'POST')).toHaveLength(1)
})
it('retains a recorded failure and requires an explicit refresh before any new action', async () => {
  const h = setup(), controller = await review(h); h.server.reports.mode = 'pending'
  controller.draft('RESOLVED', 'Reviewed'); controller.review(); await controller.submit()
  h.server.reports.status = 'FAILED'; await controller.checkStatus()
  expect(controller.read().phase).toBe('failed'); controller.review(); await controller.submit()
  expect(h.server.requests.filter(r => r.method === 'POST')).toHaveLength(1)
})
it.each(['actor', 'event', 'policy', 'clock', 'expiry', 'dispose'] as const)('permanently retires the borrowed Reports controller on %s', async cause => {
  const h = setup(), controller = await review(h), child = h.entry.read().reports!
  if (cause === 'actor' || cause === 'event') h.auth.session.replace(h.auth.session.read().generation, {
    userId: cause === 'actor' ? id(999) : wire.actor, eventId: cause === 'event' ? id(999) : wire.event, accessToken: 'TEST_ONLY_new_scope' })
  if (cause === 'policy') h.auth.privacy.changePolicy(h.auth.privacy.capture(), 'block')
  if (cause === 'clock') h.interrupt()
  if (cause === 'expiry') h.advance(60_000)
  if (cause === 'dispose') h.admin.dispose()
  expect(h.entry.read().reports).toBeNull(); expect(child.borrow()).toBeNull(); expect(child.isDisposed()).toBe(true)
  expect(controller.read()).toMatchObject({ access: false, rows: [], detail: null, reason: '', receipt: null })
  const count = h.server.requests.length; await controller.load(); await controller.submit(); expect(h.server.requests).toHaveLength(count)
})
it('withdraws the old child before an equal-field new decision becomes visible', async () => {
  const h = setup(), controller = await review(h), child = h.entry.read().reports!, values: unknown[] = []
  h.entry.subscribe(() => values.push(h.entry.read().reports)); h.server.replaceDecision(); await h.entry.refresh()
  expect(values[0]).toBeNull(); expect(child.isDisposed()).toBe(true)
  expect(h.entry.read().reports?.borrow()).not.toBe(controller); expect(controller.read().access).toBe(false)
})
it.each(['list', 'detail'] as const)('aborts a pending %s read and rejects its late completion across policy change', async kind => {
  const h = setup(); await h.entry.start(); const controller = h.entry.read().reports!.borrow()!, delayed = deferred<Response>()
  if (kind === 'detail') await controller.load()
  h.server.reports.intercept = request => new URL(request.url).pathname.endsWith(kind === 'list' ? '/admin/reports' : `/${id(301)}`) ? delayed.promise : null
  const pending = kind === 'list' ? controller.load() : controller.select(id(301))
  await vi.waitFor(() => expect(h.server.requests.at(-1)!.url).toContain(kind === 'list' ? '/admin/reports?' : `/${id(301)}`))
  const active = h.server.requests.at(-1)!
  h.auth.privacy.changePolicy(h.auth.privacy.capture(), 'block')
  await vi.waitFor(() => expect(active.signal.aborted).toBe(true))
  delayed.resolve(Response.json(kind === 'list' ? { items: [h.server.reports.report], next_cursor: null }
    : { report: h.server.reports.report, reporter_id: id(999), reason: 'PRIVACY', text: 'late private data', evidence_media_ids: [] }))
  await pending; expect(controller.read()).toMatchObject({ access: false, rows: [], detail: null })
})
it('mounts Reports through the actual default route and retains a pending command across sibling navigation', async () => {
  const h = setup('OPERATOR'); h.admin.dispose()
  const ports = createProductionPorts(h.auth); owners.push({ dispose: () => ports.dispose?.() })
  const ready = new Promise<void>(resolve => { const stop = h.auth.subscribe(() => { if (h.auth.read().capabilities !== 'loading') { stop(); resolve() } }) })
  h.auth.start(); await ready; expect(await h.auth.login('moderator@example.com', 'TEST_ONLY_password')).toBe(true)
  const scope = { generation: h.auth.session.read().generation, userId: wire.actor, eventId: wire.event }
  const router = createMemoryRouter(createAppRoutes(ports), { initialEntries: ['/admin/reports'] }); owners.push({ dispose: () => router.dispose() })
  render(<RouterProvider router={router} />)
  await screen.findByRole('heading', { name: 'Report queue' }); fireEvent.click(await screen.findByRole('button', { name: /Review 00000000/ }))
  await screen.findByText('Private test statement'); expect(screen.queryByText(id(999))).toBeNull()
  expect(document.querySelector('main main')).toBeNull()
  fireEvent.change(screen.getByLabelText('Action', { exact: true }), { target: { value: 'RESOLVED' } })
  fireEvent.change(screen.getByLabelText('Reason for this action'), { target: { value: 'Reviewed route retention' } })
  fireEvent.click(screen.getByRole('button', { name: 'Review action' })); h.server.reports.mode = 'pending'
  fireEvent.click(screen.getByRole('button', { name: 'Confirm action' })); await screen.findByRole('button', { name: 'Check original command' })
  const child = ports.admin.open!(scope)!.read().reports!, controller = child.borrow()!, key = h.server.reports.key
  fireEvent.click(screen.getByRole('link', { name: 'Event' })); await screen.findByRole('heading', { name: 'Event settings' })
  fireEvent.click(screen.getByRole('link', { name: 'Reports' })); await screen.findByRole('button', { name: 'Check original command' })
  expect(ports.admin.open!(scope)!.read().reports?.borrow()).toBe(controller); expect(h.server.reports.key).toBe(key)
  h.server.reports.status = 'FAILED'; fireEvent.click(screen.getByRole('button', { name: 'Check original command' }))
  await waitFor(() => expect(screen.getByRole('alert')).toHaveTextContent('server recorded failure'))
  expect(h.server.requests.filter(r => r.method === 'POST' && new URL(r.url).pathname.endsWith('/decision'))).toHaveLength(1)
})
