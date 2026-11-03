import { createPartyAuthoring, type PartyAuthoringController } from '../../features/party-authoring'
import { createPartyManagement, type PartyManagementController } from '../../features/party-management'
import { createGiveawayAuthoring, type GiveawayAuthoringController } from '../../features/giveaway-authoring'
import { createGiftPickup, type GiftPickupController } from '../../features/gift-pickup'
import { validCapabilities } from '../capabilities'
import { canonicalId } from '../routes'
import type { PrivacyScope } from '../../shared/query/privacy'
import type { CommunityEntryAuth, CommunityEntryRelease } from './ports'

export const sameCommunityScope = (a: PrivacyScope, b: PrivacyScope) => a.userId === b.userId && a.eventId === b.eventId
  && a.generation === b.generation && a.policyGeneration === b.policyGeneration && a.gate === b.gate
export interface CommunityEntryView { readonly phase: 'available' | 'unavailable' | 'withdrawn'; readonly revision: number }
export const unavailableCommunityEntry: CommunityEntryView = Object.freeze({ phase: 'unavailable', revision: 0 })

/** Captures one shell release. Panel navigation never disposes a command in flight. */
export function createCommunityEntry({ auth, release }: { auth: CommunityEntryAuth | null; release: CommunityEntryRelease | null }) {
  const scope = release ? Object.freeze({ ...release.scope }) : null
  const sources = release ? { ...release } : null
  let withdrawn = false, disposed = false, view = unavailableCommunityEntry
  const listeners = new Set<() => void>(), stops: (() => void)[] = []
  const authors = new Map<string, PartyAuthoringController>(), managers = new Map<string, PartyManagementController>()
  const giveaways = new Map<string, GiveawayAuthoringController>()
  // FE26's recovery journal is actor/event scoped. Never allocate competing writers for it.
  let pickup: { key: string; controller: GiftPickupController } | null = null
  function live() {
    return !withdrawn && !disposed && !!auth && !!scope && !!sources && !sources.signal.aborted
      && canonicalId(scope.userId ?? '') && canonicalId(scope.eventId ?? '') && scope.gate === 'open'
      && auth.session.isCurrent(scope) && auth.privacy.isCurrent(scope)
  }
  function time() {
    if (!live()) return null
    try { const clock = sources?.clock?.read(); return clock && sameCommunityScope(clock.scope, scope!)
      && Number.isFinite(clock.nowMs) && clock.nowMs >= 0 ? clock.nowMs : null } catch { return null }
  }
  function available() {
    if (!live() || time() === null) return false
    try {
      const a = sources!.access.read(), g = sources!.context.read()
      return !!a && !!g && sameCommunityScope(a.scope, scope!) && sameCommunityScope(g.scope, scope!)
        && validCapabilities(a.capabilities) && validCapabilities(g.capabilities) && a.phase === g.eventPhase
    } catch { return false }
  }
  function notify() { for (const fn of Array.from(listeners)) { try { fn() } catch { /* Notify every panel on withdrawal. */ } } }
  function update() {
    if (!live()) { withdraw(); return }
    // A withdrawn clock cannot scrub an unfinished recovery journal through context reset.
    // Retire this lease and retain the command metadata; the shell must qualify a new release.
    if (time() === null && (authors.size || managers.size || giveaways.size || pickup)) { withdraw(); return }
    view = Object.freeze({ phase: available() ? 'available' : 'unavailable', revision: view.revision + 1 }); notify()
  }
  function withdraw() {
    if (withdrawn) return
    withdrawn = true
    sources?.signal.removeEventListener('abort', withdraw)
    for (const stop of stops.splice(0).reverse()) { try { stop() } catch { /* Continue releasing the remaining sources. */ } }
    for (const c of [...authors.values(), ...managers.values(), ...giveaways.values(), ...(pickup ? [pickup.controller] : [])]) {
      try { c.dispose() } catch { /* Private presentation cannot depend on a consumer cleanup. */ }
    }
    authors.clear(); managers.clear(); giveaways.clear(); pickup = null
    view = Object.freeze({ phase: 'withdrawn', revision: view.revision + 1 }); notify()
  }
  // Every feature's command/read path consults these fenced borrowed ports at dispatch.
  const access = { read: () => available() ? sources!.access.read() : null,
    subscribe(fn: () => void) { listeners.add(fn); return () => { listeners.delete(fn) } } }
  const context = { read: () => available() ? sources!.context.read() : null, subscribe: access.subscribe }
  const places = () => {
    if (!available()) return []
    try { const p = sources!.places?.read(); return p && sameCommunityScope(p.scope, scope!)
      ? p.items.filter(item => canonicalId(item.id) && item.label.trim().length > 0) : [] } catch { return [] }
  }
  const now = () => time() ?? Number.NaN
  const valid = (id?: string) => id === undefined || canonicalId(id)
  function partyAuthor(id?: string) {
    if (!auth || !available() || !valid(id)) return null
    const key = id ?? 'new'; let c = authors.get(key)
    if (!c) { c = createPartyAuthoring({ auth, access, places, now, ...(id ? { partyId: id } : {}) }); authors.set(key, c) }
    return c
  }
  function partyManager(id: string) {
    if (!auth || !available() || !canonicalId(id)) return null
    let c = managers.get(id)
    if (!c) {
      const authority = sources!.rally?.(id)
      c = createPartyManagement({ auth, access, partyId: id, now, ...(authority ? { authority } : {}) }); managers.set(id, c)
    }
    return c
  }
  function giveawayAuthor(id?: string) {
    if (!auth || !available() || !valid(id)) return null
    const key = id ?? 'new'; let c = giveaways.get(key)
    if (!c) {
      const meeting = sources!.pickupMeeting?.(id), image = sources!.image?.(id)
      c = createGiveawayAuthoring({ auth, context, places, now, ...(id ? { giveawayId: id } : {}),
        ...(meeting ? { meeting } : {}), ...(image ? { image } : {}) }); giveaways.set(key, c)
    }
    return c
  }
  function giftPickup(giveawayId?: string, reservationId?: string) {
    if (!auth || !available() || !valid(giveawayId) || !valid(reservationId) || !giveawayId && !reservationId) return null
    const key = `${giveawayId ?? ''}:${reservationId ?? ''}`
    if (pickup && pickup.key !== key) {
      // Keep original status recovery reachable; never overwrite an unresolved actor/event intent.
      if (['sending', 'loading', 'checking', 'pending', 'uncertain', 'storage-unavailable'].includes(pickup.controller.read().phase)) return null
      pickup.controller.dispose(); pickup = null
    }
    if (!pickup) {
      const placeAuthority = sources!.pickupPlace?.(giveawayId, reservationId), journal = sources!.journal
      pickup = { key, controller: createGiftPickup({ auth, context, now: time,
        ...(giveawayId ? { giveawayId } : {}), ...(reservationId ? { reservationId } : {}),
        ...(placeAuthority ? { placeAuthority } : {}), ...(journal ? { journal } : {}) }) }
    }
    return pickup.controller
  }
  if (live()) {
    try {
      stops.push(auth!.privacy.registerCleanup(withdraw))
      sources!.signal.addEventListener('abort', withdraw, { once: true })
      for (const source of [sources!.access, sources!.context, sources!.clock, sources!.places]) {
        if (source) stops.push(source.subscribe(update))
      }
      update()
    } catch { withdraw() }
  }
  return {
    read: () => view,
    subscribe(fn: () => void) { if (disposed) return () => {}; listeners.add(fn); return () => { listeners.delete(fn) } },
    borrowed: () => live() ? { parties: sources!.parties, gifts: sources!.gifts } : null,
    partyAuthor, partyManager, giveawayAuthor, giftPickup,
    pickupRecoveryTarget: () => live() ? pickup?.key ?? null : null,
    pickupRecovery: () => live() && pickup && ['sending', 'loading', 'checking', 'pending', 'uncertain', 'storage-unavailable'].includes(pickup.controller.read().phase)
      ? pickup.controller : null,
    withdraw,
    dispose() { if (disposed) return; withdraw(); disposed = true; listeners.clear() },
  }
}
export type CommunityEntry = ReturnType<typeof createCommunityEntry>
