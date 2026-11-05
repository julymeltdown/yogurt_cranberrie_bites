// CONTROLLED FIXTURE. Real Auth/privacy/community clock and ART loader; synthetic social capabilities/M projection.
import { createSessionController } from '../../src/features/auth/model/session'
import { checkedBootstrap, userId, eventId } from '../../src/app/community-authority/__tests__/fixtures'
import { beginCommunityBootstrap } from '../../src/app/community-adoption/bootstrap'
import { createAuthorityTimeline } from '../../src/app/community-authority'
import { createSiteplanLoader, type DecodedSiteplanImage } from '../../src/features/paradise-city-map/spatial/image-resource'
import { productionSiteplanRegistration } from '../../src/features/paradise-city-map/spatial/registration'
import type { CapabilitySnapshot, MeetingOpenIntent, PhoneLocationPort, VenueCapabilities } from '../../src/features/paradise-city-map/ports'
import type { MeetingLeafRelease } from '../../src/app/venue-community-entry'
import { createVenueRootInput } from '../../src/app/venue-root'
import { manifestText, imageBase64 } from 'virtual:venue-root-art'

export const friendId = '00000000-0000-4000-8000-000000000099'
export async function fixture(decode?: (bytes: Uint8Array, signal: AbortSignal) => Promise<DecodedSiteplanImage>) {
  let requests = 0, keys = 0, phoneStarts = 0, phoneDisposes = 0, navigations = 0, reads = 0
  const auth = createSessionController({ eventId, fetch: async () => { requests++; throw Error('No production transport in controlled fixture') },
    cookiePolicy: { available: true, onInvalidate: () => () => {}, run: (_kind, work) => work(), dispose() {} } })
  auth.session.replace(auth.session.read().generation, { userId, eventId, accessToken: 'SYNTHETIC_MEMORY_ONLY' })
  const scope = auth.privacy.capture(), lifetime = new AbortController()
  const timeline = beginCommunityBootstrap({ privacy: auth.privacy, scope, signal: lifetime.signal,
    profile: { classification: 'TEST_ONLY', evidence: 'venue-root-controlled-fixture', maxAgeMs: 60000,
      maxObservationGapMs: 60000, uncertaintyMs: 0, maxElapsedRate: 1 } }, gap => createAuthorityTimeline({
    observe: () => ({ nowMs: performance.now(), origin: performance.timeOrigin }), maxObservationGapMs: gap,
    onInterruption: () => () => {},
  }))
  const community = timeline.complete(await checkedBootstrap()), clock = community.read()!.clock
  const loader = createSiteplanLoader(productionSiteplanRegistration, { origin: window.location.origin,
    fetch: async url => String(url).endsWith('.json') ? new Response(manifestText) : new Response(Uint8Array.from(atob(imageBase64), c => c.charCodeAt(0))),
    ...(decode ? { decode } : {}) })
  const siteplan = await loader.load(lifetime.signal), gate = siteplan.snapshot.siteplan.landmarks.find(p => p.id === 'main-gate')!
  const provider = { landmarkId: 'reviewed-place-fixture', catalogRevision: '7', landmarkRevision: '12' }
  const invalidations = new Set<() => void>(), capabilityListeners = new Set<() => void>()
  const reject = async () => ({ state: 'rejected' as const })
  const snapshot = (): CapabilitySnapshot => ({ scope, venue: siteplan.snapshot.siteplan, classification: 'CONTROLLED_FIXTURE',
    serverNowMs: clock.read()?.nowMs ?? null, accessUntilMs: (clock.read()?.nowMs ?? 0) + 60000, self: null,
    friends: [{ userId: friendId, displayName: 'Fixture friend', avatar: null, point: null, landmarkId: null,
      locationSource: 'unavailable', updatedAtMs: null, freshUntilMs: null, visible: true, canRequest: false, canMessage: true, canMeet: true }],
    notes: [], sharing: 'off', rights: { sharePhone: false, pause: false, checkIn: [], postNote: [], noteAudiences: [], noteExpiries: [] } })
  const capabilities: VenueCapabilities = { read: snapshot, subscribe(fn) { capabilityListeners.add(fn); return () => { capabilityListeners.delete(fn) } },
    beginPhoneSharing: reject, publishPhoneFix: reject, pause: reject, requestLocation: reject, checkIn: reject, postNote: reject, reply: reject, resolve: reject }
  const phone: PhoneLocationPort = { read: () => 'idle', subscribe: () => () => {}, start() { phoneStarts++ }, stop() {}, dispose() { phoneDisposes++ } }
  let navigate = () => { navigations++ }
  const meeting: MeetingLeafRelease = { scope, signal: lifetime.signal, clock, classification: 'CONTROLLED_FIXTURE',
    invalidations: { subscribe(fn) { invalidations.add(fn); return () => { invalidations.delete(fn) } } }, adapter: {
      async readSelection(intent) { reads++; const associated = intent.action === 'INVITE' ? intent : intent.landmark
        return { contextId: 'fixture-context', contextVersion: '7', selection: intent, landmarkLabel: associated ? 'Reviewed fixture place' : null,
          landmarkDescription: associated ? 'Controlled provider association; not actual guidance.' : null,
          self: { userId, displayName: 'You', portrait: null }, friend: { userId: friendId, displayName: 'Fixture friend', portrait: null },
          validUntilMs: clock.read()!.nowMs + 60000, canRead: true, canSend: true, invitationActions: associated ? ['create'] : [],
          invitation: null, messages: [], conversation: null, messagePolicy: { hint: 'Controlled fixture policy', accepts: text => text.trim().length > 0 } }
      }, async readInvitation() { return null }, execute: async () => ({ state: 'unknown' }), status: async () => ({ state: 'unknown' }),
    } }
  const borrowed = { auth, scope, signal: lifetime.signal, community, siteplan, capabilities, meeting, phone,
    meetingPlaces: new Map([[gate.id, provider]]), newCommandKey: () => `00000000-0000-4000-8000-${(++keys).toString().padStart(12, '0')}`,
    navigateToMap: () => navigate() }
  const input = createVenueRootInput(borrowed)
  const intent = (action: 'MESSAGE' | 'INVITE' = 'MESSAGE'): MeetingOpenIntent => action === 'MESSAGE'
    ? { action, scope, venue: input.venue.identity, targetUserId: friendId, origin: 'people-profile' }
    : { action, scope, venue: input.venue.identity, targetUserId: friendId, origin: 'map-landmark', ...provider }
  return { input, borrowed, auth, community, clock, siteplan, gate, meeting, lifetime, intent,
    navigate(fn: () => void) { navigate = fn }, counts: () => ({ requests, keys, phoneStarts, phoneDisposes, navigations, reads }),
    revoke() { auth.privacy.changePolicy(scope, 'consent-revoked') },
    dispose() { lifetime.abort(); loader.dispose(); community.dispose(); auth.dispose() },
  }
}
