import { createSessionController } from '../../../features/auth/model/session'
import { isolatedCookiePolicy } from '../../../features/auth/__tests__/fixtures/data'
import { bootstrapFor, createFixturePorts } from '../../__tests__/fixtures/ports'
import { post, notice, id } from '../../../features/event-feed/__tests__/fixtures'
import introSource from '../../../../../contracts/fixtures/domain/public-intro.json'
import type { SocialAvailability, SocialAvailabilityPort, SocialOptions } from '../ports'
export { post, notice, id }
export const user = post.author.user_id, event = post.event_id
export const publicIntro = () => ({ ...structuredClone(introSource), event_id: event })
export const ownIntro = () => ({ intro_id: id(80), event_id: event, owner_id: user, text: 'Private owner draft', age_bounds: { minimum_age: null, maximum_age_exclusive: null }, expires_at: null, state: 'DRAFT', revision: 1, created_at: '2026-11-01T00:00:00Z' })
export function response(r: Request) {
  const path = new URL(r.url).pathname
  if (path.endsWith('/unread-count')) return Response.json({ actor_id: user, event_id: event, unread_total: 37, as_of: '2026-11-04T00:00:00Z' })
  if (path.endsWith('/notifications')) return Response.json({ items: [{ notification_id: id(90), event_id: event, kind: 'FEED', target: { target_type: 'POST', target_id: post.post_id }, created_at: '2026-11-01T00:00:00Z', read_at: null, revision: 1 }], next_cursor: null })
  if (path.endsWith('/me/age-status')) return Response.json({ user_id: user, event_id: event, status: 'UNKNOWN', verified_at: null, expires_at: null, verification_available: false })
  if (path.endsWith('/me/intro')) return Response.json(ownIntro())
  if (path.endsWith('/intros')) return Response.json({ items: [publicIntro()], next_cursor: null })
  if (path.includes('/people/')) return Response.json(publicIntro().author)
  if (path.endsWith('/notices')) return Response.json({ items: [notice], next_cursor: null })
  if (path.includes('/notices/')) return Response.json(notice)
  if (path.endsWith('/feed')) return Response.json({ items: [post], next_cursor: null })
  if (path.includes('/posts/')) return Response.json({ post, comments: [], next_cursor: null })
  return Response.json({ code: 'RESOURCE_UNAVAILABLE', message: 'private fixture detail', details: { reason: 'RESOURCE_UNAVAILABLE' } }, { status: 404 })
}
export function fixture(handler: (r: Request) => Response | Promise<Response> = response) {
  const requests: Request[] = [], navigations: string[] = []
  const auth = createSessionController({ eventId: event, cookiePolicy: isolatedCookiePolicy(), fetch: async r => { requests.push((r as Request).clone()); return handler(r as Request) } })
  auth.session.replace(0, { userId: user, eventId: event, accessToken: 'SYNTHETIC_SOCIAL' })
  const scope = auth.privacy.capture(), bootstrap = bootstrapFor({ userId: user, eventId: event, generation: scope.generation })
  let lease: SocialAvailability | null = { scope, bootstrap, services: { feed: true, people: true, intro: true, age: true, inbox: true } }
  const listeners = new Set<() => void>()
  const availability: SocialAvailabilityPort = { read: () => lease, subscribe(f) { listeners.add(f); return () => { listeners.delete(f) } } }
  const world = createFixturePorts()
  const ports = { ...world.ports, auth, session: auth.session }
  const options: SocialOptions = { auth, bootstrap, scope, availability, navigate: d => { if (auth.privacy.isCurrent(d.scope)) navigations.push(d.path) } }
  return { auth, scope, bootstrap, options, requests, navigations, ports, stats: world.stats,
    availability, listenerCount: () => listeners.size, setLease(value: SocialAvailability | null) { lease = value; for (const f of Array.from(listeners)) f() }, dispose: () => auth.dispose() }
}
export function deferred<T>() { let resolve!: (value: T) => void; const promise = new Promise<T>(r => { resolve = r }); return { promise, resolve } }
