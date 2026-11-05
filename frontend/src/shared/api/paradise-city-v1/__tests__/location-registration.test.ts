import { execFileSync } from 'node:child_process'
import { dirname, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { DOMAIN_OPERATIONS, DOMAIN_QUERIES, DOMAIN_ERROR_RULES } from '../../domain-operations.generated'
import { createHttpClient, createMemorySession } from '../../http'
import { createHttpLocationTransport } from '../location-http'
import { locationRequests } from '../location'

const event = '00000000-0000-4000-8000-000000000201'
const key = '00000000-0000-4000-8000-000000000301'
const root = resolve(dirname(fileURLToPath(import.meta.url)), '../../../../../..')
function setup() {
  const session = createMemorySession()
  session.replace(0, { userId: '00000000-0000-4000-8000-000000000101', eventId: event, accessToken: 'fixture-token' })
  const fetch = vi.fn<typeof globalThis.fetch>(async () => Response.json({ ok: true }))
  return { fetch, transport: createHttpLocationTransport(createHttpClient({ session, fetch }), session) }
}
beforeEach(() => vi.stubGlobal('location', { origin: 'https://mcr.example' }))
afterEach(() => vi.unstubAllGlobals())

it('preserves every canonical N+M operation, query and error value when adding Location', () => {
  const previous = execFileSync('/usr/bin/git', ['show', '25d94f86558aec9e90db43092d811d5d764ab8eb:frontend/src/shared/api/domain-operations.generated.ts'], { cwd: root, encoding: 'utf8' })
  for (const [name, actual] of [['DOMAIN_OPERATIONS', DOMAIN_OPERATIONS], ['DOMAIN_QUERIES', DOMAIN_QUERIES], ['DOMAIN_ERROR_RULES', DOMAIN_ERROR_RULES]] as const) {
    const block = previous.split(`export const ${name} = {\n`)[1]?.split('\n} as const')[0]
    expect(block).toBeDefined()
    const old = JSON.parse('{' + block?.replace(/,\s*$/, '') + '}') as Record<string, unknown>
    for (const [id, value] of Object.entries(old)) expect(actual).toHaveProperty(id, value)
  }
  expect(Object.keys(DOMAIN_OPERATIONS)).toHaveLength(140)
})

it('dispatches the registered Location landmark path through the actual canonical client', async () => {
  const f = setup(), descriptor = locationRequests.landmark(event, 'zone.~west')
  const result = await f.transport.request({ ...descriptor, decode: value => value }, new AbortController().signal)
  expect(result.data).toEqual({ ok: true })
  const request = f.fetch.mock.calls[0]![0] as Request
  expect(new URL(request.url).pathname).toBe(`/api/concert/events/${event}/venue/landmarks/zone.~west`)
  expect(request.headers.get('Authorization')).toBe('Bearer fixture-token')
  expect(request.cache).toBe('no-store')
  expect(f.fetch).toHaveBeenCalledTimes(1)
})

it('uses registered Location write and lookup metadata without a synthetic operation registry', async () => {
  const f = setup(), signal = new AbortController().signal
  const write = locationRequests.setSharing(event, { expected_revision: 0, state: 'PAUSED', viewer_ids: [] }, key)
  await f.transport.request({ ...write, decode: value => value }, signal)
  const first = f.fetch.mock.calls[0]![0] as Request
  expect(first.method).toBe('PUT')
  expect(first.headers.get('Idempotency-Key')).toBe(key)
  expect(first.headers.get('X-MCR-Intent')).toBe('browser')
  const lookup = locationRequests.command(event, 'setVenueLocationSharing', key)
  await f.transport.request({ ...lookup, decode: value => value }, signal)
  const second = f.fetch.mock.calls[1]![0] as Request
  expect(second.method).toBe('GET')
  expect(new URL(second.url).search).toBe('?operation=setVenueLocationSharing')
  expect(second.headers.get('Idempotency-Key')).toBeNull()
  expect(f.fetch).toHaveBeenCalledTimes(2)
})

it('rejects malformed registered path and query values before canonical fetch', async () => {
  const f = setup(), signal = new AbortController().signal
  const landmark = locationRequests.landmark(event, 'zone')
  for (const suffix of ['\n', '\r', '\t', '%7E', '/hidden']) {
    await expect(f.transport.request({ ...landmark, path: landmark.path + suffix }, signal)).rejects.toMatchObject({ kind: 'invalid-request' })
  }
  const command = locationRequests.command(event, 'setVenueLocationSharing', key)
  await expect(f.transport.request({ ...command, query: { operation: 'resolveVenueMeetingContext' } }, signal)).rejects.toMatchObject({ kind: 'invalid-request' })
  expect(f.fetch).not.toHaveBeenCalled()
})
