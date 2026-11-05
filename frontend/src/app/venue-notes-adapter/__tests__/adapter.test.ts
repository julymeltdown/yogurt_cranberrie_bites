import { afterEach, describe, expect, it, vi } from 'vitest'
import { QueryClient } from '@tanstack/react-query'
import { createMemorySession } from '../../../shared/api/http'
import { createQueryPrivacy, type PrivacyScope } from '../../../shared/query/privacy'
import { createNotesClient, type NoteCommand, type NotesTransport, type NotePage, type PlaceNote } from '../../../shared/api/paradise-city-v1/notes'
import { landmarkId } from '../../../features/paradise-city-map/spatial/contract'
import type { CommandContext, ReadPort, VenueView } from '../../../features/paradise-city-map/ports'
import { createVenueNotesAdapter, type NoteCreationRights, type ReviewedNoteAssociations, type NoteStatusProducer } from '..'

const eventId = '00000000-0000-4000-8000-000000000201', userId = '00000000-0000-4000-8000-000000000101'
const noteId = '00000000-0000-4000-8000-000000000301', replyId = '00000000-0000-4000-8000-000000000302'
const key = '00000000-0000-4000-8000-000000000401', otherKey = '00000000-0000-4000-8000-000000000402'
const physicalId = 'reviewed.main-gate~v1', artId = landmarkId('main-gate')
const time = '2026-11-05T01:00:00Z', expiry = '2026-11-05T02:00:00Z'
const selection = { landmark_id: physicalId, catalog_revision: 2, landmark_revision: 7 }
const venue: VenueView = { identity: { venueId: 'paradise-city', siteplanId: 'main', revision: 1, variantId: 'day' },
  title: 'Venue', image: { src: '/fixture.jpg', width: 100, height: 100, description: 'Fixture' },
  landmarks: [{ id: artId, label: 'Gate', point: { x: 1, y: 1 }, meetingContext: null }], classification: 'ILLUSTRATED_UNCALIBRATED' }
function note(patch: Partial<PlaceNote> = {}): PlaceNote {
  return { id: noteId, landmark_id: physicalId, landmark_revision: 7,
    author: { user_id: userId, display_name: 'Fixture friend', bio: null, avatar: { revision: 1, config: {
      contract_version: '1.1.0', kind: 'avatar_config', manifest_id: 'mcr_selected_static_20261103', manifest_version: 1,
      representation: 'whole_character', preset_id: 'photo01_front_01' } } },
    text: 'Beside the gate', created_at: time, expires_at: expiry, revision: 4, audience: 'FRIENDS', expiry: 'ONE_HOUR',
    parent_id: null, reply_count: 0, can_reply: true, can_withdraw: true, meet_here: null, ...patch }
}
function page(patch: Partial<NotePage> = {}): NotePage {
  return { items: [note()], visible_count: 1, parent: null, next_before: null, server_now: time,
    access_until: '2026-11-05T01:00:05Z', ...patch }
}
function receipt(patch: Partial<NoteCommand> = {}): NoteCommand {
  return { receipt_id: key, operation_id: 'createVenuePlaceNote', state: 'PENDING', resource: null,
    failure_reason: null, created_at: time, expires_at: expiry, ...patch }
}
function port<T>(initial: T): ReadPort<T> & { set(value: T): void } {
  let value = initial
  const listeners = new Set<() => void>()
  return { read: () => value, subscribe(listener) { listeners.add(listener); return () => { listeners.delete(listener) } },
    set(next) { value = next; for (const listener of Array.from(listeners)) listener() } }
}
const disposals: (() => void)[] = []
afterEach(() => { for (const dispose of disposals.splice(0)) dispose() })
function fixture(settings: { status?: NoteStatusProducer | null; rights?: boolean; client?: boolean } = {}) {
  const session = createMemorySession()
  session.replace(0, { userId, eventId, accessToken: 'fixture.token' })
  const query = new QueryClient(), privacy = createQueryPrivacy({ client: query, session }), scope = privacy.capture()
  const lifetime = new AbortController(), signal = new AbortController()
  const clock = port<{ scope: PrivacyScope; nowMs: number } | null>({ scope, nowMs: Date.parse(time) + 1 })
  const associations = port<ReviewedNoteAssociations | null>({ scope, venue: venue.identity, accessUntilMs: Date.parse(expiry),
    bindings: [{ landmarkId: artId, selection }] })
  const rights = port<NoteCreationRights | null>({ scope, venue: venue.identity, accessUntilMs: Date.parse(expiry),
    rights: { postNote: [artId], noteAudiences: ['FRIENDS', 'EVERYONE'], noteExpiries: ['ONE_HOUR', 'END_OF_DAY'] } })
  let response: unknown = page(), statusCode = 200
  let deferred: Promise<{ status: number; data: unknown }> | null = null
  const send = vi.fn<NotesTransport['send']>(async () => deferred ?? { status: statusCode, data: response })
  const raw = createNotesClient({ privacy, transport: { send } })
  const requestContexts: Pick<CommandContext, 'scope' | 'signal'>[] = []
  const request: typeof raw.request = (id, input, context) => { requestContexts.push(context); return raw.request(id, input, context) }
  const adapter = createVenueNotesAdapter({ authority: { scope, signal: lifetime.signal, isCurrent: () => privacy.isCurrent(scope),
    subscribe: callback => privacy.subscribe(callback) }, venue, clock, client: settings.client === false ? null : { request },
    associations, creationRights: settings.rights === false ? null : rights,
    status: settings.status ?? null, classification: 'CONTROLLED_FIXTURE' })
  disposals.push(() => { adapter.dispose(); privacy.dispose(); query.clear() })
  const context = (patch: Partial<CommandContext> = {}): CommandContext => ({ key, scope, venue: venue.identity, signal: signal.signal, ...patch })
  return { adapter, scope, privacy, clock, associations, rights, signal, lifetime, send, requestContexts, context,
    respond(value: unknown, status = 200) { response = value; statusCode = status },
    defer(value: NonNullable<typeof deferred>) { deferred = value },
    load: () => adapter.load(artId, signal.signal),
    post: (patch: Partial<Parameters<typeof adapter.postNote>[0]> = {}, ctx = context()) => adapter.postNote({
      landmarkId: artId, body: 'Beside the gate', audience: 'FRIENDS', expiry: 'ONE_HOUR', ...patch }, ctx) }
}

