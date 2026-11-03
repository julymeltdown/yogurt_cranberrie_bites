import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { harness, beaconId, deferred, sampleReceipt, sampleBeacon, key } from './fixtures/harness'
import { beaconSchema } from '../model/contract'

type Harness = Awaited<ReturnType<typeof harness>>
const owners: Harness[] = []
async function setup() { const h = await harness(); owners.push(h); return h }
beforeEach(() => vi.stubGlobal('location', { origin: 'https://mcr.example.invalid' }))
afterEach(() => { owners.splice(0).forEach(h => h.dispose()); vi.unstubAllGlobals() })
describe('FE20 canonical transport and authority', () => {
  it('creates text-only scoped guidance once, with canonical privacy and idempotency metadata', async () => {
    const h = await setup(); h.fill()
    expect(await h.controller.create()).toBe(true)
    expect(h.controller.read().phase).toBe('current')
    const writes = h.requests.filter(r => r.method === 'POST'); expect(writes).toHaveLength(1)
    const r = writes[0]!
    expect(r.headers.get('Idempotency-Key')).toBe(key); expect(r.headers.get('X-MCR-Intent')).toBe('browser')
    expect(r.headers.get('Authorization')).toBe('Bearer synthetic-memory-token')
    expect(r.cache).toBe('no-store'); expect(r.credentials).toBe('same-origin'); expect(r.redirect).toBe('error')
    const json = await r.clone().json()
    expect(Object.keys(json).sort()).toEqual(['meeting', 'physical_presence_acknowledged'])
    expect(json.meeting.place_revision).toBe(3); expect(json.meeting.map_revision).toBe(1) // FE19 catalog is 7.
    expect(json.meeting.expires_at).toBe('2026-11-07T08:15:00.000Z')
    expect(h.controller.read().draft.identification).toBe('')
  })
  it.each(['no-authority', 'no-place-revision', 'no-time', 'write-denied', 'other-scope', 'expired-authority'] as const)('fails closed with %s', async mode => {
    const h = await setup(), a = h.authority!
    if (mode === 'no-authority') h.setAuthority(null)
    if (mode === 'no-place-revision') h.setAuthority({ ...a, placeRevisions: [] })
    if (mode === 'no-time') h.setTime(null)
    if (mode === 'write-denied') h.setAuthority({ ...a, canWrite: false })
    if (mode === 'other-scope') h.setAuthority({ ...a, scope: { ...a.scope, eventId: '00000000-0000-4000-8000-000000000999' } })
    if (mode === 'expired-authority') h.setAuthority({ ...a, expiresAt: '2026-11-07T07:59:59Z' })
    h.fill(); expect(await h.controller.create()).toBe(false)
    expect(h.requests.filter(r => r.method !== 'GET')).toHaveLength(0)
  })
  it('accepts only currently qualified audiences and physical places and enforces TTL and explicit acknowledgement', async () => {
    const h = await setup(); h.fill()
    for (const patch of [{ audienceId: 'stranger' }, { placeId: h.items[0]!.place_id }, { ttlSeconds: 901 }, { ttlSeconds: 0 }, { acknowledged: false }]) {
      h.fill(); h.controller.setDraft(patch); expect(await h.controller.create()).toBe(false)
    }
    expect(h.controller.read().places).toEqual([{ id: h.items[1]!.place_id, label: 'Synthetic reviewed landmark' }])
    expect(h.requests.filter(r => r.method !== 'GET')).toHaveLength(0)
    h.setAuthority({ ...h.authority!, maxTtlSeconds: 60, eventCutoff: '2026-11-07T08:00:30Z' })
    h.fill(); h.controller.setDraft({ ttlSeconds: 60 }); expect(await h.controller.create()).toBe(true)
    expect(h.beacon.meeting.expires_at).toBe('2026-11-07T08:00:30.000Z')
  })
})
describe('private meeting lifecycle', () => {
  it('expires and removes guidance, drafts, and photos at the exact boundary', async () => {
    const h = await setup(); await h.controller.select(beaconId); h.fill()
    h.setTime(Date.parse(h.beacon.meeting.expires_at))
    expect(h.controller.read()).toMatchObject({ phase: 'expired', beacon: null, place: null, photo: null, draft: { instructions: '', identification: '' } })
    const count = h.requests.length; await h.controller.refresh(); expect(h.requests).toHaveLength(count)
  })
  it('clears superseded details immediately and requires explicit confirmation of a newer revision', async () => {
    const h = await setup(); await h.controller.select(beaconId)
    h.controller.invalidate({ beaconId, revision: 2 }); expect(h.controller.read()).toMatchObject({ phase: 'moved', beacon: null })
    await h.controller.refresh(); expect(h.controller.read().phase).toBe('stale') // Old server revision must not revive.
    h.setBeacon({ ...h.beacon, revision: 2, meeting: { ...h.beacon.meeting, identification: 'SYNTHETIC new scarf' } })
    await h.controller.refresh(); expect(h.controller.read().phase).toBe('moved')
    expect(h.controller.confirmRevision()).toBe(true); expect(h.controller.read().phase).toBe('current')
  })
  it.each(['denied', 'missing'] as const)('discards cached private guidance on %s, without claiming expired/revoked facts', async mode => {
    const h = await setup(); await h.controller.select(beaconId); h.fill(); h.setMode(mode); await h.controller.refresh()
    expect(h.controller.read()).toMatchObject({ phase: 'unavailable', beacon: null, photo: null, place: null, draft: { instructions: '', identification: '' } })
  })
  it('revokes once with expected revision and never retains private details', async () => {
    const h = await setup(); await h.controller.select(beaconId)
    expect(await h.controller.revoke()).toBe(true)
    expect(h.controller.read()).toMatchObject({ phase: 'revoked', beacon: null, photo: null })
    const r = h.requests.find(r => r.method === 'DELETE')!; expect(new URL(r.url).searchParams.get('expected_revision')).toBe('1')
    expect(await h.controller.update()).toBe(false)
    await h.controller.select(beaconId); expect(h.controller.read()).toMatchObject({ phase: 'revoked', beacon: null })
  })
  it('cleans up synchronously on policy/identity changes and rejects a noncooperative late read', async () => {
    const h = await setup(); await h.controller.select(beaconId); h.fill()
    const wait = deferred<Response>(); h.setOverride(r => r.url.includes('/beacons/') ? wait.promise : null)
    const pending = h.controller.refresh()
    await vi.waitFor(() => expect(h.requests.filter(r => r.url.includes('/beacons/'))).toHaveLength(2))
    h.auth.privacy.changePolicy(h.auth.privacy.capture(), 'block')
    expect(h.controller.read()).toMatchObject({ beacon: null, draft: { instructions: '', identification: '' }, command: { phase: 'idle' } })
    wait.resolve(Response.json(h.beacon)); await pending
    expect(h.controller.read().beacon).toBeNull()
    h.auth.session.replace(h.auth.session.read().generation, null)
    expect(h.controller.read().audiences).toEqual([])
  })
  it('hides stale place/map guidance rather than converting catalog revision into place revision', async () => {
    const h = await setup(); h.setBeacon({ ...h.beacon, meeting: { ...h.beacon.meeting, place_revision: 7 } })
    await h.controller.select(beaconId); expect(h.controller.read()).toMatchObject({ phase: 'stale', beacon: null })
    h.setBeacon(sampleBeacon()); await h.controller.refresh(); expect(h.controller.read().phase).toBe('current')
    h.setAuthority({ ...h.authority!, placeRevisions: [{ placeId: h.items[1]!.place_id, revision: 4 }] })
    expect(h.controller.read()).toMatchObject({ phase: 'stale', beacon: null })
  })
  it('requires current scope, revision, audience, review, and photo permission before displaying a photo', async () => {
    const h = await setup(); await h.controller.select(beaconId)
    h.setPhoto({ scope: h.auth.privacy.capture(), beaconId, beaconRevision: 1, placeId: h.beacon.meeting.place_id,
      audienceKey: `party:${h.beacon.meeting.audience.kind === 'PARTY_MEMBERS' ? h.beacon.meeting.audience.party_id : ''}`,
      expiresAt: h.beacon.meeting.expires_at, permission: 'GRANTED', review: 'REVIEWED', src: 'https://media.example.invalid/synthetic.png', alt: 'Synthetic photo' })
    expect(h.controller.read().photo).toBeNull() // FE19 provenance explicitly denies photos.
  })
  it('shows only a fully qualified reviewed photo and clears it on changed bindings and expiry', async () => {
    const h = await setup()
    h.items[1]!.provenance.photo_permission = 'GRANTED'; await h.placesController.refresh(); await h.controller.select(beaconId)
    const photo = { scope: h.auth.privacy.capture(), beaconId, beaconRevision: 1, placeId: h.beacon.meeting.place_id,
      audienceKey: 'party:00000000-0000-4000-8000-000000000250', expiresAt: '2026-11-07T08:01:00Z', permission: 'GRANTED' as const,
      review: 'REVIEWED' as const, src: 'https://media.example.invalid/synthetic.png', alt: 'Synthetic reviewed photo' }
    h.setPhoto(photo); expect(h.controller.read().photo?.alt).toBe('Synthetic reviewed photo')
    for (const invalid of [{ beaconRevision: 2 }, { audienceKey: 'party:foreign' }, { scope: { ...photo.scope, policyGeneration: 99 } }, { src: 'javascript:alert(1)' }]) {
      h.setPhoto({ ...photo, ...invalid }); expect(h.controller.read().photo).toBeNull()
    }
    h.setPhoto(photo); h.setTime(Date.parse(photo.expiresAt)); expect(h.controller.read().photo).toBeNull()
    expect(h.controller.read().phase).toBe('current')
  })
  it('automatically removes expired details using the qualified server-time boundary', async () => {
    const h = await setup(); await h.controller.select(beaconId)
    vi.useFakeTimers()
    try {
      h.setTime(Date.parse(h.beacon.meeting.expires_at) - 1)
      h.setAuthority({ ...h.authority! }) // Reschedule boundary for current qualified time.
      h.advanceClock(Date.parse(h.beacon.meeting.expires_at))
      await vi.advanceTimersByTimeAsync(1)
      expect(h.controller.read()).toMatchObject({ phase: 'expired', beacon: null })
    } finally { vi.useRealTimers() }
  })
  it('rejects extra world/GPS fields and cross-event private responses', async () => {
    expect(beaconSchema.safeParse({ ...sampleBeacon(), world_position: { x: 1, y: 2 } }).success).toBe(false)
    const h = await setup(); h.setBeacon({ ...h.beacon, event_id: '00000000-0000-4000-8000-000000000999' })
    await h.controller.select(beaconId); expect(h.controller.read().beacon).toBeNull()
  })
})
describe('uncertain command recovery without replay', () => {
  it('accepts equivalent server-normalized timestamps without inventing an uncertain outcome', async () => {
    const h = await setup(); h.fill()
    h.setOverride(r => {
      if (r.method !== 'POST') return null
      return (async () => {
        const body = await r.clone().json()
        return Response.json({ ...h.beacon, meeting: { ...body.meeting, expires_at: body.meeting.expires_at.replace('.000Z', 'Z') } })
      })()
    })
    expect(await h.controller.create()).toBe(true)
    expect(h.controller.read().command.phase).toBe('succeeded')
  })
  it.each(['lost', 'server', 'pending'] as const)('retains a %s outcome, checks the bound receipt, then re-reads authorized state', async mode => {
    const h = await setup(); h.fill(); h.setMode(mode); expect(await h.controller.create()).toBe(false)
    expect(h.controller.read().command.phase).toBe(mode === 'pending' ? 'pending' : 'uncertain')
    expect(await h.controller.create()).toBe(false); expect(h.requests.filter(r => r.method === 'POST')).toHaveLength(1)
    await h.controller.checkCommand(); expect(h.controller.read().command.phase).toBe('pending')
    h.setStatus(sampleReceipt('createConcertBeacon', 'SUCCEEDED')); h.setMode('ok'); await h.controller.checkCommand()
    expect(h.controller.read().command.phase).toBe('succeeded'); expect(h.controller.read().phase).toBe('moved')
    expect(h.requests.filter(r => r.method === 'POST')).toHaveLength(1)
    const status = h.requests.find(r => new URL(r.url).pathname.endsWith('/commands'))!
    expect(new URL(status.url).searchParams.get('idempotency_key')).toBe(key)
    expect(new URL(status.url).searchParams.get('operation_id')).toBe('createConcertBeacon')
  })
  it('keeps missing, malformed, foreign, and retired receipts locked instead of generating a new mutation', async () => {
    const h = await setup(); h.fill(); h.setMode('lost'); await h.controller.create()
    h.setStatus({ ...sampleReceipt(), actor_id: '00000000-0000-4000-8000-000000000999' }); await h.controller.checkCommand()
    expect(h.controller.read().command.phase).toBe('uncertain')
    h.setStatus(sampleReceipt('createConcertBeacon', 'RETIRED')); await h.controller.checkCommand()
    expect(h.controller.read().command.phase).toBe('retired'); expect(await h.controller.create()).toBe(false)
    expect(h.requests.filter(r => r.method === 'POST')).toHaveLength(1)
  })
  it('drops stale revisions after a conflict and does not report success', async () => {
    const h = await setup(); await h.controller.select(beaconId); h.controller.editCurrent(); h.controller.setDraft({ acknowledged: true }); h.setMode('conflict')
    expect(await h.controller.update()).toBe(false)
    expect(h.controller.read()).toMatchObject({ phase: 'stale', beacon: null, command: { phase: 'failed' }, notice: 'changed' })
  })
  it('blocks duplicate submissions while a command is in flight and disposes without a late result', async () => {
    const h = await setup(), wait = deferred<Response>(); h.fill(); h.setOverride(r => r.method === 'POST' ? wait.promise : null)
    const pending = h.controller.create()
    await vi.waitFor(() => expect(h.requests.filter(r => r.method === 'POST')).toHaveLength(1))
    expect(await h.controller.create()).toBe(false); h.controller.dispose(); wait.resolve(Response.json(h.beacon)); await pending
    expect(h.controller.read().beacon).toBeNull()
  })
})
