import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { party, application } from '../../party-discovery/__tests__/fixtures/data'
import { giveaway, reservation } from '../../gift-discovery/__tests__/fixtures/data'
import { notice, post } from '../../event-feed/__tests__/fixtures'
import { deferred, error, event, id, page, setup, targetId, uid, update, user } from './fixtures/data'
import type { NotificationTarget } from '../model/contract'
const owners: ReturnType<typeof setup>[] = []
function fixture(type: NotificationTarget['target_type'], response: () => Response | Promise<Response>) {
  const h = setup(r => new URL(r.url).pathname.endsWith('/me/notifications') ? Response.json(page([update({ target: { target_type: type, target_id: targetId } })])) : response())
  owners.push(h); return h
}
const ready = async (h: ReturnType<typeof setup>) => { h.controller.start(); await vi.waitFor(() => expect(h.controller.read().phase).toBe('ready')) }
beforeEach(() => vi.stubGlobal('location', { origin: 'https://mcr.example' }))
afterEach(() => { owners.splice(0).forEach(h => h.dispose()); vi.unstubAllGlobals() })
it.each([
  ['PARTY', () => party({ party_id: targetId, event_id: event }), `/meet/parties/${targetId}`],
  ['APPLICATION', () => application({ application_id: targetId, event_id: event, party_id: uid(77) }), `/meet/parties/${uid(77)}`],
  ['GIVEAWAY', () => giveaway({ giveaway_id: targetId, event_id: event }), `/gifts/${targetId}`],
  ['RESERVATION', () => reservation({ reservation_id: targetId, event_id: event, recipient_id: user }), `/gifts/pickups/${targetId}`],
  ['POST', () => ({ post: { ...post, post_id: targetId, event_id: event }, comments: [], next_cursor: null }), `/feed/${targetId}`],
  ['NOTICE', () => ({ ...notice, notice_id: targetId, event_id: event }), `/notices/${targetId}`],
] as const)('fresh-authorizes %s before navigating a reviewed shell path and retains no projection', async (type, data, path) => {
  const h = fixture(type, () => Response.json(data())); await ready(h)
  expect(h.navigate).not.toHaveBeenCalled(); await h.controller.open(id)
  expect(h.navigate).toHaveBeenCalledWith({ path, scope: h.auth.privacy.capture() }); expect(h.fetch).toHaveBeenCalledTimes(2)
  expect(Object.keys(h.controller.read().rows[0]!)).not.toContain('text'); expect(JSON.stringify(h.controller.read())).not.toContain('rendezvous')
})
it.each([403, 404, 503])('denied/deleted/unavailable %i target never navigates or displays cached details', async status => {
  const h = fixture('PARTY', () => error(status, status === 403 ? 'PERMISSION_DENIED' : status === 404 ? 'RESOURCE_UNAVAILABLE' : 'DEPENDENCY_UNAVAILABLE'))
  await ready(h); await h.controller.open(id); expect(h.navigate).not.toHaveBeenCalled(); expect(h.controller.read().rows[0]!.target).toBeNull(); expect(h.controller.read().target.phase).toBe('unavailable')
})
it.each(['requester', 'recipient', 'foreign'] as const)('social-request link checks the current %s audience without retaining consent data', async audience => {
  const h = fixture('SOCIAL_REQUEST', () => Response.json({ request_id: targetId, event_id: event, requester_id: audience === 'requester' ? user : uid(88), recipient_id: audience === 'recipient' ? user : uid(89),
    consents: [{ platform: 'INSTAGRAM', requester_handle_version: 1, recipient_handle_version: 1, requester_consents: true }],
    created_at: '2026-11-04T00:00:00Z', expires_at: '2026-11-05T00:00:00Z', state: 'PENDING', revision: 1, intro_id: uid(77), intro_revision: 1, approved_platforms: [] }))
  await ready(h); await h.controller.open(id)
  if (audience === 'foreign') expect(h.navigate).not.toHaveBeenCalled()
  else expect(h.navigate).toHaveBeenCalledWith({ path: '/me/requests', scope: h.auth.privacy.capture() })
  expect(JSON.stringify(h.controller.read())).not.toContain('requester_handle_version')
})
it.each(['event', 'id', 'secret', 'draft', 'foreign-recipient'] as const)('rejects target breach %s before routing', async mode => {
  const h = mode === 'draft' ? fixture('GIVEAWAY', () => Response.json(giveaway({ giveaway_id: targetId, event_id: event, state: 'DRAFT' })))
    : mode === 'foreign-recipient' ? fixture('RESERVATION', () => Response.json(reservation({ reservation_id: targetId, event_id: event, recipient_id: uid(99) })))
      : fixture('PARTY', () => Response.json({ ...party({ party_id: targetId, event_id: event }), ...(mode === 'event' ? { event_id: uid(99) } : mode === 'id' ? { party_id: uid(99) } : { private_handle: '@secret' }) }))
  await ready(h); await h.controller.open(id); expect(h.navigate).not.toHaveBeenCalled(); expect(h.controller.read().target.phase).toBe('unavailable')
})
it('null and unsupported report target remain honestly unavailable without fetching moderator evidence', async () => {
  const h = fixture('REPORT', () => { throw new Error('Must not fetch evidence') }); await ready(h); await h.controller.open(id)
  expect(h.fetch).toHaveBeenCalledTimes(1); expect(h.navigate).not.toHaveBeenCalled(); expect(h.controller.read().target.phase).toBe('unavailable')
})
it('privacy invalidation while checking access fences even a late successful target', async () => {
  const late = deferred<Response>(); const h = fixture('PARTY', () => late.promise); await ready(h)
  const opening = h.controller.open(id); await vi.waitFor(() => expect(h.fetch).toHaveBeenCalledTimes(2))
  h.auth.session.replace(1, null); await opening; late.resolve(Response.json(party({ party_id: targetId, event_id: event }))); await Promise.resolve()
  expect(h.navigate).not.toHaveBeenCalled(); expect(h.controller.read().rows).toHaveLength(0)
})
it('explicit refresh cancels target authorization and late navigation', async () => {
  const late = deferred<Response>(); const h = fixture('PARTY', () => late.promise); await ready(h)
  const opening = h.controller.open(id); await vi.waitFor(() => expect(h.fetch).toHaveBeenCalledTimes(2))
  await h.controller.load(); await opening; late.resolve(Response.json(party({ party_id: targetId, event_id: event }))); await Promise.resolve()
  expect(h.navigate).not.toHaveBeenCalled(); expect(h.controller.read().target.phase).toBe('idle')
})
