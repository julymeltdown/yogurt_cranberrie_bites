import { LocationUnavailableError, type createLocationClient, type LandmarkGrant, type LandmarkSelection, type LocationCommandReceipt, type LocationRequest, type LocationSnapshot, type LocationWriteOperation, type VisibleLocation } from '../../../shared/api/paradise-city-v1/location'
import { TransportError } from '../../../shared/api/errors'
import type { MeetingClockPort } from '../../../app/meeting-entry/ports'
import type { CapabilitySnapshot, CommandContext, CommandResult, PersonView, ReadPort, VenueAuthority, VenueAvailability, VenueCapabilities, VenueUnavailableReason, VenueView } from '../ports'
import type { LandmarkId } from '../spatial/contract'

type Identity = Pick<PersonView, 'userId' | 'displayName' | 'avatar' | 'canMessage' | 'canMeet'>
interface Lease { readonly scope: VenueAuthority['scope']; readonly venue: VenueView['identity']; readonly accessUntilMs: number }
export interface LocationPeople extends Lease { readonly self: Identity; readonly friends: readonly Identity[] }
/** Supplied by the reviewed provider owner. A presentation ID alone grants nothing. */
export interface LocationAssociations extends Lease { readonly items: readonly { readonly landmarkId: LandmarkId; readonly grant: LandmarkGrant }[] }
/** Explicit viewer selection/consent, never inferred from the current friend list. */
export interface LocationSharingAudience extends Lease { readonly viewerIds: readonly string[] }
export interface VenueLocationAdapterOptions {
  readonly authority: VenueAuthority; readonly venue: VenueView; readonly clock: MeetingClockPort
  readonly client: ReturnType<typeof createLocationClient> | null
  readonly people: ReadPort<LocationPeople | null>
  readonly associations: ReadPort<LocationAssociations | null>
  readonly sharingAudience: ReadPort<LocationSharingAudience | null>
  readonly classification: CapabilitySnapshot['classification']
}
export type LocationRequestView = Pick<LocationRequest, 'request_id' | 'requester_id' | 'subject_id' | 'revision' | 'state' | 'expires_at'>
const sameScope = (a: VenueAuthority['scope'], b: VenueAuthority['scope']) => a.userId === b.userId && a.eventId === b.eventId && a.generation === b.generation && a.policyGeneration === b.policyGeneration && a.gate === b.gate
const sameVenue = (a: VenueView['identity'], b: VenueView['identity']) => a.venueId === b.venueId && a.siteplanId === b.siteplanId && a.revision === b.revision && a.variantId === b.variantId
const ms = (value: string | null) => value === null ? null : Date.parse(value)
const rejected = (): CommandResult => ({ state: 'rejected' })
const unknown = (): CommandResult => ({ state: 'unknown' })
const sameSelection = (a: LandmarkSelection, b: LandmarkSelection) => a.landmark_id === b.landmark_id && a.catalog_revision === b.catalog_revision && a.landmark_revision === b.landmark_revision

