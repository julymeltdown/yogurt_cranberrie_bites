import { describe, expect, it } from 'vitest'
import unauthorized from '../../../../../contracts/fixtures/core/auth-error-unauthenticated.json'
import limited from '../../../../../contracts/fixtures/core/auth-error-rate-limited.json'
import conflict from '../../../../../contracts/fixtures/core/error-version-conflict.json'
import cooldown from '../../../../../contracts/fixtures/core/error-mural-cooldown.json'
import origin from '../../../../../contracts/fixtures/core/error-origin-not-allowed.json'
import intent from '../../../../../contracts/fixtures/core/error-browser-intent-required.json'
import { MAX_RETRY_AFTER_MS, parseCoreDetails, parseRetryAfter, parseServerError, TransportError } from '../errors'

const envelope = (details: unknown) => ({ code: 'GATEWAY_409', message: 'Translated arbitrary message.', details })

describe('bounded inherited auth and strict core/origin envelopes', () => {
  it('keeps generic null auth details distinct from typed core reasons', () => {
    expect(parseServerError(unauthorized, 'auth', 401)).toEqual({ envelope: unauthorized, core: null, reason: null })
    expect(parseServerError(limited, 'auth', 429)?.reason).toBeNull()
    expect(parseServerError(unauthorized, 'core', 401)).toBeNull()
    expect(parseServerError(conflict, 'core', 409)).toMatchObject({ core: { reason: 'VERSION_CONFLICT' }, reason: 'VERSION_CONFLICT',
      envelope: { code: conflict.code, message: conflict.message, traceId: conflict.traceId, timestamp: conflict.timestamp } })
    expect(parseServerError(cooldown, 'core', 429)?.core).toEqual(cooldown.details)
  })

  it.each([origin, intent])('only accepts strict OriginErrorResponse at 403 ($details.reason)', value => {
    for (const family of ['auth', 'core'] as const) {
      expect(parseServerError(value, family, 403)?.reason).toBe(value.details.reason)
      expect(parseServerError(value, family, 401)).toBeNull()
      expect(parseServerError({ ...value, extra: 'private' }, family, 403)).toBeNull()
      expect(parseServerError({ ...value, details: { ...value.details, extra: true } }, family, 403)).toBeNull()
      expect(parseServerError({ ...value, timestamp: 'November 3, 2026' }, family, 403)).toBeNull()
    }
  })

  it('copies and freezes bounded generic auth details without inventing a machine reason', () => {
    const details = { reason: 'SOME_INHERITED_DETAIL', safe: [null, true, 1, 'text', { context: 'generic' }] }
    const parsed = parseServerError(envelope(details), 'auth', 409)!
    details.safe.push('later')
    expect(parsed.envelope.details?.safe).toHaveLength(5)
    expect(parsed.reason).toBeNull(); expect(parsed.core).toBeNull()
    expect(Object.isFrozen(parsed.envelope.details)).toBe(true)
    expect(Object.isFrozen(parsed.envelope.details?.safe)).toBe(true)
    expect(parseServerError({ code: 'generic', message: 'No details' }, 'auth', 403)?.envelope.details).toBeNull()
  })

  it.each([
    null, [], {}, { code: '', message: 'x' }, { code: 'x'.repeat(65), message: 'x' },
    { code: 'x', message: 'x'.repeat(513) }, { code: 'x', message: 'x', traceId: 'x'.repeat(129) },
    { code: 'x', message: 'x', traceId: 1 }, { code: 'x', message: 'x', timestamp: 'bad date' },
    envelope(['bad']), envelope({ nested: { level: { level: { level: { level: 'too deep' } } } } }),
    envelope(JSON.parse('{"__proto__":{"leak":true}}')),
    envelope({ secret: 'x'.repeat(513) }), envelope({ values: Array.from({ length: 33 }, () => 0) }),
  ])('rejects malformed or unbounded auth input %#', value => {
    expect(parseServerError(value, 'auth', 409)).toBeNull()
  })

  it.each([
    null, {}, { reason: 'UNKNOWN' }, { reason: 'VERSION_CONFLICT', current_revision: 0 },
    { reason: 'VERSION_CONFLICT', current_revision: 1.1 }, { reason: 'VERSION_CONFLICT', secret: 'private' },
    { reason: 'RATE_LIMITED', retry_after_ms: 0 }, { reason: 'MURAL_COOLDOWN', retry_after_ms: MAX_RETRY_AFTER_MS + 1 },
    { reason: 'INVALID_INPUT', current_revision: 1 },
  ])('rejects unsafe core detail shape %#', details => {
    expect(parseCoreDetails(details)).toBeNull()
    expect(parseServerError(envelope(details), 'core', 409)).toBeNull()
  })

  it('accepts only allowed fields for revision, cooldown and simple core reasons', () => {
    expect(parseCoreDetails({ reason: 'VERSION_CONFLICT', current_revision: 3 })).toEqual({ reason: 'VERSION_CONFLICT', current_revision: 3 })
    expect(parseCoreDetails({ reason: 'RATE_LIMITED', retry_after_ms: 2000 })).toEqual({ reason: 'RATE_LIMITED', retry_after_ms: 2000 })
    expect(parseCoreDetails({ reason: 'RESOURCE_UNAVAILABLE' })).toEqual({ reason: 'RESOURCE_UNAVAILABLE' })
    expect(parseServerError({ ...envelope({ reason: 'INVALID_INPUT' }), extra: true }, 'core', 400)).toBeNull()
    expect(parseServerError({ ...envelope({ reason: 'INVALID_INPUT' }), timestamp: '2026-11-03T00:00:00+00:00' }, 'core', 400)).toBeNull()
  })

  it('preserves opaque nullable correlation IDs without treating them as UUIDs', () => {
    expect(parseServerError({ ...conflict, traceId: 'edge-correlation.abc' }, 'core', 409)?.envelope.traceId).toBe('edge-correlation.abc')
    expect(parseServerError({ ...conflict, traceId: null, timestamp: null }, 'core', 409)?.envelope).toMatchObject({ traceId: null, timestamp: null })
  })

  it('does not retain raw request/response/cause objects in transport errors', () => {
    const error = new TransportError('http', { status: 403, server: parseServerError(origin, 'auth', 403) })
    expect(error.code).toBe(origin.code)
    expect(error).not.toHaveProperty('response'); expect(error).not.toHaveProperty('request'); expect(error).not.toHaveProperty('cause')
    expect(error.message).toBe('Request failed (HTTP 403).')
    expect(new TransportError('session-changed').code).toBe('SESSION_CHANGED')
  })
})

describe('Retry-After is a bounded hint and never an instruction to retry', () => {
  it.each([
    [null, null], ['', null], ['-1', null], ['1.5', null], ['garbage', null], ['2026', 2026000],
    ['0', 0], ['2', 2000], [' 3 ', 3000], ['9'.repeat(100), MAX_RETRY_AFTER_MS], ['9'.repeat(129), null],
  ] as const)('parses %s to %s', (value, expected) => expect(parseRetryAfter(value)).toBe(expected))

  it('accepts HTTP-date, clamps expired/future dates, rejects loose Date.parse forms', () => {
    const now = Date.parse('2026-11-03T00:00:00Z')
    expect(parseRetryAfter('Tue, 03 Nov 2026 00:00:05 GMT', now)).toBe(5000)
    expect(parseRetryAfter('Tue, 03 Nov 2026 00:00:00 GMT', now + 1)).toBe(0)
    expect(parseRetryAfter('Wed, 04 Nov 2026 00:00:01 GMT', now)).toBe(MAX_RETRY_AFTER_MS)
    expect(parseRetryAfter('2026-11-03T00:00:05Z', now)).toBeNull()
    expect(parseRetryAfter('Tue, 03 Foo 2026 00:00:05 GMT', now)).toBeNull()
  })
})
