import { createServer, type IncomingMessage, type ServerResponse } from 'node:http'
import type { AddressInfo } from 'node:net'
import { afterEach, expect, it, vi } from 'vitest'
import { createHttpClient, createMemorySession } from '../../http'
import { TransportError } from '../../errors'
const EVENT = '00000000-0000-4000-8000-000000000201', KEY = '00000000-0000-4000-8000-000000000301'
const body = { expected_revision: 10, x: 1, y: 2, color_index: 2 }
afterEach(() => vi.unstubAllGlobals())
async function served(work: (client: ReturnType<typeof createHttpClient>, received: { method?: string; key: string | string[] | undefined; body: unknown }[]) => Promise<void>, firstHint?: number) {
  const received: { method?: string; key: string | string[] | undefined; body: unknown }[] = []
  const server = createServer(async (req: IncomingMessage, res: ServerResponse) => {
    const parts: Buffer[] = []; for await (const part of req) parts.push(Buffer.from(part))
    received.push({ ...(req.method ? { method: req.method } : {}), key: req.headers['idempotency-key'], body: JSON.parse(Buffer.concat(parts).toString('utf8')) })
    res.writeHead(429, { 'content-type': 'application/json' })
    res.end(JSON.stringify({ code: 'MURAL_COOLDOWN_REPLAY', message: 'Fixture failed command.', details: { reason: 'MURAL_COOLDOWN', ...(received.length === 1 && firstHint !== undefined ? { retry_after_ms: firstHint } : {}) } }))
  })
  await new Promise<void>(resolve => server.listen(0, '127.0.0.1', resolve))
  const address = server.address() as AddressInfo
  vi.stubGlobal('location', { origin: `http://127.0.0.1:${address.port}` })
  const session = createMemorySession(); session.replace(0, { userId: '00000000-0000-4000-8000-000000000101', eventId: EVENT, accessToken: 'fixture-memory-token' })
  try { await work(createHttpClient({ session }), received) }
  finally { session.replace(1, null); server.closeAllConnections(); await new Promise<void>((resolve, reject) => server.close(e => e ? reject(e) : resolve())) }
}
function place(client: ReturnType<typeof createHttpClient>) {
  return client.request({ method: 'POST', path: `/api/concert/events/${EVENT}/mural/pixels`, access: 'protected', errorFamily: 'domain', domainOperation: 'placeConcertPixel',
    idempotencyKey: KEY, body: { json: body }, decode: () => { throw Error('Failed command must not decode success') } })
}
async function error(client: ReturnType<typeof createHttpClient>) {
  try { await place(client) } catch (e) { expect(e).toBeInstanceOf(TransportError); return e as TransportError }
  throw Error('Expected a failed cooldown command')
}
it('real loopback HTTP retains a reason-only failed replay with no injected fetch or invented hint', async () => {
  await served(async (client, received) => {
    const failure = await error(client)
    expect(failure).toMatchObject({ kind: 'http', status: 429, retryAfterMs: null, uncertain: false, server: { reason: 'MURAL_COOLDOWN' } })
    expect(received).toEqual([{ method: 'POST', key: KEY, body }]); expect(failure.server!.envelope.details).toEqual({ reason: 'MURAL_COOLDOWN' })
  })
})
it('real same-key failed replay drops the earlier hint while preserving the original key/body and reason', async () => {
  await served(async (client, received) => {
    expect((await error(client)).retryAfterMs).toBe(1250)
    const replay = await error(client)
    expect(replay.server?.reason).toBe('MURAL_COOLDOWN'); expect(replay.retryAfterMs).toBeNull()
    expect(received).toEqual([{ method: 'POST', key: KEY, body }, { method: 'POST', key: KEY, body }])
  }, 1250)
})
