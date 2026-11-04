// @vitest-environment jsdom
import { afterEach, expect, it } from 'vitest'
import { harness, report } from '../fixtures/harness'
const owners: ReturnType<typeof harness>[] = []
afterEach(() => owners.splice(0).forEach(h => h.dispose()))
function setup() {
  const h = harness()
  // The historical feature harness predates canonical enrollment admission.
  // Supply its strict current HTTP response here, without changing Auth or the old fixture.
  function override(fn: Parameters<typeof h.override>[0]) {
    h.override(r => new URL(r.url).pathname === '/api/auth/enrollment/admission'
      ? Response.json({ decision: 'ADMITTED', policyRevision: '11111111-1111-4111-8111-111111111111',
        acceptedRevision: '11111111-1111-4111-8111-111111111111', serverTime: new Date().toISOString() })
      : fn?.(r) ?? null)
  }
  override(null)
  return { ...h, override }
}
it.each([404, 429, 503])('keeps a pending command uncertain after status lookup HTTP %s and prevents a second write', async status => {
  const h = setup(); owners.push(h)
  await h.controller.load(); expect(h.controller.read().phase).toBe('ready'); await h.controller.select(report.report_id)
  h.controller.draft('RESOLVED', 'Reviewed factual evidence'); h.controller.review()
  h.override(r => r.method === 'POST' ? Response.json(h.receipt(), { status: 202 })
    : new URL(r.url).pathname.endsWith('/commands') ? Response.json({ code: 'TEST_ONLY', message: 'PRIVATE status diagnostics',
      details: { reason: status === 404 ? 'RESOURCE_UNAVAILABLE' : status === 429 ? 'RATE_LIMITED' : 'DEPENDENCY_UNAVAILABLE' } }, { status }) : null)
  await h.controller.submit(); expect(h.controller.read().phase).toBe('pending')
  await h.controller.checkStatus(); expect(h.controller.read().phase).toBe('unknown')
  const count = h.requests.length
  await h.controller.load(); h.controller.draft('DISMISSED', 'A different action'); h.controller.review(); await h.controller.submit()
  expect(h.requests).toHaveLength(count); expect(h.requests.filter(r => r.method === 'POST')).toHaveLength(1)
  h.override(r => new URL(r.url).pathname.endsWith('/commands') ? Response.json(h.receipt('FAILED')) : null)
  await h.controller.checkStatus(); expect(h.controller.read().phase).toBe('failed')
})
it.each([401, 403])('scrubs private state and authority on status lookup HTTP %s', async status => {
  const h = setup(); owners.push(h)
  await h.controller.load(); expect(h.controller.read().phase).toBe('ready'); await h.controller.select(report.report_id)
  h.controller.draft('RESOLVED', 'Reviewed'); h.controller.review()
  h.override(r => r.method === 'POST' ? Response.json(h.receipt(), { status: 202 })
    : new URL(r.url).pathname.endsWith('/commands') || new URL(r.url).pathname.endsWith('/refresh')
      ? Response.json({ code: 'TEST_ONLY', message: 'PRIVATE', details: { reason: status === 401 ? 'AUTHENTICATION_REQUIRED' : 'PERMISSION_DENIED' } }, { status }) : null)
  await h.controller.submit(); await h.controller.checkStatus()
  expect(h.controller.read()).toMatchObject({ access: false, rows: [], detail: null, reason: '', receipt: null })
})
it.each(['wrong-actor', 'wrong-receipt', 'network'] as const)('retains the original uncertain command after %s status and accepts only a qualified terminal receipt', async cause => {
  const h = setup(); owners.push(h)
  await h.controller.load(); expect(h.controller.read().phase).toBe('ready'); await h.controller.select(report.report_id)
  h.controller.draft('RESOLVED', 'Reviewed'); h.controller.review()
  h.override(r => r.method === 'POST' ? Response.json(h.receipt(), { status: 202 }) : null)
  await h.controller.submit(); const original = h.controller.read().receipt
  h.override(r => {
    if (!new URL(r.url).pathname.endsWith('/commands')) return null
    if (cause === 'network') throw new TypeError('TEST_ONLY lost status response')
    return Response.json(h.receipt('FAILED', cause === 'wrong-actor' ? { actor_id: report.target_id } : { receipt_id: report.target_id }))
  })
  await h.controller.checkStatus(); expect(h.controller.read()).toMatchObject({ phase: 'unknown', receipt: original })
  const count = h.requests.length; await h.controller.load(); await h.controller.submit(); expect(h.requests).toHaveLength(count)
  h.override(r => new URL(r.url).pathname.endsWith('/commands') ? Response.json(h.receipt('FAILED')) : null)
  await h.controller.checkStatus(); expect(h.controller.read().phase).toBe('failed')
})