describe('actual raw N client to primary Notes partial', () => {
  it('reads only on explicit load and returns a detached immutable primary projection', async () => {
    const f = fixture(); expect(f.adapter.read()).toBeNull(); expect(f.send).not.toHaveBeenCalled()
    expect(await f.load()).toBe(true)
    const view = f.adapter.read()!
    expect(view.notes[0]).toMatchObject({ id: noteId, landmarkId: artId, body: 'Beside the gate', audience: 'FRIENDS', expiresAtMs: Date.parse(expiry), canMeet: false })
    expect(Object.isFrozen(view.notes[0]!.author.avatar)).toBe(true)
    expect(Object.isFrozen(view.rights.postNote)).toBe(true)
    expect(f.requestContexts[0]?.signal).toBe(f.signal.signal)
  })
  it.each(['FRIENDS', 'EVERYONE'] as const)('preserves %s and maps END_OF_DAY to server venue expiry only', async audience => {
    const f = fixture(); await f.load(); f.respond(receipt(), 201)
    expect(await f.post({ audience, expiry: 'END_OF_DAY' })).toEqual({ state: 'unknown' })
    expect(f.send.mock.calls[1]?.[0]).toMatchObject({ idempotencyKey: key, body: { text: 'Beside the gate', audience,
      expiry: 'VENUE_END_DAY', landmark_selection: selection } })
    expect(f.requestContexts[1]?.signal).toBe(f.signal.signal)
  })
  it('does not infer creation rights from visible reply rights', async () => {
    const f = fixture({ rights: false }); await f.load()
    expect(f.adapter.read()!.notes[0]!.canReply).toBe(true)
    expect(f.adapter.read()!.rights.postNote).toEqual([])
    expect(await f.post()).toEqual({ state: 'rejected' }); expect(f.send).toHaveBeenCalledTimes(1)
  })
  it('keeps one-hour independently available when server day-end policy is unavailable', async () => {
    const f = fixture(); await f.load(); f.rights.set({ ...f.rights.read()!, rights: { ...f.rights.read()!.rights, noteExpiries: ['ONE_HOUR'] } })
    expect(await f.post({ expiry: 'END_OF_DAY' })).toEqual({ state: 'rejected' })
    f.respond(receipt(), 201); expect(await f.post()).toEqual({ state: 'unknown' }); expect(f.send).toHaveBeenCalledTimes(2)
  })
  it('binds replies to the actual parent revision and sends no audience/expiry override', async () => {
    const f = fixture(); await f.load(); f.respond(receipt({ operation_id: 'createVenuePlaceReply' }), 201)
    await f.adapter.reply({ noteId, body: 'On my way' }, f.context())
    expect(f.send.mock.calls[1]?.[0]).toMatchObject({ idempotencyKey: key, body: { text: 'On my way', expected_parent_revision: 4 } })
    expect(f.send.mock.calls[1]?.[0].body).toEqual({ text: 'On my way', expected_parent_revision: 4 })
  })
  it('projects only server-decoded inherited replies', async () => {
    const f = fixture(); await f.load()
    f.respond({ note: note(), replies: [note({ id: replyId, parent_id: noteId, text: 'Here' })], next_before: null,
      server_now: time, access_until: '2026-11-05T01:00:04Z' })
    expect(await f.adapter.loadThread(noteId, f.signal.signal)).toBe(true)
    expect(f.adapter.read()!.notes[0]!.replies[0]!.body).toBe('Here')
  })
  it('rejects a decoder-invalid audience/expiry reply thread', async () => {
    const f = fixture(); await f.load()
    f.respond({ note: note(), replies: [note({ id: replyId, parent_id: noteId, audience: 'EVERYONE' })], next_before: null,
      server_now: time, access_until: '2026-11-05T01:00:04Z' })
    expect(await f.adapter.loadThread(noteId, f.signal.signal)).toBe(false)
    expect(f.adapter.read()!.notes[0]!.replies).toEqual([])
  })
  it.each(['duplicate-art', 'duplicate-physical', 'missing'] as const)('fails closed on %s reviewed mapping', async kind => {
    const f = fixture(), original = f.associations.read()!
    f.associations.set({ ...original, bindings: kind === 'missing' ? [] : [...original.bindings,
      { landmarkId: kind === 'duplicate-art' ? artId : landmarkId('other-gate'), selection }] })
    expect(await f.load()).toBe(false); expect(f.send).not.toHaveBeenCalled()
  })
  it.each(['wrong-physical', 'wrong-revision', 'future-clock', 'expired-lease'] as const)('rejects %s responses', async kind => {
    const f = fixture()
    f.respond(kind === 'wrong-physical' ? page({ items: [note({ landmark_id: 'other' })] })
      : kind === 'wrong-revision' ? page({ items: [note({ landmark_revision: 8 })] })
      : kind === 'future-clock' ? page({ server_now: '2026-11-05T01:00:01Z' }) : page({ access_until: time }))
    expect(await f.load()).toBe(false); expect(f.adapter.read()).toBeNull()
  })
  it.each(['userId', 'eventId', 'generation', 'policyGeneration', 'gate'] as const)('rejects mismatched %s scope', async field => {
    const f = fixture(); await f.load()
    const scope = { ...f.scope, [field]: field === 'gate' ? 'suspended' : typeof f.scope[field] === 'number' ? 999 : 'other' } as PrivacyScope
    expect(await f.post({}, f.context({ scope }))).toEqual({ state: 'rejected' }); expect(f.send).toHaveBeenCalledTimes(1)
  })
  it('scrubs synchronously before withdrawal subscribers and fences a late raw response', async () => {
    const f = fixture(); await f.load(); f.respond(receipt(), 201); await f.post()
    const observations: unknown[] = []
    f.adapter.subscribe(() => observations.push([f.adapter.read(), f.adapter.ownsCommand(key)]))
    let release!: (value: { status: number; data: unknown }) => void
    f.defer(new Promise(resolve => { release = resolve }))
    const pending = f.load()
    f.lifetime.abort()
    release({ status: 200, data: page() })
    expect(await pending).toBe(false)
    expect(observations.at(-1)).toEqual([null, false])
  })
  it('withdraws the read lease at the same borrowed clock boundary', async () => {
    const f = fixture(); await f.load()
    f.clock.set({ scope: f.scope, nowMs: Date.parse('2026-11-05T01:00:05Z') })
    expect(f.adapter.read()).toBeNull(); expect(await f.post()).toEqual({ state: 'rejected' })
  })
  it('does not acquire a clock or manufacture missing client/provider data', async () => {
    const f = fixture(); f.clock.set(null); expect(await f.load()).toBe(false)
    const absent = fixture({ client: false }); expect(await absent.load()).toBe(false)
    f.associations.set(null); expect(f.adapter.read()).toBeNull()
  })
  it('requires an explicit original UUID and never resends a retained uncertain command', async () => {
    const f = fixture(); await f.load()
    expect(await f.post({}, f.context({ key: '' }))).toEqual({ state: 'rejected' })
    f.respond(receipt(), 201); await f.post(); await f.post()
    expect(await f.adapter.resolve(key, f.context())).toEqual({ state: 'unknown' })
    expect(f.send).toHaveBeenCalledTimes(2); expect(f.adapter.ownsCommand(key)).toBe(true)
  })
  it.each(['lease', 'rights', 'signal', 'provider'] as const)('keeps an uncertain command unknown after %s withdrawal without replay', async kind => {
    const f = fixture(); await f.load(); f.respond(receipt(), 201); await f.post()
    if (kind === 'lease') f.clock.set({ scope: f.scope, nowMs: Date.parse('2026-11-05T01:00:05Z') })
    else if (kind === 'rights') f.rights.set(null)
    else if (kind === 'signal') f.signal.abort()
    else f.associations.set(null)
    expect(await f.post()).toEqual({ state: 'unknown' })
    expect(await f.adapter.reply({ noteId, body: 'Retry' }, f.context())).toEqual({ state: 'unknown' })
    expect(f.send).toHaveBeenCalledTimes(2); expect(f.adapter.ownsCommand(key)).toBe(true)
  })
  it('reconciles only the retained original binding without another POST', async () => {
    const read = vi.fn<NoteStatusProducer['read']>(async () => receipt({ state: 'SUCCEEDED', resource: { id: noteId, revision: 4 } }))
    const f = fixture({ status: { read } }); await f.load(); f.respond(receipt(), 201); await f.post()
    expect(await f.adapter.resolve(otherKey, f.context({ key: otherKey }))).toEqual({ state: 'unknown' })
    expect(read).not.toHaveBeenCalled()
    expect(await f.adapter.resolve(key, f.context())).toEqual({ state: 'applied' })
    expect(read.mock.calls[0]?.[0]).toMatchObject({ key, scope: f.scope, operationId: 'createVenuePlaceNote', selection, receipt: receipt() })
    expect(read.mock.calls[0]?.[1].signal).toBe(f.signal.signal); expect(f.send).toHaveBeenCalledTimes(2)
  })
  it.each(['operation', 'receipt', 'resource'] as const)('rejects substituted status %s without replay', async kind => {
    const read = vi.fn<NoteStatusProducer['read']>(async () => receipt({ state: 'SUCCEEDED', resource: { id: noteId, revision: 4 },
      ...(kind === 'operation' ? { operation_id: 'createVenuePlaceReply' as const } : kind === 'receipt' ? { receipt_id: otherKey }
        : { resource: { id: replyId, revision: 4 } }) }))
    const f = fixture({ status: { read } }); await f.load()
    f.respond(receipt({ resource: { id: noteId, revision: 4 } }), 201); await f.post()
    expect(await f.adapter.resolve(key, f.context())).toEqual({ state: 'unknown' }); expect(f.send).toHaveBeenCalledTimes(2)
  })
  it.each(['PENDING', 'RETIRED'] as const)('keeps %s unknown with its original key', async state => {
    const read = vi.fn<NoteStatusProducer['read']>(async () => receipt({ state }))
    const f = fixture({ status: { read } }); await f.load(); f.respond(receipt(), 201); await f.post()
    expect(await f.adapter.resolve(key, f.context())).toEqual({ state: 'unknown' }); expect(f.send).toHaveBeenCalledTimes(2)
  })
  it('retains UNKNOWN across transport failure and aborted recovery without retransmission', async () => {
    const read = vi.fn<NoteStatusProducer['read']>()
    const f = fixture({ status: { read } }); await f.load(); f.send.mockRejectedValueOnce(new Error('lost response'))
    expect(await f.post()).toEqual({ state: 'unknown' }); f.signal.abort()
    expect(await f.adapter.resolve(key, f.context())).toEqual({ state: 'unknown' }); expect(read).not.toHaveBeenCalled()
    expect(f.send).toHaveBeenCalledTimes(2)
  })
})
