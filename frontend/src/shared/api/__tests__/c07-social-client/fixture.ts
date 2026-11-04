/** Isolated fixture mode ONLY. No import from the real entry point. Synthetic transport is not a server. */
import approved from '../../../../../../contracts/fixtures/domain/approved-social-request.json'
import intro from '../../../../../../contracts/fixtures/domain/public-intro.json'
import post from '../../../../../../contracts/fixtures/domain/public-post.json'
import notification from '../../../../../../contracts/fixtures/domain/notification-safe.json'
import { createSessionController } from '../../../../features/auth/model/session'
import { createSocialSharingController } from '../../../../features/social-sharing/model/controller'
import { createSocialOwner } from '../../../../app/social-composition/owner'
import { canonicalSharingRequests } from '../../../../app/social-composition/sharing'
import { CAPABILITIES, projectCapabilities } from '../../../../app/capabilities'
import type { ShellBootstrap } from '../../../../app/ports'
import type { SocialAvailabilityPort } from '../../../../app/social-composition/ports'
import type { SocialRequest } from '../../../../features/social-sharing/model/contract'

export const request = approved as SocialRequest
export const eventId = request.event_id, requester = request.requester_id, recipient = request.recipient_id
export const epoch = Date.parse('2026-11-07T09:00:00Z')
export const id = (n: number) => `00000000-0000-4000-8000-${n.toString(16).padStart(12, '0')}`
export const secret = () => ({ request_id: request.request_id, platform: 'INSTAGRAM', owner_id: recipient,
  handle_version: 3, handle: 'synthetic_private_name', grant_revision: 1, expires_at: request.expires_at })
export function fixtureWorld(seed = 209) {
  let current: SocialRequest = { ...structuredClone(request), state: 'PENDING', revision: 1, approved_platforms: [] }
  const calls: { user: string; method: string; path: string; body: unknown; key: string | null }[] = []
  let sequence = seed, now = epoch
  let override: ((r: Request, user: string) => Response | Promise<Response> | undefined) | undefined
  const created: ReturnType<typeof actor>[] = []
  function actor(user: string) {
    const auth = createSessionController({ eventId, qualifiedRefresh: false,
      cookiePolicy: { available: false, onInvalidate: () => () => {}, dispose() {}, async run() { throw new Error('Fixture mode never performs cookie login') } },
      fetch: async input => {
        const r = input as Request, path = new URL(r.url).pathname
        if (r.cache !== 'no-store' || r.credentials !== 'same-origin') throw new Error('Fixture privacy transport mismatch')
        calls.push({ user, method: r.method, path, body: ['GET', 'HEAD'].includes(r.method) ? null : await r.clone().json(), key: r.headers.get('Idempotency-Key') })
        const replaced = override?.(r, user); if (replaced) return replaced
        if (path.endsWith('/social-handles')) return Response.json({ items: [
          { user_id: user, platform: 'INSTAGRAM', version: user === requester ? 2 : 3, configured: true },
          { user_id: user, platform: 'X', version: 0, configured: false }] })
        if (path.endsWith('/sharing-offer')) return Response.json({ intro_id: request.intro_id, intro_revision: 1, recipient_id: recipient, available_handles: [{ platform: 'INSTAGRAM', version: 3 }] })
        if (path.endsWith('/reveal')) return Response.json(secret())
        if (path.endsWith('/decision')) current = { ...current, state: 'APPROVED', approved_platforms: ['INSTAGRAM'], revision: current.revision + 1 }
        if (path.endsWith('/revoke')) current = { ...current, state: 'REVOKED', revision: current.revision + 1 }
        if (path.includes('/social-requests')) return Response.json(path.endsWith('/social-requests') && r.method === 'GET' ? { items: [current], next_cursor: null } : current)
        if (path.endsWith('/intros')) return Response.json({ items: [intro], next_cursor: null })
        if (path.endsWith('/feed')) return Response.json({ items: [post], next_cursor: null })
        if (path.endsWith('/notices')) return Response.json({ items: [], next_cursor: null })
        if (path.endsWith('/unread-count')) return Response.json({ actor_id: user, event_id: eventId, unread_total: 1, as_of: '2026-11-07T09:00:00Z' })
        if (path.endsWith('/notifications')) return Response.json({ items: [notification], next_cursor: null })
        if (path.endsWith('/comments') && r.method === 'POST') return Response.json({ comment_id: id(++sequence), post_id: post.post_id, author: { ...post.author, user_id: user }, text: 'C07 fixture reply', created_at: '2026-11-07T09:00:00Z', revision: 1 })
        if (path.includes('/posts/')) return Response.json({ post, comments: [], next_cursor: null })
        return Response.json({ code: 'RESOURCE_UNAVAILABLE', message: 'Unmodelled fixture route', details: { reason: 'RESOURCE_UNAVAILABLE' } }, { status: 404 })
      },
    })
    // This is the ONLY synthetic identity seam. Real admission imports neither this file nor replace().
    auth.session.replace(0, { userId: user, eventId, accessToken: 'C07_SYNTHETIC_ONLY' })
    const scope = auth.privacy.capture()
    const bootstrap: ShellBootstrap = { contractVersion: '0.1.0', event: { eventId, title: 'C07 isolated fixture', phase: 'OPEN' },
      participant: { userId: user, eventId }, worldAvailable: false,
      capabilities: projectCapabilities(Object.fromEntries(CAPABILITIES.map(key => [key, { available: true, reason: null }]))) }
    const availability: SocialAvailabilityPort = { read: () => ({ scope, bootstrap, services: { feed: true, people: true, inbox: true, requests: true } }), subscribe: () => () => {} }
    const owner = createSocialOwner({ auth, scope, bootstrap, availability, requests: canonicalSharingRequests, navigate: () => {} })
    const sharing = createSocialSharingController({ auth, access: { read: () => ({ available: true, readOnly: false }), subscribe: () => () => {} }, sensitiveRead: auth.requestSensitive, now: () => now })
    sharing.start()
    return { auth, owner, sharing, scope, bootstrap, availability, dispose() { sharing.dispose(); owner.dispose(); auth.dispose() } }
  }
  return { calls, actor(user = requester) { const next = actor(user); created.push(next); return next },
    approve() { current = structuredClone(request) },
    override(handler: typeof override) { override = handler },
    setNow(value: number) { now = value; created.forEach(item => item.sharing.expire()) },
    dispose() { created.forEach(item => item.dispose()) } }
}
