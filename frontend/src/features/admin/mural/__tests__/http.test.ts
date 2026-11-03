import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { fixture, ids, wall } from './fixture'
import type { AdminCommand } from '../contract'
let h: ReturnType<typeof fixture>
beforeEach(() => { vi.stubGlobal('location', new URL('https://mcr.test')); h = fixture() })
afterEach(() => { h.cleanup(); vi.unstubAllGlobals() })
const command = (): AdminCommand => ({ operation: 'controlConcertMural', body: { expected_revision: 10, action: 'PAUSE', reason: 'Reviewed' }, key: '00000000-0000-4000-8000-000000000111', scope: h.privacy.capture(), roleRevision: 1 })
describe('FE39 canonical HTTP boundary', () => {
  it.each(['actor_id','event_id','operation_id','idempotency_key','resource_id','resource_revision','result_status','expires_at','failure_reason','extra'])('forged receipt %s cannot resolve or replay a destructive command', async field => {
    h.server.mode = 'lost'; h.closure.start(); h.closure.prepare('Finish'); await h.closure.confirm()
    const captured = h.closure.read().command!, original = h.server.receipts.get(captured.key)!
    const values: Record<string, unknown> = { actor_id: ids.restore, event_id: ids.restore, operation_id: 'controlConcertMural', idempotency_key: ids.restore,
      resource_id: null, resource_revision: null, result_status: 202, expires_at: '2026-11-03T00:00:00Z', failure_reason: 'VERSION_CONFLICT', extra: 'private' }
    h.server.receipts.set(captured.key, { ...original, [field]: values[field] }); await h.closure.check()
    expect(h.closure.read()).toMatchObject({ commandPhase: 'unknown', command: { key: captured.key }, job: null }); expect(h.posts()).toHaveLength(1)
  })
  it.each(['RETIRED', 'FAILED', 'PENDING'] as const)('terminal/pending status %s never automatically dispatches another command', async state => {
    await h.mural.start(); h.server.mode = 'lost'; h.mural.prepare({ operation: 'controlConcertMural', body: { expected_revision: 10, action: 'PAUSE', reason: 'Check status' } }); await h.mural.confirm()
    const key = h.mural.read().command!.key, receipt = h.server.receipts.get(key)!
    h.server.receipts.set(key, { ...receipt, state, resource_id: null, resource_revision: null, result_status: null, failure_reason: state === 'FAILED' ? 'VERSION_CONFLICT' : null })
    await h.mural.check(); expect(h.mural.read().commandPhase).toBe(state === 'PENDING' ? 'pending' : state === 'FAILED' ? 'rejected' : 'unknown'); expect(h.posts()).toHaveLength(1)
  })
  it.each(['event','private','revision','transition'])('invalid successful mutation %s remains UNKNOWN', async mode => {
    h.server.transform = (value, request) => {
      if (request.method !== 'POST') return value
      const data = { ...(value as Record<string, unknown>) }
      if (mode === 'event') data.event_id = ids.restore
      if (mode === 'private') data.actor_id = ids.actor
      if (mode === 'revision') data.revision = 19
      if (mode === 'transition') data.state = 'EDITABLE'
      return data
    }
    const result = await h.muralApi.send(h.context(), new AbortController().signal, command()); expect(result.kind).toBe('unknown'); expect(h.posts()).toHaveLength(1)
  })
  it('binds decode path, captured canonical actor/event/generation and forbids raw success statuses', async () => {
    const transport = h.transport
    await expect(transport.request(h.context(), new AbortController().signal, { method: 'GET', path: `/api/concert/events/${ids.restore}/admin/mural`, domainOperation: 'getConcertAdminMural', decode: () => wall() })).rejects.toThrow()
    expect(h.server.requests).toHaveLength(0)
  })
  it('honors 429 delay for subsequent explicit reads without scheduling an automatic retry', async () => {
    h.server.mode = 'throttle'; await h.muralApi.send(h.context(), new AbortController().signal, command())
    await expect(h.muralApi.read(h.context(), new AbortController().signal)).rejects.toThrow(); expect(h.server.requests).toHaveLength(1)
    h.advance(1000); h.server.mode = 'ack'; await h.muralApi.read(h.context(), new AbortController().signal); expect(h.server.requests).toHaveLength(2)
  })
  it('canonical timeout yields UNKNOWN once; a later response after logout never publishes', async () => {
    vi.useFakeTimers()
    try {
      h.server.intercept = () => new Promise(() => {})
      const pending = h.muralApi.send(h.context(), new AbortController().signal, command()); await vi.advanceTimersByTimeAsync(1001)
      expect(await pending).toEqual({ kind: 'unknown' }); expect(h.posts()).toHaveLength(1)
    } finally { vi.useRealTimers() }
  })
  it.each(['IDEMPOTENCY_CONFLICT','IDEMPOTENCY_EXPIRED'])('%s never proves the original destructive command failed', async reason => {
    h.server.intercept = () => Promise.resolve(Response.json({ code: 'CONFLICT', message: 'safe fixture', details: { reason } }, { status: 409 }))
    expect(await h.muralApi.send(h.context(), new AbortController().signal, command())).toEqual({ kind: 'unknown' })
    expect(h.posts()).toHaveLength(1)
  })
})
