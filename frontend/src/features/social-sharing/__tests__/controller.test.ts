import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { deferred, epoch, eventId, handleStates, id, pending, recipientId, request, requesterId, setup } from './fixtures/data'
import { createSocialSharingController } from '../model/controller'
import { validHandle } from '../model/contract'
const disposals: (() => void)[] = []
beforeEach(() => vi.stubGlobal('location', { origin: 'https://mcr.example' }))
afterEach(() => { disposals.splice(0).forEach(fn => fn()); vi.useRealTimers(); vi.unstubAllGlobals() })
function fixture(handler: (r: Request) => Response | Promise<Response>, userId = requesterId, qualifiedRefresh = false) {
  const fetch = vi.fn<typeof globalThis.fetch>(async input => handler(input as Request)), f = setup(fetch, userId, qualifiedRefresh); disposals.push(f.dispose); return { ...f, fetch }
}
const privateResult = () => ({ request_id: request.request_id, platform: 'INSTAGRAM', owner_id: recipientId, handle_version: 3, handle: 'synthetic_private_name', grant_revision: 1, expires_at: request.expires_at })
const receipt = (key: string, operation = 'createConcertSocialRequest', state = 'PENDING') => ({ receipt_id: id(100), event_id: eventId, actor_id: requesterId, operation_id: operation, idempotency_key: key, state, resource_id: state === 'SUCCEEDED' ? request.request_id : null, resource_revision: state === 'SUCCEEDED' ? 1 : null, failure_reason: null, created_at: '2026-11-07T09:00:00Z', expires_at: '2026-11-08T09:00:00Z', result_status: state === 'SUCCEEDED' ? 200 : null })
const sharingOffer = () => ({ intro_id: request.intro_id, intro_revision: 1, recipient_id: recipientId, available_handles: [{ platform: 'INSTAGRAM', version: 3 }] })
async function prepared(f: ReturnType<typeof fixture>) { await f.controller.loadHandles(); await f.controller.loadOffer(request.intro_id); f.controller.setRequesterConsent('INSTAGRAM', true) }
it('configures/replaces normalized handle with exact version and removes with monotone tombstone in read-only', async () => {
  let version = 2, configured = true; const writes: { method: string; body: unknown; query: string }[] = []
  const f = fixture(async r => {
    if (r.method === 'GET') return Response.json({ items: [{ user_id: requesterId, platform: 'INSTAGRAM', version, configured }, { user_id: requesterId, platform: 'X', version: 0, configured: false }] })
    writes.push({ method: r.method, body: r.method === 'PUT' ? await r.json() : null, query: new URL(r.url).search }); version++; configured = r.method !== 'DELETE'
    return Response.json({ user_id: requesterId, platform: 'INSTAGRAM', version, configured })
  })
  await f.controller.loadHandles(); f.controller.setHandleDraft('INSTAGRAM', ' Replacement.Name '); await f.controller.saveHandle('INSTAGRAM')
  await vi.waitFor(() => expect(f.controller.read().handles.phase).toBe('ready'))
  expect(writes[0]!.body).toEqual({ expected_revision: 2, handle: 'replacement.name' }); expect(f.controller.read().handleDrafts.INSTAGRAM).toBe('')
  f.setAccess({ available: true, readOnly: true }); await f.controller.removeHandle('INSTAGRAM')
  expect(writes[1]).toEqual({ method: 'DELETE', body: null, query: '?expected_revision=3' })
  await vi.waitFor(() => expect(f.controller.read().handles.data?.items[0]?.version).toBe(4)); expect(f.controller.read().handles.data?.items[0]?.configured).toBe(false)
})
it('rejects invalid platform handles locally without dispatch', async () => {
  const f = fixture(() => Response.json(handleStates())); await f.controller.loadHandles()
  for (const text of ['@bad', '.bad', 'bad.', 'bad..name', 'x'.repeat(31)]) { f.controller.setHandleDraft('INSTAGRAM', text); expect(await f.controller.saveHandle('INSTAGRAM')).toBe(false) }
  expect(validHandle('X', 'x'.repeat(16))).toBe(false); expect(f.fetch).toHaveBeenCalledTimes(1)
})
it('requires explicit per-platform requester consent and binds both handle versions without plaintext', async () => {
  let body: unknown
  const f = fixture(async r => {
    if (r.method === 'POST') { body = await r.json(); return Response.json(pending()) }
    if (r.url.endsWith('/social-handles')) return Response.json(handleStates())
    if (r.url.endsWith('/sharing-offer')) return Response.json(sharingOffer())
    return Response.json(pending())
  })
  await f.controller.loadHandles(); await f.controller.loadOffer(request.intro_id)
  expect(await f.controller.createRequest()).toBe(false)
  f.controller.setRequesterConsent('INSTAGRAM', true); await f.controller.createRequest()
  expect(body).toEqual({ recipient_id: recipientId, intro_id: request.intro_id, intro_revision: 1, consents: [{ platform: 'INSTAGRAM', requester_handle_version: 2, recipient_handle_version: 3, requester_consents: true }] })
  expect(JSON.stringify(body)).not.toContain('handle"'); expect(f.controller.read().secret.data).toBeNull()
})
it('recipient approval requires reciprocal consent and selected subset; rejection creates no approval', async () => {
  let decision: unknown, next = pending()
  const f = fixture(async r => {
    if (r.method === 'POST') { decision = await r.json(); next = { ...next, state: 'REJECTED', revision: 2 }; return Response.json(next) }
    return Response.json(next)
  }, recipientId)
  await f.controller.loadRequest(request.request_id)
  f.controller.setDecisionPlatform('INSTAGRAM', true); expect(await f.controller.decide(true)).toBe(false)
  f.controller.setRecipientConsent(true); f.controller.setDecisionPlatform('X', true); expect(await f.controller.decide(true)).toBe(false)
  f.controller.setDecisionPlatform('X', false); await f.controller.decide(false)
  expect(decision).toEqual({ expected_revision: 1, decision: 'REJECTED', approved_platforms: [], recipient_consents: false })
})
it('reveals only after explicit approved-platform action through one canonical no-store sensitive request', async () => {
  let reveals = 0
  const f = fixture(r => {
    expect(r.cache).toBe('no-store'); expect(r.credentials).toBe('same-origin')
    if (r.url.includes('/reveal')) { reveals++; return Response.json(privateResult()) }
    return Response.json(request)
  })
  await f.controller.loadRequest(request.request_id); expect(reveals).toBe(0); expect(f.auth.queryClient.getQueryCache().getAll()).toHaveLength(0)
  expect(await f.controller.revealHandle('X')).toBe(false); await f.controller.revealHandle('INSTAGRAM')
  expect(reveals).toBe(1); expect(f.controller.read().secret.data?.handle).toBe('synthetic_private_name')
  f.setNow(Date.parse(request.expires_at)); expect(f.controller.read().secret.data).toBeNull(); expect(await f.controller.revealHandle('INSTAGRAM')).toBe(false)
})
it.each(['identity', 'event', 'policy', 'revoke', 'expiry'] as const)('late reveal cannot repopulate after %s', async kind => {
  const late = deferred<Response>(); let post = request
  const f = fixture(r => r.url.includes('/reveal') ? late.promise : r.method === 'POST' ? Response.json(post = { ...request, state: 'REVOKED', revision: 3 }) : Response.json(post))
  await f.controller.loadRequest(request.request_id)
  const flight = f.controller.revealHandle('INSTAGRAM'); await vi.waitFor(() => expect(f.fetch).toHaveBeenCalledTimes(2))
  if (kind === 'identity') f.auth.session.replace(f.auth.session.read().generation, null)
  if (kind === 'event') f.auth.session.replace(f.auth.session.read().generation, { userId: requesterId, eventId: id(999), accessToken: 'other-event-token' })
  if (kind === 'policy') f.auth.privacy.changePolicy(f.auth.privacy.capture(), 'consent-revoked')
  if (kind === 'revoke') await f.controller.revokeRequest()
  if (kind === 'expiry') f.setNow(Date.parse(request.expires_at))
  late.resolve(Response.json(privateResult())); await flight; expect(f.controller.read().secret.data).toBeNull()
})
it.each([
  { owner_id: requesterId }, { handle_version: 1 }, { platform: 'X' }, { request_id: id(999) }, { expires_at: '2026-11-07T11:00:00Z' }, { private_age: 25 },
])('rejects foreign/stale/private reveal response: %j', async patch => {
  const f = fixture(r => Response.json(r.url.includes('/reveal') ? { ...privateResult(), ...patch } : request))
  await f.controller.loadRequest(request.request_id); expect(await f.controller.revealHandle('INSTAGRAM')).toBe(false); expect(f.controller.read().secret.data).toBeNull()
})
it('401 secret read with qualifiedRefresh does not retry or refresh', async () => {
  let reveals = 0
  const f = fixture(r => { if (r.url.includes('/reveal')) { reveals++; return Response.json({}, { status: 401 }) } return Response.json(request) }, requesterId, true)
  await f.controller.loadRequest(request.request_id); await f.controller.revealHandle('INSTAGRAM')
  expect(reveals).toBe(1); expect(f.fetch).toHaveBeenCalledTimes(2); expect(f.controller.read().secret.data).toBeNull()
})
it('retains exact key/body on uncertain create and checks actor/event/operation/resource-bound receipt without mutation replay', async () => {
  let writes = 0, key = '', wrong = true
  const f = fixture(r => {
    if (r.method === 'POST') { writes++; key = r.headers.get('Idempotency-Key')!; throw new Error('lost after commit') }
    if (r.url.includes('/commands')) return Response.json({ ...receipt(key, 'createConcertSocialRequest', 'SUCCEEDED'), ...(wrong ? { actor_id: id(99) } : {}) })
    if (r.url.includes('/social-handles')) return Response.json(handleStates())
    if (r.url.includes('/sharing-offer')) return Response.json(sharingOffer())
    return Response.json(pending())
  })
  await prepared(f); await f.controller.createRequest(); expect(f.controller.read().mutation).toMatchObject({ phase: 'uncertain', key })
  f.controller.setRequesterConsent('INSTAGRAM', false); expect(f.controller.read().requesterConsent).toEqual(['INSTAGRAM'])
  await f.controller.createRequest(); await f.controller.checkStatus(); expect(writes).toBe(1); expect(f.controller.read().mutation.phase).toBe('uncertain')
  wrong = false; await f.controller.checkStatus(); expect(writes).toBe(1); expect(f.controller.read().mutation.phase).toBe('succeeded')
})
it('expired/denied request and absent qualified sensitive port fail closed', async () => {
  const f = fixture(() => Response.json(request))
  const controller = createSocialSharingController({ auth: f.auth, access: f.access, now: () => epoch }); disposals.push(controller.dispose); controller.start()
  await controller.loadRequest(request.request_id); expect(await controller.revealHandle('INSTAGRAM')).toBe(false)
  expect(f.fetch).toHaveBeenCalledTimes(1)
})
it('privacy change scrubs private drafts synchronously; unblock/revalidation never restores old revealed name', async () => {
  const f = fixture(r => Response.json(r.url.endsWith('/social-handles') ? handleStates() : r.url.includes('/reveal') ? privateResult() : request))
  await f.controller.loadHandles(); f.controller.setHandleDraft('INSTAGRAM', 'synthetic_draft'); await f.controller.loadRequest(request.request_id); await f.controller.revealHandle('INSTAGRAM')
  f.auth.privacy.changePolicy(f.auth.privacy.capture(), 'block')
  expect(f.controller.read().handleDrafts.INSTAGRAM).toBe(''); expect(f.controller.read().secret.data).toBeNull()
  f.auth.privacy.changePolicy(f.auth.privacy.capture(), 'revalidated'); expect(f.controller.read().secret.data).toBeNull()
})
it('same-identity read-only transition keeps unresolved key and forbids new sharing writes', async () => {
  const f = fixture(r => r.method === 'POST' ? Promise.reject(new Error('lost')) : Response.json(r.url.includes('/social-handles') ? handleStates() : sharingOffer()))
  await prepared(f); await f.controller.createRequest(); const key = f.controller.read().mutation.key
  f.setAccess({ available: true, readOnly: true }); expect(f.controller.read().mutation).toMatchObject({ phase: 'uncertain', key }); expect(await f.controller.createRequest()).toBe(false)
})
it.each([
  { actor_id: id(90) }, { event_id: id(90) }, { operation_id: 'removeConcertHandle' }, { idempotency_key: id(90) },
  { resource_id: id(90) }, { resource_revision: 2 }, { result_status: 202 }, { failure_reason: 'PERMISSION_DENIED' },
  { handle: 'private-field' }, { expires_at: '2026-11-06T09:00:00Z' },
])('rejects mismatched successful handle receipt without releasing command or replay: %j', async patch => {
  let key = '', writes = 0
  const f = fixture(r => {
    if (r.method === 'PUT') { writes++; key = r.headers.get('Idempotency-Key')!; return Response.json(receipt(key, 'setConcertHandle'), { status: 202 }) }
    if (r.url.includes('/commands')) return Response.json({ ...receipt(key, 'setConcertHandle', 'SUCCEEDED'), resource_id: requesterId, resource_revision: 3, ...patch })
    return Response.json(handleStates())
  })
  await f.controller.loadHandles(); f.controller.setHandleDraft('INSTAGRAM', 'replacement'); await f.controller.saveHandle('INSTAGRAM')
  expect(f.controller.read().mutation.phase).toBe('pending'); await f.controller.checkStatus()
  expect(f.controller.read().mutation).toMatchObject({ phase: 'uncertain', key }); expect(await f.controller.saveHandle('INSTAGRAM')).toBe(false); expect(writes).toBe(1)
})
it('retired receipts retain uncertainty; failed receipt preserves the original command and blocks a replacement key', async () => {
  let key = '', retired = true, writes = 0
  const f = fixture(r => {
    if (r.method === 'POST') { writes++; key = r.headers.get('Idempotency-Key')!; return Response.json(receipt(key), { status: 202 }) }
    if (r.url.includes('/commands')) return Response.json({ ...receipt(key), state: retired ? 'RETIRED' : 'FAILED', failure_reason: retired ? null : 'VERSION_CONFLICT' })
    return Response.json(r.url.endsWith('/social-handles') ? handleStates() : sharingOffer())
  })
  await prepared(f); await f.controller.createRequest(); await f.controller.checkStatus(); expect(f.controller.read().mutation.phase).toBe('uncertain')
  retired = false; await f.controller.checkStatus(); expect(f.controller.read().mutation).toMatchObject({ phase: 'blocked', key }); f.controller.setRequesterConsent('INSTAGRAM', true); await f.controller.createRequest(); expect(writes).toBe(1)
})
it.each([
  { consents: [{ ...request.consents[0]!, recipient_handle_version: 99 }] }, { recipient_id: id(90) }, { state: 'APPROVED', approved_platforms: ['INSTAGRAM'] }, { handle: 'private-field' },
])('cannot materialize a wrong successful create DTO: %j', async patch => {
  const f = fixture(r => Response.json(r.method === 'POST' ? { ...pending(), ...patch } : r.url.endsWith('/social-handles') ? handleStates() : sharingOffer()))
  await prepared(f); await f.controller.createRequest(); expect(f.controller.read().mutation.phase).toBe('uncertain'); expect(f.controller.read().detail.data).toBeNull(); expect(f.controller.read().secret.data).toBeNull()
})
it.each([
  { handle: 'private-field' }, { recipient_id: id(90) }, { event_id: id(90) }, { consents: [...request.consents, ...request.consents] },
  { created_at: '2026-11-07T18:00:00+09:00' },
])('rejects metadata DTO outside strict contract and current incoming scope: %j', async patch => {
  const f = fixture(() => Response.json({ items: [{ ...request, ...patch }], next_cursor: null }), recipientId)
  await f.controller.loadRequests(); expect(f.controller.read().requests).toMatchObject({ phase: 'error', data: null }); expect(f.controller.read().secret.data).toBeNull()
})
it('rejects cyclic cursors and duplicate pagination instead of silently merging', async () => {
  const f = fixture(r => Response.json({ items: new URL(r.url).searchParams.has('cursor') ? [] : [request], next_cursor: 'repeat' }), recipientId)
  await f.controller.loadRequests(); expect(f.controller.read().requests.phase).toBe('ready'); await f.controller.loadRequests(true); expect(f.controller.read().requests).toMatchObject({ phase: 'error', data: null })
})
it.each(['REJECTED', 'CANCELLED', 'REVOKED', 'EXPIRED'] as const)('terminal %s request cannot reveal or mutate a grant', async state => {
  const f = fixture(() => Response.json({ ...request, state, approved_platforms: ['REJECTED', 'CANCELLED'].includes(state) ? [] : ['INSTAGRAM'] }))
  await f.controller.loadRequest(request.request_id); expect(await f.controller.revealHandle('INSTAGRAM')).toBe(false); expect(await f.controller.revokeRequest()).toBe(false); expect(f.fetch).toHaveBeenCalledTimes(1)
})
it('permission loss clears visible plaintext and drafts while preserving the original unresolved command key', async () => {
  const f = fixture(r => r.method === 'POST' ? Promise.reject(new Error('lost')) : Response.json(r.url.includes('/social-handles') ? handleStates() : sharingOffer()))
  await prepared(f); f.controller.setHandleDraft('INSTAGRAM', 'synthetic_draft'); await f.controller.createRequest(); const key = f.controller.read().mutation.key
  f.setAccess({ available: false, readOnly: true }); expect(f.controller.read().handleDrafts.INSTAGRAM).toBe(''); expect(f.controller.read().secret.data).toBeNull(); expect(f.controller.read().mutation).toMatchObject({ key, phase: 'uncertain' })
})
it('expiry timer clears plaintext without a new read, while background/foreground expiry is rechecked', async () => {
  vi.useFakeTimers(); let time = epoch
  const f = fixture(r => Response.json(r.url.includes('/reveal') ? { ...privateResult(), expires_at: new Date(epoch + 1000).toISOString() } : request))
  const controller = createSocialSharingController({ auth: f.auth, access: f.access, sensitiveRead: f.auth.requestSensitive, now: () => time }); disposals.push(controller.dispose); controller.start()
  await controller.loadRequest(request.request_id); await controller.revealHandle('INSTAGRAM'); expect(controller.read().secret.data).not.toBeNull()
  time += 1000; await vi.advanceTimersByTimeAsync(1000); expect(controller.read().secret.data).toBeNull()
})
// Regressions follow the independent FE31 QA probes against 310dd65.
it.each(['sending', 'hide'] as const)('reentrant identity change during %s notification cannot send the former private draft', async point => {
  let writes = 0, switched = false
  const f = fixture(r => { if (r.method === 'PUT') writes++; return Response.json(handleStates()) })
  await f.controller.loadHandles(); f.controller.setHandleDraft('INSTAGRAM', 'account_a_private')
  f.controller.subscribe(() => { if (!switched && f.controller.read().mutation.phase === (point === 'sending' ? 'sending' : 'idle')) { switched = true; f.auth.session.replace(f.auth.session.read().generation, { userId: id(99), eventId, accessToken: 'new-user-token' }) } })
  await f.controller.saveHandle('INSTAGRAM'); expect(switched).toBe(true); expect(writes).toBe(0)
})
it.each([
  { recipient_id: id(99) }, { intro_id: id(98) }, { intro_revision: 99 }, { revision: 1 }, { consents: [{ ...request.consents[0]!, recipient_handle_version: 99 }] },
])('successful receipt cannot materialize mismatched or lower-revision resource: %j', async patch => {
  let key = '', writes = 0
  const f = fixture(r => {
    if (r.method === 'POST') { writes++; key = r.headers.get('Idempotency-Key')!; return Response.json(receipt(key), { status: 202 }) }
    if (r.url.includes('/commands')) return Response.json({ ...receipt(key, 'createConcertSocialRequest', 'SUCCEEDED'), resource_revision: 2 })
    return Response.json(r.url.endsWith('/social-handles') ? handleStates() : r.url.endsWith('/sharing-offer') ? sharingOffer() : { ...pending(), revision: 2, ...patch })
  })
  await prepared(f); await f.controller.createRequest(); await f.controller.checkStatus()
  expect(f.controller.read().detail).toMatchObject({ phase: 'error', data: null }); expect(f.controller.read().mutation).toMatchObject({ phase: 'uncertain', key }); await f.controller.createRequest(); expect(writes).toBe(1)
})
it('authorized receipt reread may observe a legal later approval with the original immutable bindings', async () => {
  let key = ''
  const f = fixture(r => {
    if (r.method === 'POST') { key = r.headers.get('Idempotency-Key')!; return Response.json(receipt(key), { status: 202 }) }
    if (r.url.includes('/commands')) return Response.json(receipt(key, 'createConcertSocialRequest', 'SUCCEEDED'))
    return Response.json(r.url.endsWith('/social-handles') ? handleStates() : r.url.endsWith('/sharing-offer') ? sharingOffer() : request)
  })
  await prepared(f); await f.controller.createRequest(); await f.controller.checkStatus(); expect(f.controller.read().mutation.phase).toBe('succeeded'); expect(f.controller.read().detail.data?.state).toBe('APPROVED')
})
it.each(['expired', 'wrong-reason'] as const)('receipt %s cannot unlock an uncertain create', async kind => {
  let key = '', writes = 0
  const f = fixture(r => {
    if (r.method === 'POST') { writes++; key = r.headers.get('Idempotency-Key')!; return Response.json(receipt(key), { status: 202 }) }
    if (r.url.includes('/commands')) return Response.json(kind === 'expired' ? { ...receipt(key, 'createConcertSocialRequest', 'SUCCEEDED'), created_at: '2026-11-07T08:00:00Z', expires_at: '2026-11-07T08:30:00Z' } : { ...receipt(key, 'createConcertSocialRequest', 'FAILED'), failure_reason: 'MURAL_COOLDOWN' })
    return Response.json(r.url.endsWith('/social-handles') ? handleStates() : sharingOffer())
  })
  await prepared(f); await f.controller.createRequest(); await f.controller.checkStatus(); expect(f.controller.read().mutation).toMatchObject({ phase: 'uncertain', key }); await f.controller.createRequest(); expect(writes).toBe(1)
})
it.each([2, 99])('post-revoke stale APPROVED reread at revision%s never becomes verified current state', async revision => {
  let submitted = false
  const f = fixture(r => { if (r.method === 'POST') { submitted = true; return Response.json({ ...request, state: 'REVOKED', revision: 3 }) } return Response.json({ ...request, revision: submitted ? revision : request.revision }) })
  await f.controller.loadRequest(request.request_id); await f.controller.revokeRequest(); expect(f.controller.read().detail.data).toBeNull(); expect(f.controller.read().mutation.phase).toBe('uncertain'); expect(await f.controller.revealHandle('INSTAGRAM')).toBe(false)
})
it('confirmed revocation guard rejects subsequent stale detail and list metadata', async () => {
  let revoked = false, stale = false
  const f = fixture(r => {
    if (r.method === 'POST') revoked = true
    const data = revoked && !stale ? { ...request, state: 'REVOKED', revision: 3 } : request
    return Response.json(r.url.includes('/me/social-requests') ? { items: [data], next_cursor: null } : data)
  })
  await f.controller.loadRequest(request.request_id); await f.controller.revokeRequest(); expect(f.controller.read().mutation.phase).toBe('succeeded')
  stale = true; await f.controller.loadRequest(request.request_id); expect(f.controller.read().detail.data).toBeNull(); f.controller.setDirection('OUTGOING'); await vi.waitFor(() => expect(f.controller.read().requests.phase).toBe('error'))
})
it('private reveal expiry is rechecked after the qualified read settles and before publication', async () => {
  let time = epoch
  const f = fixture(r => Response.json(r.url.includes('/reveal') ? { ...privateResult(), expires_at: new Date(epoch + 1000).toISOString() } : request))
  const controller = createSocialSharingController({ auth: f.auth, access: f.access, now: () => time, sensitiveRead: async input => { const result = await f.auth.requestSensitive(input); time += 1000; return result } }); disposals.push(controller.dispose); controller.start()
  await controller.loadRequest(request.request_id); await controller.revealHandle('INSTAGRAM'); expect(controller.read().secret).toMatchObject({ phase: 'idle', data: null })
})
it('in-flight access loss preserves the original uncertain command and recovers by status without replay', async () => {
  const late = deferred<Response>(); let key = '', writes = 0
  const f = fixture(r => {
    if (r.method === 'POST') { writes++; key = r.headers.get('Idempotency-Key')!; return late.promise }
    if (r.url.includes('/commands')) return Response.json(receipt(key, 'createConcertSocialRequest', 'SUCCEEDED'))
    return Response.json(r.url.endsWith('/social-handles') ? handleStates() : r.url.endsWith('/sharing-offer') ? sharingOffer() : pending())
  })
  await prepared(f); const send = f.controller.createRequest(); await vi.waitFor(() => expect(key).not.toBe(''))
  f.setAccess({ available: false, readOnly: true }); await send; late.resolve(Response.json(pending())); f.setAccess({ available: true, readOnly: false })
  expect(f.controller.read().mutation).toMatchObject({ phase: 'uncertain', key }); await f.controller.checkStatus(); expect(f.controller.read().mutation.phase).toBe('succeeded'); expect(writes).toBe(1)
})
