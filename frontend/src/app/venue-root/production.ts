import type { SessionController } from '../../features/auth/model/session'
import type { HttpRequest, HttpClient } from '../../shared/api/http'
import type { PrivacyScope } from '../../shared/query/privacy'
import type { CommunityBootstrapBinding } from '../community-adoption/bootstrap'
import type { MeetingClockPort } from '../meeting-entry/ports'
import type { MeetingLeafRelease } from '../venue-community-entry'
import type { LandmarkId } from '../../features/paradise-city-map/spatial/contract'
import type {
  MeetingLandmarkContext, MeetingOpenIntent, PhoneLocationPort, ReadPort, VenueAuthority,
  VenueCapabilities, VenueView,
} from '../../features/paradise-city-map/ports'
import type { LocationAssociations, LocationPeople, LocationSharingAudience } from '../../features/paradise-city-map/model/location-adapter'
import type { NoteCreationRights, ReviewedNoteAssociations } from '../venue-notes-adapter'
import { createHttpLocationTransport } from '../../shared/api/paradise-city-v1/location-http'
import { createLocationClient, type LandmarkGrant, type LocationSnapshot } from '../../shared/api/paradise-city-v1/location'
import { createHttpNotesTransport, createNotesClient } from '../../shared/api/paradise-city-v1/notes'
import { createHttpMeetingTransport } from '../../shared/api/paradise-city-v1/meeting-http'
import { createVenueLocationAdapter } from '../../features/paradise-city-map/model/location-adapter'
import { createVenueNotesAdapter } from '../venue-notes-adapter'
import { aggregateVenueCapabilities } from '../venue-entry/capability-adapters'
import { createBrowserLocation } from '../venue-entry/browser-location'
import { createNormalizedMeetingAdapter, type MeetingPresentation } from '../venue-community-entry'
import { createMeetingClient, type MeetingInvitationSelection, type MeetingSelection } from '../../shared/api/paradise-city-v1/meeting'
import { TransportError } from '../../shared/api/errors'
import { decodeParticipant } from '../../features/me/model/contract'
import { decodeProfile } from '../../features/people/contract'

type Identity = LocationPeople['self']
type Lease = Pick<LocationPeople, 'scope' | 'venue' | 'accessUntilMs'>
const sameScope = (a: PrivacyScope, b: PrivacyScope) => a.userId === b.userId && a.eventId === b.eventId
  && a.generation === b.generation && a.policyGeneration === b.policyGeneration && a.gate === b.gate
const uuidPattern = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/
const noSubscribe = () => () => {}
const unavailableClock: MeetingClockPort = Object.freeze({ read: () => null, subscribe: noSubscribe })

/** A reviewed identity, or null while no current friend/self source has authority. */
export interface VenuePeopleDirectory extends ReadPort<LocationPeople | null> {
  /** Fed only by the authoritative Location snapshot this actor just read. */
  observe(snapshot: LocationSnapshot): void
}

export interface VenueSourcesOptions {
  readonly auth: SessionController
  readonly scope: PrivacyScope
  readonly signal: AbortSignal
  /** Siteplan identity is known before meeting contexts are mapped. */
  readonly identity: VenueView['identity']
  readonly community: CommunityBootstrapBinding | null
  /** Validated C03 participant projection; absent means self identity is unavailable. */
  readonly participant: unknown
  /** Deterministic transport seam; production borrows the canonical Auth request. */
  readonly http?: Pick<HttpClient, 'request'>
  /** Explicit reviewed friend/self identities. Defaults to the profile-backed directory. */
  readonly people?: ReadPort<LocationPeople | null>
  /** Explicit current viewer selection. Absent means no one can start sharing. */
  readonly sharingAudience?: ReadPort<LocationSharingAudience | null>
}

