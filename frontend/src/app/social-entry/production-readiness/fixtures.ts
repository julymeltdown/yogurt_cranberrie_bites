import { fixture, response, user, event, id } from '../../social-composition/__tests__/fixtures'
import type { SocialContext } from '../../social-composition/ports'
import type { IncumbentSocialWorld } from './index'
type Binding = NonNullable<ReturnType<IncumbentSocialWorld['read']>>
type Policy = NonNullable<ReturnType<NonNullable<Binding['socialPolicy']>['read']>>
export { deferred, id, user, event } from '../../social-composition/__tests__/fixtures'
export function success(request: Request) {
  if (new URL(request.url).pathname.endsWith('/me/social-handles')) return Response.json({ items: ['INSTAGRAM', 'X'].map(platform => ({ user_id: user, platform, version: 0, configured: false })) })
  if (new URL(request.url).pathname.endsWith('/me/social-requests')) return Response.json({ items: [], next_cursor: null })
  return response(request)
}
export function setup(handler: (request: Request) => Response | Promise<Response> = success) {
  const f = fixture(handler), context: SocialContext = { auth: f.auth, bootstrap: f.bootstrap, scope: f.scope }
  const worldListeners = new Set<() => void>(), policyListeners = new Set<() => void>(), release = new AbortController()
  const baseline = (): Policy => Object.freeze({ scope: f.scope, eventId: event, streamId: id(701), roomEpoch: id(702), policyRevision: 1 })
  let policy: ReturnType<typeof baseline> | null = null
  const policyPort = { read: () => policy, subscribe(fn: () => void) { policyListeners.add(fn); return () => { policyListeners.delete(fn) } } }
  const valid = { release: { scope: { userId: user, eventId: event, generation: f.scope.generation }, privacyScope: f.scope, bootstrap: f.bootstrap, signal: release.signal }, socialPolicy: policyPort }
  let binding: ReturnType<IncumbentSocialWorld['read']> = valid
  const world: IncumbentSocialWorld = { read: () => binding, subscribe(fn) { worldListeners.add(fn); return () => { worldListeners.delete(fn) } } }
  return { ...f, context, world, valid, baseline, release, policyPort,
    listenerCount: () => worldListeners.size + policyListeners.size,
    policy(value: typeof policy, notify = true) { policy = value; if (notify) for (const fn of Array.from(policyListeners)) fn() },
    binding(value: typeof binding) { binding = value; for (const fn of Array.from(worldListeners)) fn() },
  }
}
