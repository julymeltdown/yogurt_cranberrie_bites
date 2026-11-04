import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { fixture, response, deferred, user, event, id } from '../../social-composition/__tests__/fixtures'
import { createIN03Availability } from '../availability'
import type { SocialContext } from '../../social-composition/ports'

function successful(request: Request) {
  const path = new URL(request.url).pathname
  if (path.endsWith('/me/social-handles')) return Response.json({ items: ['INSTAGRAM', 'X'].map(platform => ({ user_id: user, platform, version: 0, configured: false })) })
  if (path.endsWith('/me/social-requests')) return Response.json({ items: [], next_cursor: null })
  return response(request)
}
function setup(handler: (request: Request) => Response | Promise<Response> = successful) {
  const value = fixture(handler)
  const context: SocialContext = { auth: value.auth, bootstrap: value.bootstrap, scope: value.scope }
  const port = createIN03Availability(context)!
  return { ...value, context, port }
}
const all = { feed: true, people: true, intro: true, age: true, inbox: true, requests: true }
beforeEach(() => { vi.stubGlobal('location', { origin: 'https://mcr.example' }) })
afterEach(() => { vi.unstubAllGlobals() })

describe('actual scoped read readiness (synthetic transport only)', () => {
  it('starts unavailable despite capable C03, then qualifies only validated reads without retaining bodies', async () => {
    const value = setup()
    try {
      expect(value.requests).toHaveLength(0)
      expect(Object.values(value.port.read()!.services)).not.toContain(true)
      await value.port.prepare()
      expect(value.requests).toHaveLength(7)
      expect(value.requests.every(request => request.method === 'GET')).toBe(true)
      expect(value.port.read()).toEqual({ bootstrap: value.bootstrap, scope: value.scope, services: all })
      expect(value.port.read()!.bootstrap).toBe(value.bootstrap)
      // UNKNOWN age status makes its status page readable, never a verified sharing claim.
      expect(value.port.read()!.services.age).toBe(true)
    } finally { value.port.dispose(); value.dispose() }
  })
  it('keeps a malformed or failed service false while independent successful services remain readable', async () => {
    const value = setup(request => new URL(request.url).pathname.endsWith('/feed') ? Response.json({ items: [{ event_id: id(999) }], next_cursor: null }) : successful(request))
    try { await value.port.prepare(); expect(value.port.read()!.services).toEqual({ ...all, feed: false }) }
    finally { value.port.dispose(); value.dispose() }
  })
  it('requires both own handles and incoming request reads for requests readiness', async () => {
    const value = setup(request => new URL(request.url).pathname.endsWith('/me/social-handles')
      ? Response.json({ items: ['INSTAGRAM', 'X'].map(platform => ({ user_id: id(999), platform, version: 0, configured: false })) }) : successful(request))
    try { await value.port.prepare(); expect(value.port.read()!.services.requests).toBe(false) }
    finally { value.port.dispose(); value.dispose() }
  })
  it('respects explicit C03 service denials without dispatching their probes', async () => {
    const value = setup()
    const bootstrap = { ...value.bootstrap, capabilities: { ...value.bootstrap.capabilities,
      feed: { available: false, reason: 'FEATURE_DISABLED' } as const, social_sharing: { available: false, reason: 'FEATURE_DISABLED' } as const } }
    const port = createIN03Availability({ ...value.context, bootstrap })!
    try {
      await port.prepare()
      expect(port.read()!.services).toEqual({ ...all, feed: false, requests: false })
      expect(value.requests.some(request => /\/(feed|social-handles|social-requests)$/.test(new URL(request.url).pathname))).toBe(false)
    } finally { port.dispose(); value.port.dispose(); value.dispose() }
  })
  it.each(['userId', 'eventId', 'generation', 'policyGeneration', 'gate'] as const)('rejects a stale %s scope before dispatch', async field => {
    const value = setup()
    try {
      const changes = { userId: id(999), eventId: id(998), generation: value.scope.generation + 1, policyGeneration: value.scope.policyGeneration + 1, gate: 'suspended' as const }
      expect(createIN03Availability({ ...value.context, scope: { ...value.scope, [field]: changes[field] } })).toBeNull()
      expect(value.requests).toHaveLength(0)
    } finally { value.port.dispose(); value.dispose() }
  })
  it('rejects a bootstrap for a different event or participant', () => {
    const value = setup()
    try {
      expect(createIN03Availability({ ...value.context, bootstrap: { ...value.bootstrap, participant: { userId: id(999), eventId: event } } })).toBeNull()
      expect(createIN03Availability({ ...value.context, bootstrap: { ...value.bootstrap, event: { ...value.bootstrap.event, eventId: id(999) } } })).toBeNull()
    } finally { value.port.dispose(); value.dispose() }
  })
  it('withdraws old readiness before refreshing and does not preserve a failed prior success', async () => {
    let failed = false
    const value = setup(request => failed && new URL(request.url).pathname.endsWith('/feed') ? Response.json({}, { status: 503 }) : successful(request))
    try {
      await value.port.prepare(); expect(value.port.read()!.services.feed).toBe(true)
      failed = true
      const next = value.port.prepare()
      expect(Object.values(value.port.read()!.services)).not.toContain(true)
      await next; expect(value.port.read()!.services.feed).toBe(false)
    } finally { value.port.dispose(); value.dispose() }
  })
  it('policy withdrawal wins over a delayed real-client response and notifies observers', async () => {
    const held = deferred<Response>()
    const value = setup(request => new URL(request.url).pathname.endsWith('/feed') ? held.promise : successful(request))
    let changes = 0
    const stop = value.port.subscribe(() => { changes++ })
    try {
      const pending = value.port.prepare()
      await Promise.resolve()
      value.auth.privacy.changePolicy(value.scope, 'block')
      expect(value.port.read()).toBeNull()
      held.resolve(Response.json({ items: [], next_cursor: null }))
      await pending
      expect(value.port.read()).toBeNull(); expect(changes).toBeGreaterThan(0)
    } finally { stop(); value.port.dispose(); value.dispose() }
  })
  it('honors caller abort without starting or resurrecting a port', async () => {
    const value = setup(), controller = new AbortController()
    try {
      controller.abort(); await value.port.prepare(controller.signal)
      expect(value.port.read()).toBeNull(); expect(value.requests).toHaveLength(0)
      await value.port.prepare(); expect(value.requests).toHaveLength(0)
    } finally { value.port.dispose(); value.dispose() }
  })
  it('withdraws prepared readiness when its caller lifetime aborts later', async () => {
    const value = setup(), controller = new AbortController()
    try {
      await value.port.prepare(controller.signal); expect(value.port.read()!.services.feed).toBe(true)
      controller.abort(); expect(value.port.read()).toBeNull()
    } finally { value.port.dispose(); value.dispose() }
  })
})