export interface VenueSources {
  readonly authority: VenueAuthority
  readonly clock: MeetingClockPort
  readonly http: Pick<HttpClient, 'request'>
  readonly location: ReturnType<typeof createLocationClient>
  readonly notes: ReturnType<typeof createNotesClient>
  readonly meeting: ReturnType<typeof createMeetingClient>
  readonly people: ReadPort<LocationPeople | null>
  readonly associations: ReadPort<LocationAssociations | null>
  readonly noteAssociations: ReadPort<ReviewedNoteAssociations | null>
  readonly creationRights: ReadPort<NoteCreationRights | null>
  readonly sharingAudience: ReadPort<LocationSharingAudience | null>
  readonly meetingPlaces: ReadonlyMap<LandmarkId, MeetingLandmarkContext>
  readonly labels: ReadonlyMap<string, string>
  /** Reads the frozen reviewed landmark contract; failure leaves every association empty. */
  loadLandmarks(signal: AbortSignal): Promise<boolean>
  dispose(): void
}

function authoredRequest(auth: SessionController): Pick<HttpClient, 'request'> {
  return Object.freeze({ request: <T>(input: HttpRequest<T>) => auth.request(input as Omit<HttpRequest<T>, 'refresh'>) })
}

function httpClient(http: Pick<HttpClient, 'request'>): HttpClient {
  return Object.freeze({ request: http.request, refreshSession: () => Promise.reject(new TransportError('invalid-request')) })
}

/** Fail-closed identity directory: no invented name, no retry of a rejected profile. */
export function createVenuePeopleDirectory(options: {
  readonly auth: SessionController; readonly scope: PrivacyScope; readonly signal: AbortSignal
  readonly venue: VenueView['identity']; readonly request: Pick<HttpClient, 'request'>
  readonly now: () => number | null; readonly participant: unknown
}): VenuePeopleDirectory {
  const { auth, scope, signal, request } = options
  const friends = new Map<string, Identity>()
  const asked = new Set<string>()
  let self: Identity | null = null
  let lease: Lease | null = null
  let generation = 0
  try {
    const participant = decodeParticipant(options.participant, scope)
    self = Object.freeze({ userId: participant.profile.user_id, displayName: participant.profile.display_name,
      avatar: participant.profile.avatar, canMessage: true, canMeet: true })
  } catch { /* No identity is fabricated when the participant projection is unavailable. */ }
  const current = () => {
    try { return !signal.aborted && auth.session.isCurrent(scope) && auth.privacy.isCurrent(scope) } catch { return false }
  }
  async function loadProfile(id: string, token: number) {
    try {
      const profile = await auth.privacy.run(scope, context => request.request({
        path: `/api/concert/events/${scope.eventId}/people/${id}`, method: 'GET', access: 'protected', errorFamily: 'core', signal: context.signal,
        decode: (value, response) => {
          if (response.status !== 200) throw new Error('Profile unavailable')
          return decodeProfile(value, id)
        },
      }), signal)
      if (!current() || token !== generation || profile.data.user_id !== id) return
      friends.set(id, Object.freeze({ userId: id, displayName: profile.data.display_name, avatar: profile.data.avatar,
        canMessage: true, canMeet: true }))
    } catch { /* Unknown names stay absent; a friend without an identity is not rendered. */ }
  }
  function observe(snapshot: LocationSnapshot) {
    const now = options.now()
    const until = Date.parse(snapshot.access_until)
    if (!current() || now === null || !Number.isFinite(until) || until <= now) return
    lease = Object.freeze({ scope: Object.freeze({ ...scope }), venue: options.venue, accessUntilMs: until })
    const token = ++generation
    for (const friend of snapshot.friends) {
      const id = friend.user_id
      if (id === scope.userId || !uuidPattern.test(id) || asked.has(id)) continue
      asked.add(id); void loadProfile(id, token)
    }
  }
  return Object.freeze({
    read() {
      if (!current() || !self || !lease || options.now() === null) return null
      return Object.freeze({ ...lease, self, friends: Object.freeze(Array.from(friends.values())) })
    },
    subscribe: noSubscribe,
    observe,
  })
}

