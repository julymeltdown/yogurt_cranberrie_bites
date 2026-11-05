// TEST ONLY: canonical App routes and SAME shell Auth/bootstrap clock; no second owner.
import { useEffect, useMemo, useState, type ReactNode } from 'react'
import { createBrowserRouter } from 'react-router-dom'
import { createAppRoutes } from '../../src/app/router'
import type { ShellPorts } from '../../src/app/ports'
import type { PrivacyScope } from '../../src/shared/query/privacy'
import type { SessionController } from '../../src/features/auth/model/session'
import type { CommunityBootstrapBinding, CommunityClockProfile } from '../../src/app/community-adoption/bootstrap'
import { ParadiseCityMapPanel, ParadiseCityVenueProvider } from '../../src/app/venue-entry/ParadiseCityMapPanel'
import type { CapabilitySnapshot, CommandContext, CommandResult, MeetingConsumerPort, MeetingOpenIntent, VenueCapabilities, VenueView } from '../../src/features/paradise-city-map/ports'
import { initialSnapshot, person } from '../../src/features/paradise-city-map/__tests__/consumer/harness'
import { createSiteplanLoader, type LoadedSiteplan } from '../../src/features/paradise-city-map/spatial/image-resource'
import { productionSiteplanRegistration } from '../../src/features/paradise-city-map/spatial/registration'
import { projectLoadedSiteplan } from '../../src/features/paradise-city-map/model/siteplan-view'
import { preset } from '../../src/features/avatar/__tests__/reviewed-production/fixtures'

