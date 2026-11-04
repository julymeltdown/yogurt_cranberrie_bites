import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { DOMAIN_ERROR_RULES } from '../../domain-operations.generated'
import { parseServerError, TransportError } from '../../errors'
import { createHttpClient, createMemorySession, type HttpRequest } from '../../http'
const EVENT = '00000000-0000-4000-8000-000000000201', USER = '00000000-0000-4000-8000-000000000101', KEY = '00000000-0000-4000-8000-000000000301'
const envelope = (details: unknown) => ({ code: 'MURAL_COOLDOWN_REPLAY', message: 'An arbitrary localized error message.', traceId: 'cooldown-fixture', details })
const cooldown = (hint: number) => envelope({ reason: 'MURAL_COOLDOWN', retry_after_ms: hint })
const replay = () => envelope({ reason: 'MURAL_COOLDOWN' })
beforeEach(() => vi.stubGlobal('location', { origin: 'https://mcr.example' }))
afterEach(() => { vi.unstubAllGlobals(); vi.useRealTimers() })
function setup(handler: (r: Request) => Response) {
  const session = createMemorySession(); session.replace(0, { userId: USER, eventId: EVENT, accessToken: 'fixture-memory-token' })
  const fetch = vi.fn<typeof globalThis.fetch>(async input => handler(input as Request))
  return { session, fetch, client: createHttpClient({ session, fetch }) }
}
const BODY = { expected_revision: 10, x: 1, y: 2, color_index: 2 }
const request = (): HttpRequest<never> => ({ method: 'POST', path: `/api/concert/events/${EVENT}/mural/pixels`, access: 'protected', errorFamily: 'domain',
  domainOperation: 'placeConcertPixel', idempotencyKey: KEY, body: { json: BODY }, decode: () => { throw Error('A failed command cannot decode success') } })
