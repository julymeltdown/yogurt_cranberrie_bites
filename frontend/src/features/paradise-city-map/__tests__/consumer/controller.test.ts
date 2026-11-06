import { afterEach, expect, it } from 'vitest'
import { harness, mainGate, person, testVenue } from './harness'
import { locationLabel } from '../../model/controller'

const owners: ReturnType<typeof harness>[] = []
const setup = () => { const value = harness(); owners.push(value); return value }
afterEach(() => owners.splice(0).forEach(h => h.dispose()))
it('constructs without a location watch, permission request or capability mutation', () => {
  const h = setup(); expect(h.watches).toBe(0); expect(h.calls).toEqual([])
})
it('keeps uncalibrated siteplan selection usable when production capabilities are absent', () => {
  const h = setup(); h.publish(null); h.controller.select({ kind: 'landmark', id: mainGate })
  expect(h.controller.read().selection).toEqual({ kind: 'landmark', id: 'test-main-gate' }); expect(h.controller.read().snapshot).toBeNull()
})
it.each(['scope', 'policy', 'venue', 'clock', 'expiry', 'withdrawal'] as const)('scrubs private selection and drafts on %s changes', kind => {
  const h = setup(); h.controller.select({ kind: 'friend', id: 'friend-test' }); h.controller.compose('test-main-gate'); h.controller.edit('Private draft')
  if (kind === 'scope') h.publish({ ...h.snapshot, scope: { ...h.snapshot.scope, userId: 'different' } })
  if (kind === 'policy') h.publish({ ...h.snapshot, scope: { ...h.snapshot.scope, policyGeneration: 1 } })
  if (kind === 'venue') h.publish({ ...h.snapshot, venue: { ...h.snapshot.venue, revision: 2 } })
  if (kind === 'clock') h.publish({ ...h.snapshot, serverNowMs: null })
  if (kind === 'expiry') h.publish({ ...h.snapshot, accessUntilMs: h.snapshot.serverNowMs })
  if (kind === 'withdrawal') h.setCurrent(false)
  const state = h.controller.read(); expect(state.people).toEqual([]); expect(state.notes).toEqual([]); expect(state.draft).toBeNull(); expect(state.selection).toBeNull()
})
it('does not resurrect private selection after a synchronous withdraw/rebind', () => {
  const h = setup(); h.controller.select({ kind: 'friend', id: 'friend-test' }); h.setCurrent(false); h.setCurrent(true)
  expect(h.controller.read().selection).toBeNull(); expect(h.controller.read().people).toHaveLength(1)
})
it('derives current names from the latest roster and evicts departed selections', () => {
  const h = setup(); h.controller.select({ kind: 'friend', id: 'friend-test' })
  h.publish({ ...h.snapshot, friends: [{ ...person(), displayName: 'New name' }] }); expect(h.controller.read().people[0]?.displayName).toBe('New name')
  h.publish({ ...h.snapshot, friends: [] }); expect(h.controller.read().selection).toBeNull()
})
it('uses server time for stale coordinates and note expiry, without a local-clock grant', () => {
  const h = setup(), now = h.snapshot.serverNowMs!
  h.publish({ ...h.snapshot, serverNowMs: now + 3600001 })
  expect(h.controller.read().people[0]?.point).toBeNull(); expect(h.controller.read().people[0]?.locationSource).toBe('last-known'); expect(h.controller.read().notes).toEqual([])
  expect(locationLabel(h.controller.read().people[0]!, now + 3600001)).toContain('Last update')
})
it('requires explicit consent and current server permission before a bounded phone watch', async () => {
  const h = setup(); await h.controller.startPhone(false); expect(h.watches).toBe(0)
  await h.controller.startPhone(true); expect(h.watches).toBe(1); expect(h.calls.map(c => c.name)).toEqual(['beginPhoneSharing'])
})
it('drops late phone fixes and self-marker data immediately after Pause', async () => {
  const h = setup(); await h.controller.startPhone(true); const before = h.calls.length
  const paused = h.controller.pause(); h.emitFix({ latitude: 1, longitude: 2, accuracy: 3, capturedAtMs: 10 }); await paused
  expect(h.controller.self()?.point).toBeNull(); expect(h.calls.slice(before).map(c => c.name)).toEqual(['pause'])
})
it('stops phone upload on background and does not automatically resume', async () => {
  const h = setup(); await h.controller.startPhone(true); h.background(); await Promise.resolve(); h.emitFix({ latitude: 1, longitude: 2, accuracy: 3, capturedAtMs: 10 })
  expect(h.watches).toBe(1); expect(h.calls.filter(c => c.name === 'publishPhoneFix')).toHaveLength(0)
})
it('allows Pause while an unrelated note command is pending', async () => {
  const h = setup(); await h.controller.startPhone(true)
  let finish!: (value: { state: 'applied' }) => void
  h.setResult(async name => name === 'postNote' ? new Promise(resolve => { finish = resolve }) : { state: 'applied' })
  h.controller.compose('test-main-gate'); h.controller.edit('Pending note'); const posting = h.controller.submit()
  await h.controller.pause(); expect(h.calls.at(-1)?.name).toBe('pause'); expect(h.controller.self()?.point).toBeNull(); finish({ state: 'applied' }); await posting
})
it('keeps an unknown note outcome and original key locked until status-only recovery', async () => {
  const h = setup(); h.setResult(async () => ({ state: 'unknown' })); h.controller.compose('test-main-gate'); h.controller.edit('One intended note')
  await h.controller.submit(); await h.controller.submit(); h.controller.cancelDraft(); expect(h.calls).toHaveLength(1); expect(h.controller.read().draft?.body).toBe('One intended note')
  const key = h.calls[0]!.context.key; h.setResult(async () => ({ state: 'applied' })); await h.controller.resolve()
  expect(h.calls[1]?.name).toBe('resolve'); expect(h.calls[1]?.input).toBe(key); expect(h.calls[1]?.context.key).toBe(key); expect(h.controller.read().draft).toBeNull()
})
it('reply uses only its parent ID and body, without an independent audience or expiry', async () => {
  const h = setup(); h.controller.compose('test-main-gate', 'note-test'); h.controller.edit('A reply'); await h.controller.submit()
  expect(h.calls[0]?.name).toBe('reply'); expect(h.calls[0]?.input).toEqual({ noteId: 'note-test', body: 'A reply' })
})
it('removes a reply draft when the parent expires or permission is withdrawn', () => {
  const h = setup(); h.controller.compose('test-main-gate', 'note-test'); h.controller.edit('Private reply')
  h.publish({ ...h.snapshot, notes: [] }); expect(h.controller.read().draft).toBeNull()
})
it('MESSAGE opens the scoped borrowed leaf without creating an invitation or writing', async () => {
  const h = setup(); await h.controller.openMeeting('MESSAGE', person(), undefined, 'map-friend')
  expect(h.opens).toHaveLength(1); expect(h.opens[0]?.action).toBe('MESSAGE'); expect(h.opens[0]?.scope.policyGeneration).toBe(0); expect(h.calls).toEqual([])
  expect(h.opens[0]).not.toHaveProperty('invitationId'); expect(h.opens[0]).not.toHaveProperty('position'); expect(h.opens[0]).not.toHaveProperty('landmark')
})
it('does not make an unapproved illustrated landmark a MESSAGE prerequisite', async () => {
  const h = setup(); await h.controller.openMeeting('MESSAGE', person(), testVenue.landmarks[2]!, 'map-friend'); expect(h.opens).toHaveLength(1); expect(h.opens[0]).not.toHaveProperty('landmark')
})
it('associates a reviewed landmark with MESSAGE without creating or sending anything', async () => {
  const h = setup(); await h.controller.openMeeting('MESSAGE', person(), testVenue.landmarks[0]!, 'map-friend')
  expect(h.opens[0]?.action === 'MESSAGE' && h.opens[0].landmark).toEqual(testVenue.landmarks[0]!.meetingContext); expect(h.calls).toEqual([])
})
it('requires a reviewed fixed landmark for INVITE', async () => {
  const h = setup(); await h.controller.openMeeting('INVITE', person(), undefined, 'map-friend'); await h.controller.openMeeting('INVITE', person(), testVenue.landmarks[2]!, 'map-friend'); expect(h.opens).toEqual([])
  await h.controller.openMeeting('INVITE', person(), testVenue.landmarks[0]!, 'map-friend'); expect(h.opens[0]?.action).toBe('INVITE')
  expect(h.opens[0]).toMatchObject(testVenue.landmarks[0]!.meetingContext!); expect(h.opens[0]).not.toHaveProperty('landmarkId', mainGate)
})
it('rejects a bare-user or unauthorized MESSAGE even with a reviewed landmark', async () => {
  const h = setup(); await h.controller.openMeeting('MESSAGE', { userId: 'not-a-current-friend' }, testVenue.landmarks[0]!, 'map-friend')
  h.publish({ ...h.snapshot, friends: [{ ...person(), canMessage: false }] }); await h.controller.openMeeting('MESSAGE', person(), testVenue.landmarks[0]!, 'map-friend'); expect(h.opens).toEqual([])
})
it('opens MESSAGE for an authorized friend without location or check-in permission', async () => {
  const h = setup(); h.publish({ ...h.snapshot, friends: [{ ...person(), point: null, landmarkId: null, locationSource: 'unavailable', canMeet: false }], rights: { ...h.snapshot.rights, checkIn: [], sharePhone: false } })
  await h.controller.openMeeting('MESSAGE', person(), undefined, 'map-friend'); expect(h.opens).toHaveLength(1); expect(h.calls).toEqual([])
})
it('keeps independently authorized friend MESSAGE available when self sharing is revoked', async () => {
  const h = setup(); h.publish({ ...h.snapshot, sharing: 'revoked', rights: { ...h.snapshot.rights, sharePhone: false } })
  expect(h.controller.read().people).toHaveLength(1); expect(h.controller.self()?.point).toBeNull()
  await h.controller.openMeeting('MESSAGE', person(), undefined, 'map-friend'); expect(h.opens).toHaveLength(1)
})
it('never renders a future-dated self fix as a current shared position', () => {
  const h = setup(); h.publish({ ...h.snapshot, sharing: 'active', self: { ...person(), updatedAtMs: h.snapshot.serverNowMs! + 1 } }); expect(h.controller.self()?.point).toBeNull()
})
it('locked borrowed leaf prevents primary tab changes and close', () => {
  const h = setup(); h.publishLeaf({ active: true, locked: true, content: null }); h.controller.setTab('NOTES'); expect(h.controller.read().tab).toBe('MAP'); expect(h.controller.closeMeeting()).toBe(false)
})
it('renders a failed friend gate as unavailable instead of an empty roster', () => {
  const h = setup(); h.controller.select({ kind: 'friend', id: 'friend-test' })
  h.setAvailability({ state: 'unavailable', reason: 'friend-gate-unavailable', readAtMs: null, ageMs: null, staleAtMs: null, refreshing: false, canRefresh: true })
  const state = h.controller.read(); expect(state.people).toEqual([]); expect(state.selection).toBeNull()
  expect(state.availability).toMatchObject({ state: 'unavailable', reason: 'friend-gate-unavailable' })
})
it('never labels an aged snapshot as current and keeps only its last-known markers', () => {
  const h = setup()
  h.setAvailability({ state: 'stale', reason: null, readAtMs: h.snapshot.serverNowMs!, ageMs: 20000, staleAtMs: h.snapshot.serverNowMs! + 10000, refreshing: false, canRefresh: true })
  const state = h.controller.read(), person = state.people[0]!
  expect(person.locationSource).toBe('last-known'); expect(person.freshUntilMs).toBeNull(); expect(person.point).not.toBeNull()
  expect(locationLabel(person, h.snapshot.serverNowMs!)).toContain('Last update')
})
it('refetches on demand, recovers to a fresh snapshot and never duplicates actors', async () => {
  const h = setup(); h.publish({ ...h.snapshot, friends: [person(), person()] })
  expect(h.controller.read().people).toHaveLength(1)
  h.setAvailability({ state: 'stale', reason: 'offline', readAtMs: h.snapshot.serverNowMs!, ageMs: 30000, staleAtMs: h.snapshot.serverNowMs! + 10000, refreshing: false, canRefresh: true })
  await h.controller.refreshAuthority(); expect(h.refreshes).toBe(1)
  expect(h.controller.read().availability.state).toBe('ready'); expect(h.controller.read().people).toHaveLength(1)
  expect(h.controller.read().notice).toContain('refreshed')
  h.setRefreshResult(false); await h.controller.refreshAuthority(); expect(h.controller.read().notice).toContain('unavailable')
})
it('reports denied device permission, clears current position and asks the server to stop sharing', async () => {
  const h = setup(); await h.controller.startPhone(true); expect(h.controller.self()?.point).not.toBeNull()
  h.deny(); await Promise.resolve(); await Promise.resolve()
  expect(h.controller.read().sharingFailure).toBe('permission-denied'); expect(h.controller.self()?.point).toBeNull()
  expect(h.calls.some(call => call.name === 'pause')).toBe(true)
})
it('reports a server refusal without starting a phone watch or sharing', async () => {
  const h = setup(); h.setResult(async () => ({ state: 'rejected' }))
  await h.controller.startPhone(true); expect(h.watches).toBe(0); expect(h.controller.read().sharingFailure).toBe('server-refused')
  expect(h.controller.read().notice).toContain('did not authorize')
})
it('revokes sharing explicitly and removes current access immediately', async () => {
  const h = setup(); await h.controller.startPhone(true); const before = h.calls.length
  await h.controller.revoke()
  expect(h.calls.slice(before).map(call => call.name)).toEqual(['revoke']); expect(h.controller.self()?.point).toBeNull()
  expect(h.controller.read().snapshot?.sharing).toBe('revoked'); expect(h.controller.read().notice).toContain('revoked')
})
it('requesting a friend location never grants sharing consent', async () => {
  const h = setup(); await h.controller.requestLocation('friend-test')
  expect(h.calls.map(call => call.name)).toEqual(['requestLocation']); expect(h.controller.read().snapshot?.sharing).toBe('off')
  expect(h.watches).toBe(0)
})
it('aborts a pending mutation on scope withdrawal and ignores its late completion', async () => {
  const h = setup(); let finish!: (value: { state: 'applied' }) => void
  h.setResult(async () => new Promise(resolve => { finish = resolve })); h.controller.compose('test-main-gate'); h.controller.edit('Secret'); const action = h.controller.submit()
  h.setCurrent(false); expect(h.calls[0]?.context.signal.aborted).toBe(true); finish({ state: 'applied' }); await action
  expect(h.controller.read().draft).toBeNull(); expect(h.controller.read().notice).not.toContain('Note posted')
})