const testProfile: CommunityClockProfile = Object.freeze({ classification: 'TEST_ONLY', evidence: 'Paradise City synthetic actual-App fixture; not measured clock or physical service qualification', maxAgeMs: 180000, uncertaintyMs: 1000, maxElapsedRate: 1.01, maxObservationGapMs: 5000 })
export function createVenueFixtureRouter(ports: ShellPorts) {
  if (!import.meta.env.DEV) throw Error('Development-only composition')
  return createBrowserRouter(createAppRoutes({ ...ports, communityClock: testProfile }, { panels: { '/world/map': <ParadiseCityMapPanel /> } }))
}
function makeFixture(scope: PrivacyScope, binding: CommunityBootstrapBinding, venue: VenueView) {
  if (!scope.userId) throw Error('Authenticated fixture identity unavailable')
  const userId: string = scope.userId
  const listeners = new Set<() => void>(), calls: { name: string; input: unknown; key: string }[] = [], opens: MeetingOpenIntent[] = []
  const initial = initialSnapshot(), now = binding.read()?.clock.read()?.nowMs
  if (now === undefined) throw Error('Original canonical community clock unavailable')
  const main = venue.landmarks.find(place => place.id === 'main-gate'), sub = venue.landmarks.find(place => place.id === 'sub-gate')
  if (!main || !sub) throw Error('Actual producer landmarks missing')
  const portrait = (index: number) => preset(index)
  const friend = { ...person(userId + '-friend', 'inkparade'), avatar: portrait(1), point: main.point, landmarkId: main.id, updatedAtMs: now - 40000, freshUntilMs: now + 120000 }
  let snapshot: CapabilitySnapshot | null = { ...initial, scope, venue: venue.identity, serverNowMs: now, accessUntilMs: now + 150000,
    rights: { ...initial.rights, checkIn: [main.id], postNote: [main.id, sub.id], noteExpiries: ['ONE_HOUR'] },
    self: { ...person(userId, 'You'), avatar: portrait(0), point: main.point, landmarkId: main.id, updatedAtMs: now, freshUntilMs: now + 120000 },
    friends: [friend, { ...friend, userId: userId + '-peer', displayName: 'Gerardway', avatar: portrait(2), point: sub.point, landmarkId: sub.id },
      { ...friend, userId: userId + '-stale', displayName: 'frankie_13', avatar: portrait(3), point: null, landmarkId: null, locationSource: 'unavailable', updatedAtMs: null, freshUntilMs: null }],
    notes: initial.notes.map(note => ({ ...note, body: 'Trading stickers here until 18:30.', landmarkId: main.id, author: friend, createdAtMs: now - 300000, expiresAtMs: now + 3600000 })) }
  let uncertain = false
  function publish() { for (const listener of listeners) listener() }
  async function command(name: string, input: unknown, context: CommandContext): Promise<CommandResult> {
    if (context.signal.aborted || !binding.isCurrent() || !snapshot) return { state: 'rejected' }
    calls.push({ name, input, key: context.key })
    if (uncertain && name !== 'resolve') return { state: 'unknown' }
    const time = binding.read()?.clock.read()?.nowMs
    if (time === undefined) return { state: 'rejected' }
    const current = snapshot
    if (name === 'beginPhoneSharing') snapshot = { ...snapshot, sharing: 'active' }
    if (name === 'pause') snapshot = { ...snapshot, sharing: 'paused' }
    if (name === 'postNote') {
      const note = input as Parameters<VenueCapabilities['postNote']>[0]
      if (note.expiry === 'END_OF_DAY') return { state: 'rejected' } // No invented venue-day policy.
      snapshot = { ...current, notes: [...current.notes, { id: 'synthetic-note-' + calls.length, landmarkId: note.landmarkId, body: note.body, audience: note.audience, author: { userId, displayName: 'You', avatar: portrait(0) }, createdAtMs: time, expiresAtMs: time + 3600000, canMeet: false, canReply: true, replies: [] }] }
    }
    if (name === 'reply') {
      const reply = input as Parameters<VenueCapabilities['reply']>[0]
      snapshot = { ...current, notes: current.notes.map(note => note.id !== reply.noteId ? note : { ...note, replies: [...note.replies, { id: 'synthetic-reply-' + calls.length, body: reply.body, createdAtMs: time, author: { userId, displayName: 'You', avatar: portrait(0) } }] }) }
    }
    publish(); return { state: 'applied' }
  }
  const capabilities: VenueCapabilities = {
    read: () => snapshot, subscribe(listener) { listeners.add(listener); return () => { listeners.delete(listener) } },
    beginPhoneSharing: ctx => command('beginPhoneSharing', null, ctx), publishPhoneFix: (fix, ctx) => command('publishPhoneFix', fix, ctx),
    pause: (reason, ctx) => command('pause', reason, ctx), requestLocation: (id, ctx) => command('requestLocation', id, ctx),
    checkIn: (id, ctx) => command('checkIn', id, ctx), postNote: (input, ctx) => command('postNote', input, ctx), reply: (input, ctx) => command('reply', input, ctx), resolve: (key, ctx) => command('resolve', key, ctx),
  }
  // Only borrowed-leaf entry is exercised here. Invitation/conversation UI remains204-owned.
  const meeting: MeetingConsumerPort = { read: () => null, subscribe: () => () => {}, open: async intent => { opens.push(intent); return { kind: 'unavailable' } }, close: () => true }
  return { capabilities, meeting, api: { state: () => ({ calls, opens, classification: 'CONTROLLED_FIXTURE', siteplan: { identity: venue.identity, landmarks: venue.landmarks.map(place => ({ id: place.id, label: place.label, point: place.point, illustration: place.illustration })), imageWidth: venue.image.width, imageHeight: venue.image.height, sourceKind: 'VERIFIED_PRODUCER_IMAGE_LEASE' }, clockFromOriginalBinding: binding.read()?.clock.read() ?? null }),
    unavailable() { snapshot = null; publish() }, uncertain(value: boolean) { uncertain = value },
    noLocation() { if (snapshot) { snapshot = { ...snapshot, friends: snapshot.friends.map(p => ({ ...p, point: null, landmarkId: null, locationSource: 'unavailable' })) }; publish() } },
    cluster() { if (snapshot) { snapshot = { ...snapshot, friends: snapshot.friends.map((p, i) => i < 2 ? { ...p, point: { x: main.point.x + i * 8, y: main.point.y } } : p) }; publish() } },
    withdraw() { snapshot = null; publish() },
  }, dispose() { snapshot = null; publish(); listeners.clear() } }
}
export function VenueFixtureBoundary({ auth, scope, binding, children }: { readonly auth: SessionController | undefined; readonly scope: PrivacyScope | null; readonly binding: CommunityBootstrapBinding | null; readonly children: ReactNode }) {
  const [loaded, setLoaded] = useState<{ binding: CommunityBootstrapBinding; value: LoadedSiteplan } | null>(null)
  useEffect(() => {
    if (!binding || !auth || !scope) return
    const loader = createSiteplanLoader(productionSiteplanRegistration, { origin: location.origin })
    let active = true
    void loader.load(binding.signal).then(value => { if (active && auth.privacy.isCurrent(scope)) setLoaded({ binding, value }) }).catch(error => { if (active) console.error('Siteplan fixture load failed', error) })
    return () => { active = false; loader.dispose() }
  }, [auth, scope, binding])
  const venue = useMemo(() => loaded?.binding === binding ? projectLoadedSiteplan(loaded.value) : null, [loaded, binding])
  const fixture = useMemo(() => auth && scope && binding && venue ? makeFixture(scope, binding, venue) : null, [auth, scope, binding, venue])
  const input = useMemo(() => auth && scope && binding && fixture && venue ? { auth, scope, community: binding, signal: binding.signal, venue, capabilities: fixture.capabilities, meeting: fixture.meeting } : null, [auth, scope, binding, fixture, venue])
  useEffect(() => { if (!fixture) return; Object.assign(window, { __PARADISE_CITY_FIXTURE__: fixture.api }); return () => { Reflect.deleteProperty(window, '__PARADISE_CITY_FIXTURE__') } }, [fixture])
  return <ParadiseCityVenueProvider input={input}>{children}</ParadiseCityVenueProvider>
}
