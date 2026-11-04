import { createGiftDiscoveryController } from '../../features/gift-discovery'
import { createPartyController } from '../../features/party-discovery'
import { createSessionIntentJournal, type IntentJournal } from '../../features/gift-pickup'
import type { SessionController } from '../../features/auth/model/session'
import type { PrivacyScope } from '../../shared/query/privacy'
import type { ShellBootstrap } from '../ports'
import { CAPABILITIES, projectCapabilities } from '../capabilities'
import { sameCommunityScope } from '../community-entry/bridge'
import type { CommunityEntryRelease } from '../community-entry'
import type { CommunityBootstrapBinding } from './bootstrap'

/** Owns the existing discovery pair. The BootstrapBoundary alone owns timing. */
export function createCommunityOwner({ auth, bootstrap, scope, binding }: {
  readonly auth: SessionController; readonly bootstrap: ShellBootstrap; readonly scope: PrivacyScope
  readonly binding: CommunityBootstrapBinding | null
}) {
  let disposed = false, retired = false
  const listeners = new Set<() => void>(), stops: (() => void)[] = []
  const matches = !!binding && binding.isCurrent() && !!binding.scope && sameCommunityScope(binding.scope, scope)
    && binding.source.document === bootstrap.document && binding.source.event.eventId === bootstrap.event.eventId
    && binding.source.participant.userId === bootstrap.participant.userId
    && CAPABILITIES.every(key => binding.source.capabilities[key].available === bootstrap.capabilities[key].available
      && binding.source.capabilities[key].reason === bootstrap.capabilities[key].reason)
  const authority = matches ? binding!.read() : null
  const live = () => !disposed && !retired && auth.privacy.isCurrent(scope) && (!binding || matches && binding.isCurrent())
  const now = () => live() ? authority?.clock.read()?.nowMs ?? Number.NaN : Number.NaN
  const readOnlyCapabilities = projectCapabilities({ ...bootstrap.capabilities, mutations: { available: false, reason: 'DEPENDENCY_UNAVAILABLE' } })
  const capabilities = () => Number.isFinite(now()) ? bootstrap.capabilities : readOnlyCapabilities
  const subscribe = (listener: () => void) => { listeners.add(listener); return () => { listeners.delete(listener) } }
  const access = { read: () => live() ? { scope, capabilities: capabilities(), phase: bootstrap.event.phase } : null, subscribe }
  const context = { read: () => live() && Number.isFinite(now()) ? { scope, capabilities: capabilities(), eventPhase: bootstrap.event.phase } : null, subscribe }
  const parties = createPartyController({ auth, access, now })
  const gifts = createGiftDiscoveryController({ auth, context, clock: { now,
    setTimeout: (callback, delay) => globalThis.setTimeout(callback, delay),
    clearTimeout: handle => globalThis.clearTimeout(handle as ReturnType<typeof setTimeout>),
  } })
  function retire() {
    if (retired || disposed) return
    retired = true
    for (const fn of Array.from(listeners)) { try { fn() } catch { /* Every borrowed consumer withdraws. */ } }
  }
  let journal: IntentJournal | undefined
  try { if (typeof sessionStorage !== 'undefined') journal = createSessionIntentJournal(sessionStorage) } catch { /* Missing recovery storage disables pickup commands. */ }
  const release: CommunityEntryRelease | null = authority && matches && Number.isFinite(now()) ? Object.freeze({
    scope, signal: binding!.signal, access, context, parties, gifts, clock: authority.clock, places: authority.places,
    ...(journal ? { journal } : {}),
  }) : null
  try {
    stops.push(auth.privacy.registerCleanup(retire))
    if (binding) { binding.signal.addEventListener('abort', retire, { once: true }); stops.push(() => binding.signal.removeEventListener('abort', retire)) }
    if (authority) stops.push(authority.clock.subscribe(() => { if (!authority.clock.read()) retire() }))
    if (!live()) retire()
  } catch { retire() }
  return Object.freeze({ parties, gifts, release,
    dispose() {
      if (disposed) return
      retire(); disposed = true
      for (const stop of stops.splice(0).reverse()) { try { stop() } catch { /* Continue releasing owners. */ } }
      parties.dispose(); gifts.dispose(); listeners.clear()
      // StrictMode may replay this owner; it must not dispose the Boundary's original request timing.
    },
  })
}
