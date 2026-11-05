import { afterEach, describe, expect, it } from 'vitest'
import { createMeetingLeafOwner } from '../../src/app/venue-community-entry/owner'
import { canonicalId, parentPath, resolvePanel, safeReturnTarget } from '../../src/app/routes'
import { deferred, fixtureIds, meetingFixture } from './fixture'
import type { MeetingContextView } from '../../src/features/venue-invitations/model'
import type { LeafCommandResult } from '../../src/app/venue-community-entry/ports'
import type { MeetingOpenIntent } from '../../src/features/paradise-city-map/ports'
import { landmarkId } from '../../src/features/paradise-city-map/spatial/contract'

const fixtures: ReturnType<typeof meetingFixture>[] = []
const fixture = () => { const f = meetingFixture(); fixtures.push(f); return f }
afterEach(() => { for (const f of fixtures.splice(0)) f.dispose() })

describe('read-only contextual entry', () => {
  it('opens and sends in an authorized friend/event context with NO landmark or invitation', async () => {
    const f = fixture()
    const { landmark: _association, ...noPlace } = f.intent
    void _association
    f.patch({ selection: noPlace, landmarkLabel: null, landmarkDescription: null, invitationActions: [] })
    expect((await f.owner.open(noPlace)).kind).toBe('opened'); expect(f.writes).toHaveLength(0)
    expect(f.owner.read().surface).toBe('conversation'); expect(f.owner.surface('invitation')).toBe(false)
    f.owner.setDraft('See you at the show!'); expect(await f.owner.execute('send')).toBe(true)
    expect(f.writes[0]?.command).toMatchObject({ kind: 'send', invitationId: null }); expect(await f.owner.execute('create')).toBe(false)
  })
  it('still opens MESSAGE when the producer withdraws an optional place association', async () => {
    const f = fixture(), { landmark: _association, ...noPlace } = f.intent
    void _association
    f.patch({ selection: noPlace, landmarkLabel: null, landmarkDescription: null, invitationActions: [] })
    expect((await f.owner.open(f.intent)).kind).toBe('opened'); expect(f.owner.read().context?.landmarkLabel).toBeNull()
    f.owner.setDraft('Hello'); expect(await f.owner.execute('send')).toBe(true)
  })
  it('opens MESSAGE with no invitation or conversation and writes nothing', async () => {
    const f = fixture(); expect(await f.owner.open(f.intent)).toEqual({ kind: 'opened' })
    expect(f.owner.read()).toMatchObject({ surface: 'conversation', phase: 'ready', draft: '', context: { invitation: null, conversation: null } })
    expect(f.writes).toHaveLength(0); expect(f.reads).toHaveLength(1)
    expect(f.reads[0]?.request.scope).toEqual(f.scope); expect(f.reads[0]?.request.signal).toBe(f.lifetime.signal)
  })
  it('deduplicates simultaneous identical reads and changes only the requested surface', async () => {
    const f = fixture(), held = deferred<MeetingContextView | null>(); f.nextRead(held.promise)
    const first = f.owner.open(f.intent), second = f.owner.open(f.inviteIntent)
    expect(second).toBe(first); await Promise.resolve(); expect(f.reads).toHaveLength(1)
    held.resolve(f.context()); await first
    expect(f.owner.read().surface).toBe('invitation'); expect(f.writes).toHaveLength(0)
  })
  it('retains safe draft across Map close/reopen and invite/conversation navigation', async () => {
    const f = fixture(); await f.owner.open(f.intent); f.owner.setDraft('Near the gate?')
    expect(f.owner.close()).toBe(true); await f.owner.open(f.inviteIntent); f.owner.surface('conversation')
    expect(f.owner.read().draft).toBe('Near the gate?'); expect(f.reads).toHaveLength(1)
  })
  it('rejects bare user, own-user and mismatched full privacy scope opens', async () => {
    const f = fixture()
    for (const intent of [{ ...f.inviteIntent, landmarkId: '' } as MeetingOpenIntent,
      { ...f.inviteIntent, catalogRevision: '' } as MeetingOpenIntent, { ...f.intent, targetUserId: fixtureIds.actor },
      { ...f.intent, scope: { ...f.scope, policyGeneration: f.scope.policyGeneration + 1 } }]) {
      expect((await f.owner.open(intent)).kind).toBe('unavailable')
    }
    expect(f.reads).toHaveLength(0); expect(f.writes).toHaveLength(0)
  })
  it.each(['friend', 'landmark', 'catalog', 'policy', 'read', 'expiry'] as const)('rejects a cross-context %s projection', async mismatch => {
    const f = fixture(), current = f.context()
    if (mismatch === 'friend') f.patch({ friend: { ...current.friend, userId: fixtureIds.other } })
    if (mismatch === 'landmark') f.patch({ selection: { ...f.intent, landmark: { ...f.intent.landmark!, landmarkRevision: 'other-revision' } } })
    if (mismatch === 'catalog') f.patch({ selection: { ...f.intent, landmark: { ...f.intent.landmark!, catalogRevision: '8' } } })
    if (mismatch === 'policy') f.patch({ selection: { ...f.intent, scope: { ...f.scope, policyGeneration: 8 } } })
    if (mismatch === 'read') f.patch({ canRead: false })
    if (mismatch === 'expiry') f.patch({ validUntilMs: f.time() })
    expect((await f.owner.open(f.intent)).kind).toBe('unavailable'); expect(f.owner.read().context).toBeNull()
  })
  it('ignores a late prior-target read after a new selection', async () => {
    const f = fixture(), held = deferred<MeetingContextView | null>(); f.nextRead(held.promise)
    const prior = f.owner.open(f.intent); await Promise.resolve()
    const next = { ...f.intent, targetUserId: fixtureIds.other }
    f.patch({ selection: next, friend: { ...f.context().friend, userId: fixtureIds.other, displayName: 'Other friend' } })
    expect((await f.owner.open(next)).kind).toBe('opened'); held.resolve(null)
    expect((await prior).kind).toBe('stale'); expect(f.owner.read().context?.friend.userId).toBe(fixtureIds.other)
  })
  it('does not turn an absent producer into ready state', async () => {
    const f = fixture(), owner = createMeetingLeafOwner({ auth: f.auth, release: null, newCommandKey: () => fixtureIds.invitation })
    expect((await owner.open(f.intent)).kind).toBe('unavailable'); expect(owner.read().context).toBeNull(); owner.dispose()
  })
})
describe('borrowed Map focus', () => {
  it('uses the sole scoped focus port before closing and Root-owned navigation', async () => {
    const f = fixture(), calls: unknown[] = [], order: string[] = []
    const owner = createMeetingLeafOwner({ auth: f.auth, newCommandKey: () => fixtureIds.invitation,
      release: { ...f.release, mapVenue: { identity: f.intent.venue, title: 'Fixture map', image: { src: '', width: 1280, height: 853, description: 'Controlled uncalibrated illustration' },
        classification: 'ILLUSTRATED_UNCALIBRATED', landmarks: [{ id: landmarkId('synthetic-art-gate'), label: 'Main Gate', point: { x: 50, y: 50 }, meetingContext: f.intent.landmark! }] },
        mapFocus: { focus(intent) { calls.push(intent); order.push('focus'); return true } }, onMapFocused: () => { order.push('navigate') } } })
    await owner.open(f.intent); expect(owner.read().canViewOnMap).toBe(true); expect(owner.viewOnMap()).toBe(true)
    expect(calls).toEqual([{ scope: f.scope, venue: f.intent.venue, landmarkId: 'synthetic-art-gate' }])
    expect(f.intent.landmark!.landmarkId).toBe('synthetic-real-main-gate')
    expect(order).toEqual(['focus', 'navigate']); expect(owner.read().active).toBe(false); expect(f.writes).toHaveLength(0); owner.dispose()
  })
  it('never coerces a real provider landmark ID into a Map art ID without an explicit mapping', async () => {
    const f = fixture(); let calls = 0
    const owner = createMeetingLeafOwner({ auth: f.auth, newCommandKey: () => fixtureIds.invitation,
      release: { ...f.release, mapFocus: { focus() { calls++; return true } } } })
    await owner.open(f.intent); expect(owner.read().canViewOnMap).toBe(false)
    expect(owner.viewOnMap()).toBe(false); expect(calls).toBe(0); owner.dispose()
  })
  it('does not focus another surface while an original command is uncertain', async () => {
    const f = fixture(); let focuses = 0
    const owner = createMeetingLeafOwner({ auth: f.auth, newCommandKey: () => fixtureIds.invitation,
      release: { ...f.release, mapFocus: { focus() { focuses++; return true } } } })
    f.setWriteResult({ state: 'unknown' }); await owner.open(f.intent); owner.setDraft('Hello'); await owner.execute('send')
    expect(owner.viewOnMap()).toBe(false); expect(focuses).toBe(0); owner.dispose()
  })
})
describe('original command identity and current permissions', () => {
  it('sends an explicit message without creating an invitation', async () => {
    const f = fixture(); await f.owner.open(f.intent); f.owner.setDraft('I am near Main Gate.')
    expect(await f.owner.execute('send')).toBe(true); expect(f.writes).toHaveLength(1)
    expect(f.writes[0]?.command).toMatchObject({ kind: 'send', body: 'I am near Main Gate.', invitationId: null, invitationVersion: null })
    expect(f.owner.read().context?.messages).toHaveLength(1); expect(f.owner.read().draft).toBe('')
  })
  it('locks duplicate/reentrant sends and target switches while pending', async () => {
    const f = fixture(); await f.owner.open(f.intent); f.owner.setDraft('Hello')
    const pending = deferred<LeafCommandResult>(); f.nextWrite(pending.promise)
    const first = f.owner.execute('send'); expect(await f.owner.execute('send')).toBe(false)
    expect(f.owner.close()).toBe(false); expect((await f.owner.open({ ...f.intent, targetUserId: fixtureIds.other })).kind).toBe('unavailable')
    await f.owner.open(f.inviteIntent); f.owner.surface('conversation'); expect(f.writes).toHaveLength(1)
    pending.resolve({ state: 'applied' }); await first; expect(f.writes).toHaveLength(1)
  })
  it('retains an uncertain command and resolves only its original status', async () => {
    const f = fixture(); await f.owner.open(f.intent); f.owner.setDraft('Meet here?'); f.setWriteResult({ state: 'unknown' })
    await f.owner.execute('send'); const command = f.owner.read().command?.command
    f.owner.surface('invitation'); await f.owner.open(f.intent); expect(f.owner.close()).toBe(false)
    expect(f.owner.read().draft).toBe('Meet here?'); expect(await f.owner.execute('send')).toBe(false)
    await f.owner.resolve(); expect(f.statuses[0]?.command).toBe(command); expect(f.writes).toHaveLength(1)
    expect(f.statuses[0]?.command.selection).toEqual(f.intent)
    expect(f.statuses[0]?.request.signal).toBe(f.lifetime.signal); expect(f.owner.read().context?.messages).toHaveLength(1)
  })
  it('treats transport loss as uncertain and never auto-resends', async () => {
    const f = fixture(); await f.owner.open(f.intent); f.owner.setDraft('Hello')
    f.nextWrite(Promise.reject(new Error('PRIVATE transport payload'))); await f.owner.execute('send')
    expect(f.owner.read().command?.phase).toBe('unknown'); expect(f.owner.read().notice).toBe('unknown')
    f.owner.surface('invitation'); f.owner.surface('conversation'); expect(f.writes).toHaveLength(1)
  })
  it('uses current server actions for invitation commands and original versions', async () => {
    const f = fixture(); await f.owner.open(f.inviteIntent); await f.owner.execute('create')
    expect(f.owner.read().context?.invitation?.status).toBe('pending'); expect(await f.owner.execute('accept')).toBe(false)
    await f.owner.execute('cancel'); expect(f.writes[1]?.command).toMatchObject({ kind: 'cancel', invitationId: fixtureIds.invitation, invitationVersion: '1' })
    expect(f.owner.read().context?.invitation?.status).toBe('cancelled')
    f.owner.setDraft('Still here?'); expect(await f.owner.execute('send')).toBe(true)
  })
  it.each(['accept', 'decline'] as const)('executes explicit recipient %s with server-granted capability', async action => {
    const f = fixture(); f.patch({ invitation: { id: fixtureIds.invitation, version: '7', status: 'pending', expiresAtMs: f.time() + 50_000 }, invitationActions: [action] })
    await f.owner.openInvitation(fixtureIds.invitation, 'invitation'); expect(f.writes).toHaveLength(0)
    await f.owner.execute(action); expect(f.writes[0]?.command).toMatchObject({ kind: action, invitationVersion: '7' })
  })
  it('does not infer send rights from accepted invitation state', async () => {
    const f = fixture(); f.patch({ canSend: false, invitation: { id: fixtureIds.invitation, version: '1', status: 'accepted', expiresAtMs: f.time() + 100_000 } })
    await f.owner.open(f.intent); f.owner.setDraft('Hello'); expect(await f.owner.execute('send')).toBe(false); expect(f.writes).toHaveLength(0)
  })
  it('withdraws expired invitation actions without revoking conversation', async () => {
    const f = fixture(); f.patch({ invitation: { id: fixtureIds.invitation, version: '1', status: 'pending', expiresAtMs: f.time() + 10 }, invitationActions: ['accept'] })
    await f.owner.open(f.intent); f.advance(11)
    expect(await f.owner.execute('accept')).toBe(false); expect(f.owner.read().context?.canSend).toBe(true)
    f.owner.setDraft('Still meeting?'); expect(await f.owner.execute('send')).toBe(true)
  })
  it('preserves rejected message draft and uses adapter length policy', async () => {
    const f = fixture(); await f.owner.open(f.intent); f.owner.setDraft('x'.repeat(501))
    expect(await f.owner.execute('send')).toBe(false); expect(f.writes).toHaveLength(0)
    f.owner.setDraft('Try this'); f.setWriteResult({ state: 'rejected' }); await f.owner.execute('send')
    expect(f.owner.read().draft).toBe('Try this'); expect(f.owner.read().command?.phase).toBe('rejected')
  })
})
describe('synchronous withdrawal and cleanup', () => {
  it.each(['block', 'logout', 'clock', 'clock-scope', 'expiry', 'revoke', 'signal', 'dispose'] as const)('scrubs before callbacks on %s', async cause => {
    const f = fixture(); await f.owner.open(f.intent); f.owner.setDraft('Private draft')
    const pending = deferred<LeafCommandResult>(); f.nextWrite(pending.promise); const sent = f.owner.execute('send')
    let leaked = false
    f.owner.subscribe(() => { if (f.owner.read().phase === 'withdrawn') leaked ||= f.owner.read().context !== null || f.owner.read().draft !== '' || f.owner.read().command !== null })
    if (cause === 'block') f.auth.privacy.changePolicy(f.scope, 'block')
    if (cause === 'logout') f.auth.session.replace(f.scope.generation, null)
    if (cause === 'clock') f.loseClock()
    if (cause === 'clock-scope') f.replaceClockScope({ ...f.scope, policyGeneration: 5 })
    if (cause === 'expiry') f.advance(900_001)
    if (cause === 'revoke') f.invalidate()
    if (cause === 'signal') f.lifetime.abort()
    if (cause === 'dispose') f.owner.dispose()
    expect(f.owner.read()).toMatchObject({ phase: 'withdrawn', context: null, draft: '', command: null })
    expect(f.subscriptions()).toEqual({ clock: 0, invalidation: 0 }); expect(leaked).toBe(false)
    pending.resolve({ state: 'applied' }); await sent
    expect(f.owner.read().context).toBeNull(); expect(f.reads).toHaveLength(1)
  })
  it('suppresses a read response arriving after policy withdrawal', async () => {
    const f = fixture(), pending = deferred<MeetingContextView | null>(); f.nextRead(pending.promise)
    const opened = f.owner.open(f.intent); await Promise.resolve(); f.auth.privacy.changePolicy(f.scope, 'block')
    pending.resolve(f.context()); expect((await opened).kind).toBe('stale'); expect(f.owner.read().context).toBeNull()
  })
})
describe('new route safety', () => {
  it('inherits strict canonical UUID and safe return handling for both routes', () => {
    const detail = `/meet/invitations/${fixtureIds.invitation}`, conversation = `${detail}/conversation`
    expect(canonicalId(fixtureIds.invitation)).toBe(true); expect(resolvePanel(detail)?.valid).toBe(true)
    expect(resolvePanel(conversation)?.valid).toBe(true); expect(parentPath(conversation)).toBe(detail)
    expect(safeReturnTarget(conversation)).toBe(conversation)
    for (const unsafe of ['/meet/invitations/nope', `${detail}?token=private`, `${detail}/../other`, `//evil.example${detail}`,
      '/meet/invitations/0000000A-0000-4000-8000-000000000005']) {
      expect(safeReturnTarget(unsafe)).toBe('/world')
    }
  })
})


