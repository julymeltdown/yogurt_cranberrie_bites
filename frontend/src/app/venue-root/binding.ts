import type { PrivacyScope } from '../../shared/query/privacy'
import type { MeetingConsumerPort, VenueMapFocusPort } from '../../features/paradise-city-map/ports'
import { sameSiteplanIdentity } from '../../features/paradise-city-map/spatial/contract'
import type { VenueConsumerInput } from '../venue-entry/consumer'
import type { MeetingLeafRelease } from '../venue-community-entry'
import type { VenueRootInput } from './input'

export const sameVenueRootScope = (a: PrivacyScope, b: PrivacyScope) => a.userId === b.userId && a.eventId === b.eventId
  && a.generation === b.generation && a.policyGeneration === b.policyGeneration && a.gate === b.gate

/** One shell-owned bridge. Proxies retain identity while the actual leaves bind.
 * It never owns/disposes the injected ports, image, Auth, clock or producers. */
export function createVenueRootBinding(input: VenueRootInput) {
  let active = true, focus: VenueMapFocusPort | null = null, meeting: MeetingConsumerPort | null = null
  let stopMeeting: (() => void) | null = null, focusSucceeded = false, meetingSerial = 0
  const stops: (() => void)[] = [], listeners = new Set<() => void>(), meetingListeners = new Set<() => void>()
  const notify = (set: Set<() => void>) => { for (const listener of Array.from(set)) { try { listener() } catch { /* Notify all borrowers. */ } } }
  function isCurrent() {
    try {
      const image = input.siteplan.image.source
      return active && !input.signal.aborted && input.scope.gate === 'open'
        && input.scope.userId !== null && input.scope.eventId !== null
        && input.auth.session.isCurrent(input.scope) && input.auth.privacy.isCurrent(input.scope)
        && typeof HTMLImageElement !== 'undefined' && image instanceof HTMLImageElement && image.src === input.venue.image.src
    } catch { return false }
  }
  function meetingCurrent() {
    try {
      const release = input.meeting, community = input.community
      if (!isCurrent() || !release || !community || !community.isCurrent() || community.signal !== input.signal
        || !community.scope || !sameVenueRootScope(community.scope, input.scope)
        || release.signal !== input.signal || !sameVenueRootScope(release.scope, input.scope)) return false
      const clock = community.read()?.clock, value = clock?.read()
      return clock === release.clock && !!value && sameVenueRootScope(value.scope, input.scope) && Number.isFinite(value.nowMs)
    } catch { return false }
  }
  const focusProxy: VenueMapFocusPort = Object.freeze<VenueMapFocusPort>({ focus(intent) {
    focusSucceeded = false
    if (!isCurrent() || !sameVenueRootScope(intent.scope, input.scope) || !sameSiteplanIdentity(intent.venue, input.venue.identity)
      || !input.venue.landmarks.some(place => place.id === intent.landmarkId)) return false
    const target = focus
    try { focusSucceeded = !!target?.focus(intent) && focus === target && isCurrent(); return focusSucceeded }
    catch { return false }
  } })
  const onMapFocused = () => {
    const permitted = focusSucceeded; focusSucceeded = false
    if (!permitted || !isCurrent()) return
    try { input.navigateToMap() } catch { /* Root navigation failure grants no new permission. */ }
  }
  const meetingProxy: MeetingConsumerPort = Object.freeze<MeetingConsumerPort>({
    read() { try { return meetingCurrent() ? meeting?.read() ?? null : null } catch { return null } },
    subscribe(listener) { if (!active) return () => {}; meetingListeners.add(listener); return () => { meetingListeners.delete(listener) } },
    async open(intent) {
      if (!meetingCurrent() || !sameVenueRootScope(intent.scope, input.scope) || !sameSiteplanIdentity(intent.venue, input.venue.identity)) return { kind: 'unavailable' }
      const target = meeting, serial = meetingSerial
      try {
        const result = await target?.open(intent)
        return meetingCurrent() && target === meeting && serial === meetingSerial && result ? result : { kind: 'stale' }
      } catch { return { kind: 'unavailable' } }
    },
    close() { try { return meetingCurrent() ? meeting?.close() ?? true : true } catch { return false } },
  })
  const bindFocus = (next: VenueMapFocusPort | null) => { focus = isCurrent() ? next : null; focusSucceeded = false }
  function bindConsumer(next: MeetingConsumerPort | null) {
    const serial = ++meetingSerial, stop = stopMeeting
    // Clear references BEFORE an old unsubscribe/owner callback can re-enter.
    meeting = null; stopMeeting = null
    try { stop?.() } catch { /* Old cleanup cannot keep its delegate current. */ }
    if (serial !== meetingSerial) return
    if (next && meetingCurrent()) {
      meeting = next
      try {
        const cleanup = next.subscribe(() => { if (serial === meetingSerial && meetingCurrent()) notify(meetingListeners) })
        if (serial === meetingSerial && meeting === next && meetingCurrent()) stopMeeting = cleanup
        else cleanup()
      } catch { if (serial === meetingSerial && meeting === next) meeting = null }
    }
    notify(meetingListeners)
  }
  const release: MeetingLeafRelease | null = meetingCurrent() ? Object.freeze({ ...input.meeting!,
    mapFocus: focusProxy, mapVenue: input.venue, onMapFocused }) : null
  const mapInput: VenueConsumerInput = Object.freeze({ auth: input.auth, scope: input.scope, signal: input.signal,
    community: input.community, venue: input.venue, ...(input.capabilities ? { capabilities: input.capabilities } : {}),
    meeting: meetingProxy, ...(input.phone ? { phone: input.phone } : {}) })
  function dispose() {
    if (!active) return
    active = false; focusSucceeded = false; ++meetingSerial
    const stop = stopMeeting; focus = null; meeting = null; stopMeeting = null
    const cleanup = [stop, ...stops.splice(0).reverse()]
    for (const end of cleanup) { try { end?.() } catch { /* Complete all cleanup after scrubbing. */ } }
    notify(meetingListeners); notify(listeners)
    meetingListeners.clear(); listeners.clear()
  }
  function register(make: () => () => void) {
    if (!active) return
    try { const stop = make(); if (active) stops.push(stop); else stop() } catch { dispose() }
  }
  if (!isCurrent()) dispose()
  else {
    input.signal.addEventListener('abort', dispose, { once: true })
    stops.push(() => input.signal.removeEventListener('abort', dispose))
    register(() => input.auth.privacy.registerCleanup(dispose))
    if (!isCurrent()) dispose()
  }
  return Object.freeze({ input, mapInput, release, focus: focusProxy, meeting: meetingProxy, bindFocus, bindConsumer, onMapFocused,
    read: isCurrent, subscribe(listener: () => void) { if (!active) return () => {}; listeners.add(listener); return () => { listeners.delete(listener) } }, dispose })
}
export type VenueRootBinding = ReturnType<typeof createVenueRootBinding>
