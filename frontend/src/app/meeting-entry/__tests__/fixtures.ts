import { harness, beaconId } from '../../../features/meet-beacon/__tests__/fixtures/harness'
import { createMeetingEntry, type MeetingEntry } from '../bridge'
import type { MeetingEntryRelease, MeetingInvalidation } from '../ports'
import type { PrivacyScope } from '../../../shared/query/privacy'

export { beaconId }
export async function entryFixture() {
  const h = await harness(); h.controller.dispose() // Reuse canonical Auth/FE19 fixture owners, not a second active beacon owner.
  const clockListeners = new Set<() => void>(), authorityListeners = new Set<() => void>(), invalidations = new Set<(value: MeetingInvalidation) => void>()
  const lifetime = new AbortController(), scope = h.auth.privacy.capture(), entries: MeetingEntry[] = []
  let clockScope: PrivacyScope = scope, clockAvailable = true, placeListeners = 0
  const release: MeetingEntryRelease = {
    scope, signal: lifetime.signal,
    places: { read: h.placesController.read, subscribe(listener) { placeListeners++; const stop = h.placesController.subscribe(listener); return () => { placeListeners--; stop() } } },
    authority: { read: () => h.authority, subscribe(listener) { authorityListeners.add(listener); return () => { authorityListeners.delete(listener) } } },
    clock: { read: () => clockAvailable && h.time !== null ? { scope: clockScope, nowMs: h.time } : null,
      subscribe(listener) { clockListeners.add(listener); return () => { clockListeners.delete(listener) } } },
    invalidations: { subscribe(listener) { invalidations.add(listener); return () => { invalidations.delete(listener) } } },
  }
  const notifyClock = () => { for (const fn of Array.from(clockListeners)) fn() }
  return { h, scope, release, lifetime,
    create(value: MeetingEntryRelease | null = release) { const entry = createMeetingEntry({ auth: h.auth, release: value }); entries.push(entry); return entry },
    notifyAuthority() { for (const fn of Array.from(authorityListeners)) fn() },
    setClock(available: boolean) { clockAvailable = available; notifyClock() },
    setClockScope(value: PrivacyScope) { clockScope = value; notifyClock() },
    setTime(value: number | null) { h.advanceClock(value); notifyClock() },
    invalidate(value: MeetingInvalidation) { for (const fn of Array.from(invalidations)) fn(value) },
    subscriptions: () => ({ clock: clockListeners.size, authority: authorityListeners.size, places: placeListeners, invalidations: invalidations.size }),
    dispose() { entries.forEach(entry => entry.dispose()); h.dispose() },
  }
}