/** Builds the frozen-contract clients and the reviewed local projections they consume. */
export function createVenueSources(options: VenueSourcesOptions): VenueSources {
  const { auth, scope, signal, identity, community } = options
  const http = options.http ?? authoredRequest(auth)
  const clock: MeetingClockPort = community?.read()?.clock ?? unavailableClock
  const authority: VenueAuthority = Object.freeze({
    scope, signal,
    isCurrent: () => {
      try { return !signal.aborted && scope.gate === 'open' && scope.userId !== null && scope.eventId !== null
        && auth.session.isCurrent(scope) && auth.privacy.isCurrent(scope) } catch { return false }
    },
    subscribe: (withdraw: () => void) => auth.privacy.registerCleanup(withdraw),
  })
  function now(): number | null {
    try {
      if (!authority.isCurrent()) return null
      const value = clock.read()
      return value && sameScope(value.scope, scope) && value.scope.gate === 'open' && Number.isFinite(value.nowMs) ? value.nowMs : null
    } catch { return null }
  }
  const directory = options.people ?? createVenuePeopleDirectory({ auth, scope, signal, venue: identity, request: http, now, participant: options.participant })
  const lease = { scope, signal, isCurrent: () => authority.isCurrent(), serverNow: () => now() ?? Number.NaN }
  const raw = createLocationClient(createHttpLocationTransport(httpClient(http), auth.session), lease)
  const observe = (directory as Partial<VenuePeopleDirectory>).observe
  const location = Object.freeze({ ...raw, async getSnapshot() {
    const result = await raw.getSnapshot()
    if (typeof observe === 'function') observe.call(directory, result.data)
    return result
  } })
  const notes = createNotesClient({ privacy: auth.privacy, transport: createHttpNotesTransport(http, auth.session) })
  const meeting = createMeetingClient(createHttpMeetingTransport(httpClient(http), auth.session), { scope, signal,
    isCurrent: () => authority.isCurrent(), now: () => now() ?? Number.NaN })
  const listeners = new Set<() => void>()
  let grants: readonly LandmarkGrant[] = [], leaseValue: Lease | null = null, disposed = false
  const notify = () => { for (const listener of Array.from(listeners)) { try { listener() } catch { /* Independent consumers. */ } } }
  const live = (): Lease | null => {
    const time = now()
    return leaseValue && time !== null && leaseValue.accessUntilMs > time && authority.isCurrent() ? leaseValue : null
  }
  function readAssociations(): LocationAssociations | null {
    const value = live(); if (!value) return null
    return Object.freeze({ ...value, items: Object.freeze(grants.flatMap(grant => grant.presentation_id === null ? []
      : [{ landmarkId: grant.presentation_id as LandmarkId, grant }])) })
  }
  function readNoteAssociations(): ReviewedNoteAssociations | null {
    const value = live(); if (!value) return null
    return Object.freeze({ ...value, bindings: Object.freeze(grants.flatMap(grant => grant.presentation_id === null || !grant.can_note ? []
      : [{ landmarkId: grant.presentation_id as LandmarkId, selection: Object.freeze({ ...grant.selection }) }])) })
  }
  function readCreationRights(): NoteCreationRights | null {
    const value = live(); if (!value) return null
    return Object.freeze({ ...value, rights: Object.freeze({ postNote: Object.freeze(grants.flatMap(grant => grant.presentation_id === null || !grant.can_note ? [] : [grant.presentation_id as LandmarkId])),
      noteAudiences: Object.freeze(['FRIENDS', 'EVERYONE'] as const), noteExpiries: Object.freeze(['ONE_HOUR', 'END_OF_DAY'] as const) }) })
  }
  const emptyAudience: ReadPort<LocationSharingAudience | null> = Object.freeze({ read: () => null, subscribe: noSubscribe })
  const sources: VenueSources = {
    authority, clock, http, location, notes, meeting,
    people: directory,
    sharingAudience: options.sharingAudience ?? emptyAudience,
    associations: { read: readAssociations, subscribe(listener) { if (disposed) return () => {}; listeners.add(listener); return () => { listeners.delete(listener) } } },
    noteAssociations: { read: readNoteAssociations, subscribe(listener) { if (disposed) return () => {}; listeners.add(listener); return () => { listeners.delete(listener) } } },
    creationRights: { read: readCreationRights, subscribe(listener) { if (disposed) return () => {}; listeners.add(listener); return () => { listeners.delete(listener) } } },
    get meetingPlaces() {
      const places = new Map<LandmarkId, MeetingLandmarkContext>()
      for (const grant of grants) {
        if (!grant.can_meet || grant.presentation_id === null) continue
        places.set(grant.presentation_id as LandmarkId, Object.freeze({ landmarkId: grant.selection.landmark_id,
          catalogRevision: String(grant.selection.catalog_revision), landmarkRevision: String(grant.selection.landmark_revision) }))
      }
      return places
    },
    get labels() {
      const labels = new Map<string, string>()
      for (const grant of grants) if (grant.presentation_id !== null) labels.set(grant.presentation_id, grant.label)
      return labels
    },
    async loadLandmarks(readSignal: AbortSignal): Promise<boolean> {
      if (disposed || readSignal.aborted || now() === null || !authority.isCurrent()) return false
      try {
        const result = await auth.privacy.run(scope, () => location.listLandmarks(), readSignal)
        const value = result.data, time = now()
        if (disposed || readSignal.aborted || time === null || !authority.isCurrent() || value.event_id !== scope.eventId) return false
        const server = Date.parse(value.server_time), until = Date.parse(value.access_until)
        if (!Number.isFinite(server) || !Number.isFinite(until) || server > time || until <= time) return false
        grants = Object.freeze(value.items.filter(grant => grant.presentation_id !== null
          && Date.parse(grant.valid_until) > time && Date.parse(grant.reviewed_at) <= Date.parse(grant.server_time)))
        leaseValue = Object.freeze({ scope: Object.freeze({ ...scope }), venue: identity, accessUntilMs: until })
        notify(); return true
      } catch { if (!disposed) { grants = []; leaseValue = null; notify() } return false }
    },
    dispose() {
      if (disposed) return
      disposed = true; grants = []; leaseValue = null; listeners.clear()
    },
  }
  return Object.freeze(sources)
}

