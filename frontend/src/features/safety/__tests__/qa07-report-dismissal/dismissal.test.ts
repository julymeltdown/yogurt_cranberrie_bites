// @vitest-environment jsdom
import { afterEach, expect, it, vi } from 'vitest'
import { deferred, harness, ids, link, now, receipt, report } from '../fixtures'

const owners: ReturnType<typeof harness>[] = []
function setup(...args: Parameters<typeof harness>) {
  const owner = harness(...args)
  owners.push(owner)
  return owner
}
afterEach(() => owners.splice(0).forEach(owner => owner.dispose()))

it('dismisses an unsent draft without revoking the retained Report/Block owner', async () => {
  const h = setup(() => Response.json({ target_user_id: ids.target, blocked_by_me: true, revision: 1 }))
  const scope = h.auth.privacy.capture(), cleanup = vi.fn()
  h.auth.privacy.registerCleanup(cleanup)
  await h.controller.open(link)
  h.controller.edit({ reason: 'PRIVACY', text: 'PRIVATE draft', evidence: [ids.media] })
  h.controller.close()
  expect(h.controller.read()).toMatchObject({ phase: 'idle', target: null, reason: 'HARASSMENT', text: '', evidence: [], report: null, receipt: null, message: '' })
  expect(h.auth.privacy.capture()).toEqual(scope)
  expect(cleanup).not.toHaveBeenCalled()
  expect(h.requests).toHaveLength(0)
  await h.controller.open(link)
  expect(h.controller.read()).toMatchObject({ phase: 'ready', text: '', evidence: [] })
  h.controller.close()
  await h.controller.block(ids.target)
  expect(h.requests.map(r => r.method)).toEqual(['PUT'])
  expect(cleanup).toHaveBeenCalledTimes(1)
  expect(h.controller.read().phase).toBe('closed')
})

it('dismisses a completed report and lets the same owner start another authorized report', async () => {
  const h = setup()
  await h.controller.open(link)
  await h.controller.report()
  expect(h.controller.read().phase).toBe('succeeded')
  h.controller.close()
  expect(h.controller.read()).toMatchObject({ phase: 'idle', report: null, text: '', evidence: [] })
  await h.controller.open(link)
  expect(h.controller.read().phase).toBe('ready')
})

it('cancels a dismissed authorization and fences an ignored-abort late target', async () => {
  const late = deferred<boolean>(), started = vi.fn()
  const h = setup(undefined, { authorization: { async check(target, evidenceMediaIds, scope) {
    started()
    await late.promise
    return { target, evidenceMediaIds, scope, allowed: true, expiresAt: now + 10000 }
  } } })
  const opening = h.controller.open(link)
  expect(h.controller.read().phase).toBe('authorizing')
  await vi.waitFor(() => expect(started).toHaveBeenCalledOnce())
  h.controller.close()
  late.resolve(true)
  await opening
  expect(h.controller.read()).toMatchObject({ phase: 'idle', target: null, text: '' })
  expect(h.requests).toHaveLength(0)
})

it.each(['identity', 'event', 'block', 'suspended', 'event-closed', 'dispose'] as const)(
  'dismissal cannot revive authority or late report work after %s cleanup', async reason => {
    const late = deferred<Response>(), submitted = vi.fn()
    const h = setup(() => late.promise, { onReportSubmitted: submitted })
    await h.controller.open(link)
    h.controller.edit({ text: 'PRIVATE draft', evidence: [ids.media] })
    const pending = h.controller.report()
    await vi.waitFor(() => expect(h.requests).toHaveLength(1))
    if (reason === 'identity' || reason === 'event') {
      h.auth.session.replace(h.auth.session.read().generation, { userId: reason === 'identity' ? ids.target : ids.actor, eventId: reason === 'event' ? ids.target : ids.event, accessToken: 'NEXT_SYNTHETIC' })
    } else if (reason === 'dispose') h.controller.dispose()
    else h.auth.privacy.changePolicy(h.auth.privacy.capture(), reason)
    h.controller.close()
    await h.controller.open(link)
    await h.controller.block(ids.target)
    await h.controller.checkStatus()
    expect(h.controller.read()).toMatchObject({ phase: 'closed', target: null, text: '', evidence: [], report: null, blocks: null, receipt: null })
    late.resolve(Response.json(report()))
    await pending
    expect(h.requests).toHaveLength(1)
    expect(h.controller.read().phase).toBe('closed')
    expect(submitted).not.toHaveBeenCalled()
  },
)

it.each(['pending', 'unknown', 'FAILED', 'RETIRED'] as const)(
  'dismissal preserves the original command and sends no replacement after %s', async outcome => {
    const key = vi.fn(() => ids.key)
    const h = setup(r => {
      if (r.method === 'POST') return outcome === 'unknown' ? Promise.reject(new TypeError('offline')) : Response.json(receipt(), { status: 202 })
      return Response.json(receipt(outcome === 'FAILED' ? { state: 'FAILED', failure_reason: 'RESOURCE_UNAVAILABLE' } : outcome === 'RETIRED' ? { state: 'RETIRED' } : {}))
    }, { newKey: key })
    await h.controller.open(link)
    h.controller.edit({ text: 'PRIVATE draft', evidence: [ids.media] })
    await h.controller.report()
    if (outcome === 'FAILED' || outcome === 'RETIRED') await h.controller.checkStatus()
    h.controller.close()
    expect(h.controller.read()).toMatchObject({ phase: 'unknown', text: '', evidence: [], target: null })
    await h.controller.open(link)
    await h.controller.report()
    await h.controller.block(ids.target)
    await h.controller.checkStatus()
    expect(key).toHaveBeenCalledTimes(1)
    expect(h.requests.filter(r => r.method !== 'GET')).toHaveLength(1)
    expect(new URL(h.requests.at(-1)!.url).searchParams.get('idempotency_key')).toBe(ids.key)
    expect(h.controller.read().phase).not.toBe('succeeded')
  },
)

it('dismisses an in-flight write without accepting its late result or losing the original status key', async () => {
  const late = deferred<Response>(), submitted = vi.fn(), key = vi.fn(() => ids.key)
  const h = setup(r => r.method === 'POST' ? late.promise : Response.json(new URL(r.url).pathname.endsWith('/commands')
    ? receipt({ state: 'SUCCEEDED', resource_id: ids.report, resource_revision: 1, result_status: 200 })
    : { items: [report()], next_cursor: null }), { newKey: key, onReportSubmitted: submitted })
  await h.controller.open(link)
  h.controller.edit({ text: 'PRIVATE draft' })
  const sending = h.controller.report()
  await vi.waitFor(() => expect(h.requests).toHaveLength(1))
  h.controller.close()
  await h.controller.open(link)
  await h.controller.block(ids.target)
  late.resolve(Response.json(report()))
  await sending
  expect(h.controller.read()).toMatchObject({ phase: 'unknown', text: '', report: null })
  expect(submitted).not.toHaveBeenCalled()
  await h.controller.checkStatus()
  expect(new URL(h.requests[1]!.url).searchParams.get('idempotency_key')).toBe(ids.key)
  expect(h.requests.filter(r => r.method !== 'GET')).toHaveLength(1)
  expect(key).toHaveBeenCalledTimes(1)
  expect(submitted).toHaveBeenCalledTimes(1)
  expect(h.controller.read().phase).toBe('succeeded')
})
