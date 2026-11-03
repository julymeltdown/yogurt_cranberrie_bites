import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { fixture, ids } from './fixture'
import type { AdminCommand } from '../../../mural/contract'
let f: ReturnType<typeof fixture>
beforeEach(async () => { vi.stubGlobal('location', new URL('https://mcr.test')); f = fixture(); await f.open() })
afterEach(() => { f.cleanup(); vi.unstubAllGlobals() })
const resume = (): AdminCommand => ({ operation: 'closeConcertEvent', key: '00000000-0000-4000-8000-000000000800', scope: f.privacy.capture(), roleRevision: f.context().roleRevision,
  body: { confirmation: 'RESUME_CLOSURE', expected_revision: 8, job_id: ids.job, reason: 'Explicit recovery' } })
describe('C04 resume HTTP fences', () => {
  it('same-key identical-body transport replay returns admission without retrying a later failed boundary', async () => {
    const command = resume(); await f.api.send(f.context(), new AbortController().signal, command); f.failAgain()
    const before = structuredClone(f.server.job); const result = await f.api.send(f.context(), new AbortController().signal, command)
    expect(result.kind).toBe('confirmed'); expect(f.server.job).toEqual(before); expect(f.server.admissions).toBe(1)
  })
  it.each(['wrong-job','wrong-event','nonadvancing','private-author','regress-success','false-complete'])('invalid 200 %s remains unknown', async mode => {
    f.server.transform = (value, request) => {
      if (request.method !== 'POST') return value
      const p = structuredClone(value) as typeof f.server.job & { actor_id?: string }
      if (mode === 'wrong-job') p.job_id = ids.other
      if (mode === 'wrong-event') p.event_id = ids.other
      if (mode === 'nonadvancing') p.revision = 8
      if (mode === 'private-author') p.actor_id = ids.actor
      if (mode === 'regress-success') p.steps[0]!.state = 'PENDING'
      if (mode === 'false-complete') p.state = 'COMPLETED'
      return p
    }
    f.controller.prepareResume('Recovery'); await f.controller.confirm(); expect(f.controller.read().commandPhase).toBe('unknown'); expect(f.posts()).toHaveLength(1)
  })
  it.each(['actor_id','event_id','operation_id','idempotency_key','resource_id','resource_revision','result_status','expiry','extra'])('forged receipt %s cannot resolve or replay the original resume', async field => {
    f.server.mode = 'lost'; f.controller.prepareResume('Recovery'); await f.controller.confirm(); const key = f.command().key
    f.server.transform = (value, request) => {
      if (!new URL(request.url).pathname.endsWith('/commands')) return value
      const p = { ...(value as Record<string, unknown>) }
      const changes: Record<string, unknown> = { actor_id: ids.other, event_id: ids.other, operation_id: 'controlConcertMural', idempotency_key: ids.other,
        resource_id: ids.other, resource_revision: 8, result_status: 202, expiry: '2026-11-03T00:00:00Z', extra: 'private' }
      p[field === 'expiry' ? 'expires_at' : field] = changes[field]; return p
    }
    await f.controller.check(); expect(f.controller.read().commandPhase).toBe('unknown'); expect(f.command().key).toBe(key); expect(f.posts()).toHaveLength(1)
    if (field === 'resource_id') expect(f.server.requests.filter(r => r.url.endsWith(`/closure/${ids.other}`))).toHaveLength(0)
  })
  it.each(['RETIRED','PENDING','expired','404'])('%s status keeps the original command blocked', async state => {
    f.server.mode = 'lost'; f.controller.prepareResume('Recovery'); await f.controller.confirm(); const original = f.command(), r = f.server.receipts.get(original.key)!
    if (state === '404') f.server.receipts.delete(original.key)
    else if (state === 'expired') f.advance(86400000)
    else Object.assign(r, { state, resource_id: null, resource_revision: null, result_status: null, failure_reason: null })
    await f.controller.check(); expect(f.controller.prepareResume('Again')).toBe(false); expect(f.command()).toBe(original); expect(f.posts()).toHaveLength(1)
  })
  it('success metadata requires a current authorized job read at least at the accepted revision', async () => {
    f.server.mode = 'lost'; f.controller.prepareResume('Recovery'); await f.controller.confirm()
    f.server.intercept = request => request.method === 'GET' && request.url.endsWith(`/closure/${ids.job}`)
      ? Promise.resolve(Response.json({ code: 'FIXTURE', message: 'Access revoked', details: { reason: 'PERMISSION_DENIED' } }, { status: 403 })) : null
    await f.controller.check(); expect(f.controller.read().commandPhase).toBe('unknown'); expect(f.posts()).toHaveLength(1)
    expect(f.server.requests.filter(r => r.method === 'GET' && r.url.endsWith(`/closure/${ids.job}`))).toHaveLength(2)
  })
  it('another current operator cannot read or resume the initiating actor’s job', async () => {
    f.login(ids.other); f.update({ ...f.context(), scope: f.privacy.capture(), roleRevision: 2 })
    expect((await f.api.send(f.context(), new AbortController().signal, resume())).kind).toBe('rejected'); expect(f.server.admissions).toBe(0)
    await expect(f.api.read(f.context(), new AbortController().signal, ids.job)).rejects.toThrow()
  })
  it('server role denial wins over a locally prepared control', async () => {
    f.controller.prepareResume('Recovery'); f.server.operator = false; await f.controller.confirm()
    expect(f.controller.read()).toMatchObject({ commandPhase: 'rejected', job: null }); expect(f.server.admissions).toBe(0)
  })
  it('server policy unavailability preserves an uncertain command without admitting work', async () => {
    f.controller.prepareResume('Recovery'); f.server.policy = false; await f.controller.confirm()
    expect(f.controller.read().commandPhase).toBe('unknown'); expect(f.server.admissions).toBe(0)
    await f.controller.check(); expect(f.controller.read().commandPhase).toBe('unknown'); expect(f.posts()).toHaveLength(1)
  })
  it('throttling prevents repeated explicit requests until the server delay has passed', async () => {
    f.server.mode = 'throttle'; f.controller.prepareResume('Recovery'); await f.controller.confirm(); const count = f.server.requests.length
    await f.controller.refresh(); expect(f.server.requests).toHaveLength(count)
    f.advance(1000); f.server.mode = 'ack'; await f.controller.refresh(); expect(f.controller.canResume()).toBe(true); expect(f.posts()).toHaveLength(1)
  })
})