async function rejected(work: Promise<unknown>): Promise<TransportError> {
  try { await work } catch (e) { expect(e).toBeInstanceOf(TransportError); return e as TransportError }
  throw Error('Expected the command to fail')
}
describe('canonical optional cooldown hint', () => {
  it('reflects the actual generated optional rule while retaining mandatory rate-limit metadata', () => {
    expect(DOMAIN_ERROR_RULES.MURAL_COOLDOWN).toEqual({ type: 'integer', minimum: 1, maximum: 3600000, required: false })
    expect(DOMAIN_ERROR_RULES.RATE_LIMITED).toEqual({ type: 'integer', minimum: 1, maximum: 3600000, required: true })
  })
  it('retains a reason-only failed cooldown replay without manufacturing an undefined/zero retry hint', () => {
    const value = replay(), parsed = parseServerError(value, 'domain', 429)!
    expect(parsed.reason).toBe('MURAL_COOLDOWN'); expect(parsed.envelope.details).toEqual({ reason: 'MURAL_COOLDOWN' })
    expect(Object.hasOwn(parsed.envelope.details!, 'retry_after_ms')).toBe(false)
    expect(Object.isFrozen(parsed.envelope.details)).toBe(true); expect(parsed.envelope.details).not.toBe(value.details)
  })
  it.each([1, 1250, 3600000])('retains a bounded supplied hint %s', hint => {
    expect(parseServerError(cooldown(hint), 'domain', 429)?.envelope.details).toEqual({ reason: 'MURAL_COOLDOWN', retry_after_ms: hint })
  })
  it.each([undefined, null, 0, -1, 1.5, 3600001, Number.MAX_SAFE_INTEGER, NaN, Infinity, '1000', true, {}, []])('rejects a present malformed hint %#', hint => {
    expect(parseServerError(envelope({ reason: 'MURAL_COOLDOWN', retry_after_ms: hint }), 'domain', 429)).toBeNull()
  })
  it.each([{ reason: 'MURAL_COOLDOWN', secret: true }, { reason: 'MURAL_COOLDOWN', retry_after_ms: 1, next_allowed_at: '2026-11-04T00:00:00Z' },
    { reason: 'RESOURCE_UNAVAILABLE', retry_after_ms: 1 }, { reason: 'UNKNOWN', retry_after_ms: 1 }])('keeps unknown fields/reasons closed %#', details => {
    expect(parseServerError(envelope(details), 'domain', 429)).toBeNull()
  })
  it('does not make RATE_LIMITED hints optional or change inherited core semantics', () => {
    expect(parseServerError(envelope({ reason: 'RATE_LIMITED' }), 'domain', 429)).toBeNull()
    expect(parseServerError(envelope({ reason: 'RATE_LIMITED', retry_after_ms: 1 }), 'domain', 429)?.reason).toBe('RATE_LIMITED')
    expect(parseServerError(replay(), 'core', 429)?.core).toEqual({ reason: 'MURAL_COOLDOWN' })
  })
})
describe('canonical HTTP failed-command boundary', () => {
  it('preserves reason/no-hint through the registered operation, sends once and never schedules a POST replay', async () => {
    vi.useFakeTimers(); const f = setup(() => Response.json(replay(), { status: 429 }))
    const error = await rejected(f.client.request(request()))
    expect(error).toMatchObject({ kind: 'http', status: 429, retryAfterMs: null, uncertain: false, server: { reason: 'MURAL_COOLDOWN' } })
    expect(error.server!.envelope.details).toEqual({ reason: 'MURAL_COOLDOWN' }); expect(error).not.toHaveProperty('response')
    await vi.advanceTimersByTimeAsync(3600000); expect(f.fetch).toHaveBeenCalledTimes(1)
    const sent = f.fetch.mock.calls[0]![0] as Request
    expect(sent.method).toBe('POST'); expect(sent.headers.get('Idempotency-Key')).toBe(KEY)
    f.session.replace(1, null)
  })
  it('allows an explicit same-key failed replay to omit the earlier hint without retaining the old hint', async () => {
    let calls = 0; const f = setup(() => Response.json(++calls === 1 ? cooldown(1000) : replay(), { status: 429 }))
    expect((await rejected(f.client.request(request()))).retryAfterMs).toBe(1000)
    const repeated = await rejected(f.client.request(request()))
    expect(repeated.server?.reason).toBe('MURAL_COOLDOWN'); expect(repeated.retryAfterMs).toBeNull(); expect(f.fetch).toHaveBeenCalledTimes(2)
    expect(f.fetch.mock.calls.map(([r]) => (r as Request).headers.get('Idempotency-Key'))).toEqual([KEY, KEY])
  })
  it('keeps Retry-After as a separate bounded transport hint without inventing a details field', async () => {
    const f = setup(() => Response.json(replay(), { status: 429, headers: { 'Retry-After': '2' } })), error = await rejected(f.client.request(request()))
    expect(error.retryAfterMs).toBe(2000); expect(error.server?.envelope.details).toEqual({ reason: 'MURAL_COOLDOWN' })
  })
  it.each([400, 409, 503])('does not admit cooldown under an unregistered HTTP status %s', async status => {
    const f = setup(() => Response.json(replay(), { status })), error = await rejected(f.client.request(request()))
    expect(error.status).toBe(status); expect(error.server).toBeNull(); expect(error.retryAfterMs).toBeNull(); expect(f.fetch).toHaveBeenCalledTimes(1)
  })
  it('rejects a malformed provided hint without converting the failure to success or automatically replaying', async () => {
    const f = setup(() => Response.json(envelope({ reason: 'MURAL_COOLDOWN', retry_after_ms: null }), { status: 429 })), error = await rejected(f.client.request(request()))
    expect(error.kind).toBe('http'); expect(error.status).toBe(429); expect(error.server).toBeNull(); expect(f.fetch).toHaveBeenCalledTimes(1)
  })
})