it('scrubs normalized adapter windows before withdrawal subscribers', async () => {
  const f = meetingFixture(); let cleared = false
  f.release.adapter.withdraw = () => { cleared = true }
  await f.owner.open(f.intent)
  const stop = f.owner.subscribe(() => { if (f.owner.read().phase === 'withdrawn') expect(cleared).toBe(true) })
  f.owner.withdraw(); expect(cleared).toBe(true); stop(); f.dispose()
})

it('loads a further authorized message page without a command or a replacement lifetime', async () => {
  const f = meetingFixture(); f.patch({ conversation: { id: 'fixture-conversation', earlierCursor: null, nextCursor: '1' } })
  let calls = 0
  f.release.adapter.loadMore = async (context, request) => {
    calls++; expect(request.signal).toBe(f.lifetime.signal)
    return { ...context, messages: [{ id: 'next-message', authorId: fixtureIds.friend, body: 'Next page', timeLabel: 'Now' }],
      conversation: { id: 'fixture-conversation', earlierCursor: null, nextCursor: null } }
  }
  await f.owner.open(f.intent); expect(await f.owner.loadMore()).toBe(true)
  expect(f.owner.read().context?.messages[0]?.body).toBe('Next page'); expect(calls).toBe(1); expect(f.writes).toHaveLength(0); f.dispose()
})

it.each(['dangling', 'no-invitation'] as const)('rejects an invalid shared-place message projection: %s', async reason => {
  const f = fixture(); f.patch({ sharedPlaceMessageId: 'message-linked',
    messages: [{ id: reason === 'dangling' ? 'another-message' : 'message-linked', authorId: fixtureIds.actor, body: 'Hello', timeLabel: 'Now' }],
    invitation: reason === 'dangling' ? { id: fixtureIds.invitation, version: '1', status: 'accepted', expiresAtMs: f.time() + 50000 } : null })
  expect((await f.owner.open(f.intent)).kind).toBe('unavailable'); expect(f.owner.read().context).toBeNull()
})