/** Injected wire client only. Construction/read/subscribe never dispatch network work. */
export function createVenueLocationAdapter(options: VenueLocationAdapterOptions) {
  const { authority, venue, clock, client, people, associations, sharingAudience } = options
  const listeners = new Set<() => void>(), stops: (() => void)[] = []
  type Binding = { operation: LocationWriteOperation; intent: string; result: CommandResult; pending: boolean; expiresAt: number }
  const commands = new Map<string, Binding>()
  let active = true, raw: LocationSnapshot | null = null, loadVersion = 0, revision = 0
  let lastReadAtMs: number | null = null, lastFailure: VenueUnavailableReason | null = null, refreshing = false
  let loading: Promise<boolean> | null = null
  function time() {
    if (!active || authority.signal.aborted || !authority.isCurrent()) return null
    const value = clock.read()
    return value && sameScope(value.scope, authority.scope) && value.scope.gate === 'open' && Number.isFinite(value.nowMs) ? value.nowMs : null
  }
  function lease<T extends Lease>(value: T | null, now: number): T | null {
    return value && sameScope(value.scope, authority.scope) && sameVenue(value.venue, venue.identity) && Number.isFinite(value.accessUntilMs) && value.accessUntilMs > now ? value : null
  }
  function currentRaw(now: number) {
    const value = raw
    return value && value.event_id === authority.scope.eventId && value.actor_id === authority.scope.userId
      && ms(value.access_until)! > now && ms(value.policy.valid_until)! > now && ms(value.clock.valid_until)! > now
      && value.policy.venue_key === value.clock.venue_key && value.policy.revision === value.clock.policy_revision ? value : null
  }
  function notify() { revision++; for (const listener of Array.from(listeners)) listener() }
  function changed() {
    if (time() === null) { raw = null; loadVersion++; commands.clear(); lastReadAtMs = null; lastFailure = 'access-unavailable' }
    notify()
  }
  /** Distinguishes a denied friendship gate (503) from transport loss without guessing. */
  function classify(error: unknown): { reason: VenueUnavailableReason; clear: boolean } | null {
    if (error instanceof LocationUnavailableError) return { reason: 'friend-gate-unavailable', clear: true }
    if (error instanceof TransportError) {
      if (error.kind === 'aborted' || error.kind === 'session-changed' || error.kind === 'authentication-required') return { reason: 'access-unavailable', clear: true }
      if (error.kind === 'network' || error.kind === 'timeout') return { reason: 'offline', clear: false }
      if (error.status === 503) return { reason: 'friend-gate-unavailable', clear: true }
      return { reason: 'producer-refused', clear: true }
    }
    return { reason: 'producer-refused', clear: true }
  }
  function availability(): VenueAvailability {
    const now = time(), value = now === null ? null : currentRaw(now)
    const age = now === null || lastReadAtMs === null ? null : Math.max(0, now - lastReadAtMs)
    if (now === null) return { state: 'unavailable', reason: 'access-unavailable', readAtMs: lastReadAtMs, ageMs: age, staleAtMs: null, refreshing, canRefresh: false }
    if (!value) return { state: 'unavailable',
      reason: raw ? 'access-unavailable' : lastFailure ?? (lastReadAtMs === null ? 'producer-refused' : 'offline'),
      readAtMs: lastReadAtMs, ageMs: age, staleAtMs: null, refreshing, canRefresh: !!client }
    const staleAtMs = lastReadAtMs === null ? null : lastReadAtMs + value.policy.fresh_ms
    const stale = lastFailure !== null || staleAtMs !== null && now >= staleAtMs
    return { state: stale ? 'stale' : 'ready', reason: lastFailure, readAtMs: lastReadAtMs, ageMs: age, staleAtMs, refreshing, canRefresh: !!client }
  }
  function roster(now: number) {
    const value = lease(people.read(), now)
    if (!value || value.self.userId !== authority.scope.userId || new Set(value.friends.map(p => p.userId)).size !== value.friends.length
      || value.friends.some(p => p.userId === value.self.userId)) return null
    return value
  }
  function reviewed(now: number, value: LocationSnapshot) {
    const binding = lease(associations.read(), now)
    if (!binding || new Set(binding.items.map(p => p.landmarkId)).size !== binding.items.length
      || new Set(binding.items.map(p => p.grant.selection.landmark_id)).size !== binding.items.length) return []
    return binding.items.filter(({ landmarkId, grant }) => venue.landmarks.some(p => p.id === landmarkId)
      && grant.event_id === authority.scope.eventId && grant.provider_id === value.policy.provider_id
      && grant.source_revision === value.policy.source_revision && grant.venue_key === value.policy.venue_key
      && grant.policy_revision === value.policy.revision && grant.review_reference.trim().length > 0
      && ms(grant.reviewed_at)! <= now && ms(grant.server_time)! <= now && ms(grant.valid_until)! > now)
  }
  function person(identity: Identity, location: VisibleLocation | null, now: number, value: LocationSnapshot | null): PersonView {
    const visible = !!location && location.user_id === identity.userId && ms(location.access_until)! > now
    const observation = visible ? ms(location.observed_at) : null
    // The frozen wire supplies fresh_until only for PHONE. Check-in and LAST_KNOWN
    // are current no longer than expires_at; UNAVAILABLE has no current horizon.
    const horizon = !location ? null : location.source === 'PHONE' ? location.fresh_until
      : location.source === 'SELF_REPORTED_CHECK_IN' || location.source === 'LAST_KNOWN' ? location.expires_at : null
    const fresh = visible && horizon !== null ? Math.min(ms(horizon) ?? -Infinity, ms(location.expires_at) ?? -Infinity, ms(location.access_until) ?? -Infinity) : null
    const mapped = visible && value && location.source === 'SELF_REPORTED_CHECK_IN' && location.landmark
      ? reviewed(now, value).find(p => sameSelection(p.grant.selection, location.landmark!.selection)
        && p.grant.provider_id === location.landmark!.provider_id && p.grant.source_revision === location.landmark!.source_revision
        && p.grant.venue_key === location.landmark!.venue_key && p.grant.map_id === location.landmark!.map_id
        && p.grant.map_revision === location.landmark!.map_revision && p.grant.policy_revision === location.landmark!.policy_revision
        && p.grant.review_reference === location.landmark!.review_reference && ms(location.landmark!.valid_until)! > now) : undefined
    const current = observation !== null && Number.isFinite(observation) && observation <= now && fresh !== null && Number.isFinite(fresh) && fresh > now
    const place = mapped && current ? venue.landmarks.find(p => p.id === mapped.landmarkId) : undefined
    return { ...identity, visible: true, point: place?.point ?? null, landmarkId: place?.id ?? null,
      locationSource: !visible || location.source === 'UNAVAILABLE' || observation === null ? 'unavailable'
        : location.source === 'SELF_REPORTED_CHECK_IN' && !current ? 'unavailable'
          : !current || location.source === 'LAST_KNOWN' ? 'last-known' : location.source === 'PHONE' ? 'phone' : 'check-in',
      updatedAtMs: observation, freshUntilMs: fresh !== null && Number.isFinite(fresh) ? fresh : null,
      canRequest: identity.userId !== authority.scope.userId && visible && location.can_request_location }
  }
  function read(): CapabilitySnapshot | null {
    // A failed owner read is never reconstructed as a friend roster.
    if (!raw && lastFailure !== null) return null
    const stamp = revision
    const now = time(); if (now === null) return null
    const currentPeople = roster(now); if (!currentPeople) return null
    const value = currentRaw(now), rights = value ? reviewed(now, value) : []
    const accessUntilMs = Math.min(currentPeople.accessUntilMs, value ? ms(value.access_until)! : Infinity,
      value ? ms(value.policy.valid_until)! : Infinity, value ? ms(value.clock.valid_until)! : Infinity)
    const snapshot: CapabilitySnapshot = { scope: authority.scope, venue: venue.identity, classification: options.classification,
      serverNowMs: now, accessUntilMs, self: person(currentPeople.self, value?.self_location ?? null, now, value),
      friends: currentPeople.friends.map(p => person(p, value?.friends.find(f => f.user_id === p.userId) ?? null, now, value)),
      notes: [], noteClusters: [], notesVisibleCount: null, locationRequests: { incoming: requests().incoming.map(requestView), outgoing: requests().outgoing.map(requestView) },
      sharing: value?.sharing.state === 'ACTIVE' && ms(value.sharing.expires_at)! > now ? 'active'
        : value?.sharing.state === 'PAUSED' ? 'paused' : value?.sharing.state === 'REVOKED' ? 'revoked' : 'off',
      rights: { sharePhone: !!value?.capabilities.can_share_phone, pause: !!value?.capabilities.can_pause, revoke: !!value?.capabilities.can_revoke,
        checkIn: rights.filter(p => p.grant.can_check_in && value!.capabilities.check_in_landmark_ids.includes(p.grant.selection.landmark_id)).map(p => p.landmarkId),
        postNote: [], noteAudiences: [], noteExpiries: [] } }
    return time() !== null && revision === stamp ? snapshot : null
  }
  function valid(context: CommandContext) { return !context.signal.aborted && sameScope(context.scope, authority.scope) && sameVenue(context.venue, venue.identity) && time() !== null }
  async function load(signal: AbortSignal): Promise<boolean> {
    if (loading) return loading
    const version = ++loadVersion
    if (!client || signal.aborted || time() === null) {
      if (!client) lastFailure = 'producer-refused'
      else if (time() === null) lastFailure = 'access-unavailable'
      notify(); return false
    }
    // Keep the last authoritative read visible only as stale while the re-read is in flight.
    refreshing = true; notify()
    loading = (async () => {
      try {
        const response = await client.getSnapshot()
        if (version !== loadVersion || signal.aborted || time() === null) return false
        const now = time()!
        raw = response.data
        if (!currentRaw(now)) { raw = null; lastFailure = 'access-unavailable'; lastReadAtMs = null; notify(); return false }
        lastReadAtMs = now; lastFailure = null
        notify(); return true
      } catch (error) {
        if (version !== loadVersion) return false
        const failure = classify(error)
        if (!failure || failure.reason === 'access-unavailable' && error instanceof TransportError && error.kind === 'aborted') return false
        if (failure.clear) { raw = null; lastReadAtMs = null }
        lastFailure = failure.reason
        notify(); return false
      } finally {
        if (version === loadVersion) { refreshing = false; notify() }
        loading = null
      }
    })()
    return loading
  }
  async function write(operation: LocationWriteOperation, context: CommandContext, intent: string, dispatch: () => Promise<{ data: LocationCommandReceipt }>): Promise<CommandResult> {
    const now = time()
    if (!client || now === null || !valid(context) || !currentRaw(now) || !roster(now)) return rejected()
    const previous = commands.get(context.key)
    if (previous) return previous.operation === operation && previous.intent === intent ? previous.pending ? unknown() : previous.result : rejected()
    for (const [key, binding] of commands) if (!binding.pending && binding.result.state !== 'unknown' && binding.expiresAt <= now) commands.delete(key)
    if (commands.size >= 1024) return rejected()
    const binding: Binding = { operation, intent, result: unknown(), pending: true, expiresAt: now + currentRaw(now)!.policy.command_retention_ms }
    commands.set(context.key, binding)
    try {
      const response = await dispatch()
      if (!valid(context)) return binding.result
      if (response.data.command_key !== context.key || response.data.operation !== operation) return binding.result
      binding.result = response.data.state === 'SUCCEEDED' ? { state: 'applied' } : rejected()
      await load(context.signal)
    } catch (error) {
      const definite = error instanceof TransportError && !error.uncertain && (error.kind === 'invalid-request' || error.kind === 'authentication-required'
        || error.kind === 'http' && error.status !== null && error.status >= 400 && error.status < 500 && error.status !== 408)
      binding.result = definite ? rejected() : unknown()
    } finally { binding.pending = false }
    return binding.result
  }
  function requests() {
    const now = time(), currentPeople = now === null ? null : roster(now), value = now === null ? null : currentRaw(now)
    if (!currentPeople || !value || now === null) return { incoming: [], outgoing: [] }
    const eligible = (request: LocationRequest) => request.event_id === authority.scope.eventId && request.state === 'PENDING' && ms(request.expires_at)! > now
      && currentPeople.friends.some(p => p.userId === (request.subject_id === authority.scope.userId ? request.requester_id : request.subject_id))
    return { incoming: value.incoming_requests.filter(r => eligible(r) && r.subject_id === authority.scope.userId),
      outgoing: value.outgoing_requests.filter(r => eligible(r) && r.requester_id === authority.scope.userId) }
  }
  function requestView(request: LocationRequest) { return { id: request.request_id, requesterId: request.requester_id, subjectId: request.subject_id, revision: request.revision, expiresAtMs: ms(request.expires_at)! } }
  const api = {
    read, subscribe(listener: () => void) { listeners.add(listener); return () => { listeners.delete(listener) } }, load,
    ownsCommand: (key: string) => time() !== null && commands.has(key),
    async beginPhoneSharing(context: CommandContext) {
      const now = time(), currentPeople = now === null ? null : roster(now), value = now === null ? null : currentRaw(now)
      const audience = now === null ? null : lease(sharingAudience.read(), now)
      if (!value?.capabilities.can_share_phone || !currentPeople || !audience || new Set(audience.viewerIds).size !== audience.viewerIds.length
        || audience.viewerIds.length > value.policy.max_viewers || audience.viewerIds.some(id => !currentPeople.friends.some(p => p.userId === id))) return rejected()
      return write('setVenueLocationSharing', context, JSON.stringify(['ACTIVE', audience.viewerIds]), () => client!.setSharing({ expected_revision: value.sharing.revision, state: 'ACTIVE', viewer_ids: [...audience.viewerIds] }, context.key))
    },
    async publishPhoneFix(fix: Parameters<VenueCapabilities['publishPhoneFix']>[0], context: CommandContext) {
      const now = time(), value = now === null ? null : currentRaw(now)
      if (!value || read()?.sharing !== 'active' || !value.capabilities.can_share_phone || now === null || !Number.isFinite(fix.latitude) || fix.latitude < -90 || fix.latitude > 90
        || !Number.isFinite(fix.longitude) || fix.longitude < -180 || fix.longitude > 180 || !Number.isFinite(fix.accuracy) || fix.accuracy < 0 || fix.accuracy > value.policy.max_accuracy_m
        || !Number.isFinite(fix.capturedAtMs) || fix.capturedAtMs > now + value.policy.future_skew_ms || fix.capturedAtMs < now - value.policy.fix_max_age_ms) return rejected()
      return write('publishVenueLocationFix', context, JSON.stringify(fix), () => client!.publishFix({ expected_sharing_revision: value.sharing.revision, source: 'DEVICE_GEOLOCATION_REPORTED', latitude: fix.latitude, longitude: fix.longitude, accuracy_m: fix.accuracy, captured_at: new Date(fix.capturedAtMs).toISOString() }, context.key))
    },
    async pause(_reason: 'user' | 'background' | 'withdrawal', context: CommandContext) {
      const now = time(), value = now === null ? null : currentRaw(now)
      if (!value?.capabilities.can_pause) return rejected()
      // L9: PAUSED/REVOKED require an empty audience and remove all current grants.
      return write('setVenueLocationSharing', context, 'PAUSED', () => client!.setSharing({ expected_revision: value.sharing.revision, state: 'PAUSED', viewer_ids: [] }, context.key))
    },
    async revoke(context: CommandContext) {
      const now = time(), value = now === null ? null : currentRaw(now)
      if (!value?.capabilities.can_revoke) return rejected()
      return write('setVenueLocationSharing', context, 'REVOKED', () => client!.setSharing({ expected_revision: value.sharing.revision, state: 'REVOKED', viewer_ids: [] }, context.key))
    },
    async requestLocation(userId: string, context: CommandContext) {
      if (!read()?.friends.some(p => p.userId === userId && p.canRequest) || requests().outgoing.some(r => r.subject_id === userId)) return rejected()
      return write('requestVenueLocation', context, userId, () => client!.requestLocation({ subject_id: userId }, context.key))
    },
    async checkIn(landmarkId: LandmarkId, context: CommandContext) {
      const now = time(), value = now === null ? null : currentRaw(now)
      const binding = value && now !== null ? reviewed(now, value).find(p => p.landmarkId === landmarkId) : null
      if (!value || !binding || !read()?.rights.checkIn.includes(landmarkId)) return rejected()
      return write('setVenueLocationCheckIn', context, JSON.stringify(binding.grant.selection), () => client!.checkIn({ expected_sharing_revision: value.sharing.revision, landmark: binding.grant.selection }, context.key))
    },
    requests,
    async decideRequest(id: string, decision: 'APPROVE' | 'DENY' | 'CANCEL', context: CommandContext) {
      const list = requests(), request = (decision === 'CANCEL' ? list.outgoing : list.incoming).find(r => r.request_id === id)
      if (!request) return rejected()
      return write('decideVenueLocationRequest', context, JSON.stringify([id, request.revision, decision]), () => client!.decideRequest(id, { expected_revision: request.revision, decision }, context.key))
    },
    async resolve(key: string, context: CommandContext): Promise<CommandResult> {
      const binding = commands.get(key)
      if (!client || !binding || context.key !== key || !valid(context)) return rejected()
      if (binding.pending || binding.result.state !== 'unknown') return binding.pending ? unknown() : binding.result
      try {
        const response = await client.getCommand(binding.operation, key)
        if (!valid(context) || response.data.command_key !== key || response.data.operation !== binding.operation) return unknown()
        binding.result = response.data.state === 'SUCCEEDED' ? { state: 'applied' } : rejected()
        await load(context.signal); return binding.result
      } catch { return unknown() }
    },
    availability, refresh() { return load(authority.signal) },
    dispose() { if (!active) return; active = false; loadVersion++; raw = null; lastReadAtMs = null; lastFailure = 'access-unavailable'; commands.clear(); for (const stop of stops.splice(0)) stop(); authority.signal.removeEventListener('abort', changed); notify(); listeners.clear() },
  }
  stops.push(authority.subscribe(changed), clock.subscribe(changed), people.subscribe(() => { raw = null; loadVersion++; changed() }), associations.subscribe(changed), sharingAudience.subscribe(changed))
  authority.signal.addEventListener('abort', changed, { once: true })
  return api
}
