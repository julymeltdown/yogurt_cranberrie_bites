import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { fixtureWorld, request, requester, recipient, secret, id } from './fixture'
import { readJSON } from '../../../../../e2e/real/social/cohort.mjs'
import { createSafetyController } from '../../../../features/safety/controller'
import { createSocialOwner } from '../../../../app/social-composition/owner'
import { privateQueryKey } from '../../../query/keys'

let world: ReturnType<typeof fixtureWorld>
beforeEach(() => {
  vi.stubGlobal('location', { origin: 'https://mcr.example' })
  let sequence = 209
  vi.spyOn(globalThis.crypto, 'randomUUID').mockImplementation(() => id(++sequence) as ReturnType<Crypto['randomUUID']>)
  world = fixtureWorld()
})
afterEach(() => { world.dispose(); vi.restoreAllMocks(); vi.unstubAllGlobals() })
it('deterministic two-principal discovery/feed/comment/consent/decision/reveal/revoke uses canonical consumers', async () => {
  const a = world.actor(), b = world.actor(recipient)
  a.owner.people.start(); await a.owner.people.load()
  expect(a.owner.people.read().page.data?.items[0]?.intro_id).toBe(request.intro_id)
  a.owner.feed.start(); await a.owner.feed.loadFeed()
  const post = readJSON('contracts/fixtures/domain/public-post.json')
  expect(a.owner.feed.read().feed.data?.items[0]?.post_id).toBe(post.post_id)
  a.owner.thread.start(); a.owner.thread.select(post.post_id)
  await vi.waitFor(() => expect(a.owner.thread.read().detail.phase).toBe('ready'))
  a.owner.thread.setReplyText('C07 fixture reply'); expect(await a.owner.thread.submitComment()).toBe(true)
  expect(world.calls.find(c => c.path.endsWith('/comments') && c.method === 'POST')?.body).toEqual({ text: 'C07 fixture reply', post_revision: 1 })
  await a.sharing.loadHandles(); await a.sharing.loadOffer(request.intro_id)
  expect(await a.sharing.createRequest()).toBe(false)
  expect(world.calls.filter(c => c.method === 'POST' && c.path.endsWith('/social-requests'))).toHaveLength(0)
  a.sharing.setRequesterConsent('INSTAGRAM', true); expect(await a.sharing.createRequest()).toBe(true)
  const create = world.calls.find(c => c.method === 'POST' && c.path.endsWith('/social-requests'))!
  expect(create.key).toMatch(/^[0-9a-f-]{36}$/)
  expect(create.body).toEqual({ recipient_id: recipient, intro_id: request.intro_id, intro_revision: 1,
    consents: [{ platform: 'INSTAGRAM', requester_handle_version: 2, recipient_handle_version: 3, requester_consents: true }] })
  await b.sharing.loadRequest(request.request_id); b.sharing.setDecisionPlatform('INSTAGRAM', true)
  expect(await b.sharing.decide(true)).toBe(false)
  b.sharing.setRecipientConsent(true); expect(await b.sharing.decide(true)).toBe(true)
  expect(world.calls.find(c => c.path.endsWith('/decision'))?.body).toEqual({ expected_revision: 1, decision: 'APPROVED', approved_platforms: ['INSTAGRAM'], recipient_consents: true })
  await a.sharing.loadRequest(request.request_id)
  expect(a.sharing.read().secret.data).toBeNull()
  expect(world.calls.some(c => c.path.endsWith('/reveal'))).toBe(false)
  expect(await a.sharing.revealHandle('INSTAGRAM')).toBe(true)
  expect(a.sharing.read().secret.data?.handle).toBe('synthetic_private_name')
  a.owner.inbox.start(); await a.owner.inbox.load(); await a.owner.inbox.loadUnreadCount()
  expect(a.owner.inbox.read().rows).toHaveLength(1)
  expect(a.owner.inbox.read().count.total).toBe(1)
  expect(await a.sharing.revokeRequest()).toBe(true)
  expect(a.sharing.read().secret.data).toBeNull()
  expect(await a.sharing.revealHandle('INSTAGRAM')).toBe(false)
  expect(world.calls.filter(c => c.path.endsWith('/reveal'))).toHaveLength(1)
})
it.each(['block', 'consent-revoked', 'consent-expired', 'suspended', 'event-closed'] as const)('synthetic %s cause clears canonical caches and rejects a late sensitive read', async reason => {
  world.approve(); const a = world.actor(); await a.sharing.loadRequest(request.request_id)
  let resolve!: (r: Response) => void
  const delayed = new Promise<Response>(r => { resolve = r })
  world.override(r => r.url.endsWith('/reveal') ? delayed : undefined)
  const flight = a.sharing.revealHandle('INSTAGRAM')
  await vi.waitFor(() => expect(world.calls.some(c => c.path.endsWith('/reveal'))).toBe(true))
  a.auth.queryClient.setQueryData(privateQueryKey({ ...a.scope, userId: requester }, 'social-reveal', [request.request_id]), { handle: 'synthetic_cache' })
  a.owner.notifyPolicy({ scope: a.scope, reason })
  expect(a.owner.isDisposed()).toBe(true)
  expect(a.auth.queryClient.getQueryCache().getAll()).toHaveLength(0)
  resolve(Response.json(secret())); await flight
  expect(a.sharing.read().secret.data).toBeNull()
})
it('foreign policy scope cannot invalidate the current owner; no remote event protocol is invented', () => {
  const a = world.actor()
  a.owner.notifyPolicy({ scope: { ...a.scope, userId: id(999) }, reason: 'block' })
  expect(a.owner.isDisposed()).toBe(false)
  const wire = readJSON('contracts/realtime/v1/world.schema.json')
  expect(wire.$defs.Revoke.properties.reason.enum).not.toContain('CONSENT_REVOKED')
})
it.each([401, 403, 404, 409, 503])('denied HTTP %s sensitive read does not replay, refresh or expose a name', async status => {
  world.approve(); const a = world.actor(); await a.sharing.loadRequest(request.request_id)
  world.override(r => r.url.endsWith('/reveal') ? Response.json({ code: 'PERMISSION_DENIED', message: 'Denied', details: { reason: 'PERMISSION_DENIED' } }, { status }) : undefined)
  expect(await a.sharing.revealHandle('INSTAGRAM')).toBe(false)
  expect(a.sharing.read().secret.data).toBeNull()
  expect(world.calls.filter(c => c.path.endsWith('/reveal'))).toHaveLength(1)
  expect(world.calls.some(c => c.path.includes('/refresh'))).toBe(false)
})
it('expiry and wrong response context fail closed without widening a grant', async () => {
  world.approve(); const a = world.actor(); await a.sharing.loadRequest(request.request_id)
  world.override(r => r.url.endsWith('/reveal') ? Response.json({ ...secret(), owner_id: requester }) : undefined)
  expect(await a.sharing.revealHandle('INSTAGRAM')).toBe(false)
  world.setNow(Date.parse(request.expires_at)); const count = world.calls.length
  expect(await a.sharing.revealHandle('INSTAGRAM')).toBe(false); expect(world.calls).toHaveLength(count)
})
it('restricted discovery cannot retain a previous public intro or infer age', async () => {
  const a = world.actor(); a.owner.people.start(); await a.owner.people.load()
  expect(a.owner.people.read().page.data?.items).toHaveLength(1)
  world.override(r => r.url.endsWith('/intros') ? Response.json({ code: 'PERMISSION_DENIED', message: 'Restricted', details: { reason: 'PERMISSION_DENIED' } }, { status: 403 }) : undefined)
  await a.owner.people.load(); expect(a.owner.people.read().page.data).toBeNull()
})
it('confirmed synthetic HTTP block uses the retained safety policy cleanup', async () => {
  const a = world.actor()
  world.override(r => r.url.includes('/me/blocks/') ? Response.json({ target_user_id: recipient, blocked_by_me: true, revision: 1 }) : undefined)
  const safety = createSafetyController({ auth: a.auth, blocksAvailable: true, newKey: () => id(500) })
  try {
    await safety.block(recipient)
    expect(world.calls.filter(c => c.method === 'PUT' && c.path.endsWith(`/blocks/${recipient}`))).toHaveLength(1)
    expect(a.owner.isDisposed()).toBe(true)
  } finally { safety.dispose() }
})
it('capabilities without an explicit availability producer never dispatch social reads', async () => {
  const a = world.actor()
  const owner = createSocialOwner({ ...a.owner.options, availability: { read: () => null, subscribe: () => () => {} } })
  try {
    expect(owner.available('people')).toBe(false); expect(owner.available('requests')).toBe(false)
    owner.people.start(); await owner.people.load()
    expect(world.calls).toHaveLength(0)
  } finally { owner.dispose() }
})
it('foreign event discovery and foreign actor unread count fail closed in their actual consumers', async () => {
  const a = world.actor()
  world.override(r => {
    if (r.url.includes('/intros')) return Response.json({ items: [{ ...readJSON('contracts/fixtures/domain/public-intro.json'), event_id: id(999) }], next_cursor: null })
    if (r.url.endsWith('/unread-count')) return Response.json({ actor_id: recipient, event_id: request.event_id, unread_total: 1, as_of: '2026-11-07T09:00:00Z' })
    return undefined
  })
  a.owner.people.start(); await a.owner.people.load()
  expect(a.owner.people.read().page.phase).toBe('error'); expect(a.owner.people.read().page.data).toBeNull()
  a.owner.inbox.start(); await a.owner.inbox.loadUnreadCount()
  expect(a.owner.inbox.read().count.phase).toBe('error'); expect(a.owner.inbox.read().count.total).toBeNull()
})
