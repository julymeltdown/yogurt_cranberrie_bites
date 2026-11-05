import { expect, it } from 'vitest'
import { readCallbackLocation, readResetLocation } from '../../model/callback'

it.each(['/world', '/feed', '/me', '/meet/people'])('accepts the agreed OAuth-only return path %s', target => {
  expect(readCallbackLocation(`?status=success&returnTo=${encodeURIComponent(target)}`, '')).toEqual({ kind: 'handoff', target })
})
it.each(['/friends', '/admin', '/auth/callback', '//evil.example', 'https://evil.example', '/feed?secret=x', '/me#secret', '/feed/../me', '/%2Ffeed', '/meet/people/'])('reduces unsafe OAuth return %s to the fixed fallback', target => {
  expect(readCallbackLocation(`?status=success&returnTo=${encodeURIComponent(target)}`, '')).toEqual({ kind: 'handoff', target: '/world' })
})
it.each(['?status=success&status=success', '?status=success&returnTo=/feed&returnTo=/me', '?status=unknown', '?status=success&code=PRIVATE', '?status=success&state=PRIVATE', '?status=success&error=access_denied', '?status=success&access_token=PRIVATE'])('rejects ambiguous or foreign callback %s', search => {
  expect(readCallbackLocation(search, '').kind).toBe('invalid-state')
})
it.each(['denied', 'error'])('never turns the backend status %s into a handoff', status => {
  expect(readCallbackLocation(`?status=${status}&returnTo=/feed`, '').kind).toBe(status === 'denied' ? 'denied' : 'unavailable')
})
it('rejects a credential fragment even beside a valid status and preserves reset interpretation', () => {
  expect(readCallbackLocation('?status=success', '#access_token=PRIVATE').kind).toBe('invalid-state')
  expect(readResetLocation('?status=success', '')).toMatchObject({ invalid: true, token: null })
})
