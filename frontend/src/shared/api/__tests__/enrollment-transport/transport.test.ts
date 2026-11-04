import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { createHttpClient, createMemorySession, type HttpRequest } from '../../http'
const paths = ['policy','attempts','signup','verify','resend','admission','consent'] as const
const protectedPath = (path: string) => path === 'admission' || path === 'consent'
const method = (path: string) => path === 'policy' || path === 'admission' ? 'GET' as const : 'POST' as const
beforeEach(() => vi.stubGlobal('location', { origin: 'https://mcr.example' }))
afterEach(() => vi.unstubAllGlobals())
function setup() {
  const requests: Request[] = [], session = createMemorySession()
  session.replace(0, { userId: '33333333-3333-4333-8333-333333333333', eventId: null, accessToken: 'canonical' })
  const http = createHttpClient({ session, fetch: async input => { requests.push((input as Request).clone()); return Response.json({ ok: true }) } })
  return { requests, session, http }
}
function request(path: string): HttpRequest<unknown> { return { path: `/api/auth/enrollment/${path}`, method: method(path),
  access: protectedPath(path) ? 'protected' : 'public', errorFamily: 'auth', decode: value => value,
  ...(method(path) === 'POST' ? { body: { json: { idempotencyKey: '22222222-2222-4222-8222-222222222222' } } } : {}) } }
it.each(paths)('includes bound cookie via canonical transport for exactly %s; no secret/header aliases', async path => {
  const f = setup(); await f.http.request(request(path)); const r = f.requests[0]!
  expect(r.credentials).toBe('include'); expect(r.cache).toBe('no-store'); expect(r.referrerPolicy).toBe('no-referrer'); expect(r.redirect).toBe('error')
  expect(r.headers.get('Idempotency-Key')).toBeNull(); expect(r.headers.get('Authorization')).toBe(protectedPath(path) ? 'Bearer canonical' : null)
  expect(r.headers.get('X-MCR-Intent')).toBe(method(path) === 'POST' ? 'browser' : null)
  if (method(path) === 'POST') expect(await r.json()).toEqual({ idempotencyKey: '22222222-2222-4222-8222-222222222222' })
})
it.each(paths)('rejects method/access/refresh/error family/header/top-level-idempotency mismatch for %s before dispatch', async path => {
  const f = setup(), r = request(path)
  for (const patch of [{ method: r.method === 'GET' ? 'POST' as const : 'GET' as const },
    { access: r.access === 'public' ? 'protected' as const : 'public' as const }, { refresh: 'once' as const },
    { errorFamily: 'core' as const }, { idempotencyKey: '22222222-2222-4222-8222-222222222222' }, { headers: { 'Idempotency-Key': 'private' } }]) {
    await expect(f.http.request({ ...r, ...patch })).rejects.toMatchObject({ kind: 'invalid-request' })
  }
  expect(f.requests).toHaveLength(0)
})
it('retains existing same-origin capability transport and rejects unregistered enrollment routes', async () => {
  const f = setup(); await f.http.request({ path: '/api/auth/capabilities', method: 'GET', access: 'public', errorFamily: 'core', decode: value => value })
  expect(f.requests[0]!.credentials).toBe('same-origin')
  await expect(f.http.request(request('publish'))).rejects.toMatchObject({ kind: 'invalid-request' })
})
it('requires canonical identity for protected admission with no anonymous refresh or synthesized actor', async () => {
  const f = setup(); f.session.replace(f.session.read().generation, null)
  await expect(f.http.request(request('admission'))).rejects.toMatchObject({ kind: 'authentication-required' }); expect(f.requests).toHaveLength(0)
})
