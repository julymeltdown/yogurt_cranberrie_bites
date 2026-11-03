import { expect, it } from 'vitest'
import { readCallbackLocation, readResetLocation } from '../model/callback'
const token = 'A'.repeat(43)
it('accepts only the source-bound fragment token handoff without reflecting credentials', () => {
  expect(readResetLocation('?returnTo=%2Ffeed', `#token=${token}`)).toEqual({ target: '/feed', token, invalid: false })
  expect(readResetLocation('', '')).toEqual({ target: '/world', token: null, invalid: false })
})
it.each([['?token=secret', ''], ['', '#token=short'], ['', `#token=${token}&token=${token}`], ['', `#token=${token}&email=private`],
  ['', '#access_token=secret'], ['', '#token=%41'.repeat(43)], ['?email=private', `#token=${token}`]])('does not import an unreviewed token URL %j %j', (search, hash) => {
  expect(readResetLocation(search, hash)).toMatchObject({ token: null, invalid: true })
})
it.each(['https://evil.example', '//evil.example', '/\\evil.example', '/feed%2f..', '/feed/../me', '/auth/callback', '/login', '/admin',
  '/world?token=secret', '/world#secret', '/world\n', '/feed?returnTo=https://evil.example', '/feed?token=secret'])('rejects hostile return %j for both public flows', value => {
  const query = `?returnTo=${encodeURIComponent(value)}`
  expect(readResetLocation(query, '').target).toBe('/world'); expect(readCallbackLocation(query, '').target).toBe('/world')
})
it('rejects duplicate return targets rather than selecting one', () => {
  const q = '?returnTo=%2Ffeed&returnTo=%2Fme'
  expect(readResetLocation(q, '')).toMatchObject({ invalid: true, target: '/world' })
  expect(readCallbackLocation(q, '')).toEqual({ kind: 'invalid-state', target: '/world' })
})
it('presents denial using a fixed local marker and ignores provider descriptions/URLs', () => {
  expect(readCallbackLocation('?error=access_denied&state=private&error_description=private&error_uri=https://evil.example&returnTo=%2Ffeed', ''))
    .toEqual({ kind: 'denied', target: '/feed' })
})
it.each(['?code=secret&state=secret', '?access_token=secret', '?id_token=secret', '?error=invalid_state', '?state=secret',
  '?error=access_denied&error=other', '?error=access_denied&code=secret', '?error=access_denied&state=a&state=b'])('never fabricates OAuth authority for %j', query => {
  expect(readCallbackLocation(query, '').kind).toBe('invalid-state')
})
it('bounds incoming URL material before parsing', () => {
  expect(readResetLocation(`?${'a'.repeat(16_384)}`, '')).toEqual({ target: '/world', token: null, invalid: true })
  expect(readCallbackLocation('', `#${'a'.repeat(16_384)}`)).toEqual({ kind: 'invalid-state', target: '/world' })
})
it('defaults to provider unavailable and rejects every callback fragment without credential parsing', () => {
  expect(readCallbackLocation('', '')).toEqual({ kind: 'unavailable', target: '/world' })
  expect(readCallbackLocation('', '#access_token=secret').kind).toBe('invalid-state')
})