export interface VenueProductionOptions {
  readonly venue: VenueView
  readonly sources: VenueSources
}

export interface VenueProduction {
  readonly capabilities: VenueCapabilities
  readonly phone: PhoneLocationPort
  readonly meeting: MeetingLeafRelease | null
  readonly meetingPlaces: ReadonlyMap<LandmarkId, MeetingLandmarkContext>
  /** Loads the authoritative snapshot; the caller keeps the shell release alive. */
  start(signal: AbortSignal): Promise<boolean>
  dispose(): void
}

/** One shell-owned production composition. Every port fails closed on missing authority. */
export function createVenueProduction({ venue, sources }: VenueProductionOptions): VenueProduction {
  const { authority, clock, people } = sources
  const location = createVenueLocationAdapter({ authority, venue, clock, client: sources.location,
    people, associations: sources.associations, sharingAudience: sources.sharingAudience, classification: 'AUTHORITATIVE_ADAPTER' })
  const notes = createVenueNotesAdapter({ authority, venue, clock, client: sources.notes, associations: sources.noteAssociations,
    creationRights: sources.creationRights, status: null, classification: 'AUTHORITATIVE_ADAPTER' })
  const capabilities = aggregateVenueCapabilities({ authority, venue, clock, location, notes })
  const phone = createBrowserLocation({ signal: authority.signal })
  const selectionFor = (intent: MeetingOpenIntent): MeetingSelection | null => {
    const association = intent.action === 'INVITE' ? intent : intent.landmark
    const base = { selectedFriendId: intent.targetUserId }
    if (!association) return base
    const catalogRevision = Number(association.catalogRevision), landmarkRevision = Number(association.landmarkRevision)
    if (!Number.isSafeInteger(catalogRevision) || catalogRevision < 1
      || !Number.isSafeInteger(landmarkRevision) || landmarkRevision < 1) return null
    return { ...base, landmark: { landmarkId: association.landmarkId, catalogRevision, landmarkRevision } }
  }
  function intentFor(selection: MeetingInvitationSelection): MeetingOpenIntent | null {
    const landmark = selection.landmark
    const place = venue.landmarks.find(item => item.meetingContext?.landmarkId === landmark.landmarkId
      && item.meetingContext.catalogRevision === String(landmark.catalogRevision)
      && item.meetingContext.landmarkRevision === String(landmark.landmarkRevision))
    if (!place?.meetingContext) return null
    return { scope: authority.scope, venue: venue.identity, targetUserId: selection.selectedFriendId,
      origin: 'notification', action: 'INVITE', ...place.meetingContext }
  }
  function presentationFor(intent: MeetingOpenIntent): MeetingPresentation | null {
    const directory = people.read()
    const friend = directory?.friends.find(person => person.userId === intent.targetUserId)
    if (!directory || !friend || directory.self.userId !== authority.scope.userId) return null
    const association = intent.action === 'INVITE' ? intent : intent.landmark
    const place = association ? venue.landmarks.find(item => item.meetingContext?.landmarkId === association.landmarkId
      && item.meetingContext.catalogRevision === association.catalogRevision
      && item.meetingContext.landmarkRevision === association.landmarkRevision) : undefined
    const label = association ? sources.labels.get(association.landmarkId) ?? null : null
    return { self: { userId: directory.self.userId, displayName: directory.self.displayName, portrait: null },
      friend: { userId: friend.userId, displayName: friend.displayName, portrait: null },
      landmarkLabel: place?.label ?? label, landmarkDescription: label }
  }
  const meetingAdapter = createNormalizedMeetingAdapter({ client: sources.meeting, scope: authority.scope, signal: authority.signal,
    isCurrent: current => authority.isCurrent() && sameScope(current, authority.scope), now: () => clock.read()?.nowMs ?? null,
    registerCleanup: withdraw => authority.subscribe(withdraw), selectionFor, intentFor, presentationFor })
  const meeting: MeetingLeafRelease = Object.freeze({
    scope: authority.scope, signal: authority.signal, clock, adapter: meetingAdapter.adapter,
    classification: 'AUTHORITATIVE_ADAPTER' as const,
    invalidations: { subscribe: (withdraw: () => void) => authority.subscribe(withdraw) },
    mapVenue: venue,
  })
  let disposed = false
  return Object.freeze({
    capabilities, phone, meeting,
    get meetingPlaces() { return sources.meetingPlaces },
    async start(signal: AbortSignal) {
      if (disposed || signal.aborted) return false
      try {
        const loaded = await location.load(signal)
        if (loaded) return true
        // A denied friend gate or offline producer still yields a truthful unavailable
        // panel. A missing clock/access (access-unavailable) keeps the shell closed.
        const status = location.availability()
        return status.state === 'unavailable'
          && (status.reason === 'friend-gate-unavailable' || status.reason === 'offline' || status.reason === 'producer-refused')
      } catch { return false }
    },
    dispose() {
      if (disposed) return
      disposed = true
      for (const stop of [() => meetingAdapter.dispose(), () => notes.dispose(), () => location.dispose(), () => phone.dispose(), () => sources.dispose()]) {
        try { stop() } catch { /* Release the remaining independent leaf. */ }
      }
    },
  })
}
