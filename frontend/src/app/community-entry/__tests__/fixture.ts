import { admission } from '../../../features/auth/__tests__/enrollment-consent/fixtures'
import { createSessionController } from '../../../features/auth/model/session'
import { createPartyController, type PartyAccess } from '../../../features/party-discovery'
import { createGiftDiscoveryController, type GiftContext } from '../../../features/gift-discovery'
import { createSessionIntentJournal } from '../../../features/gift-pickup'
import { gift, reservation, giftId, reservationId, event, owner, recipient, place, now, meeting } from '../../../features/gift-pickup/__tests__/harness'
import { party, profile, ids } from '../../../features/party-discovery/__tests__/fixtures/data'
import { CAPABILITIES, projectCapabilities } from '../../capabilities'
import type { CommunityEntryRelease } from '../ports'
import type { PrivacyScope } from '../../../shared/query/privacy'

export { giftId, reservationId, place, ids, now }
export function fixture(options: { owner?: boolean; storage?: Pick<Storage, 'getItem' | 'setItem' | 'removeItem'> } = {}) {
  const admissionRequests: Request[] = [], requests: Request[] = [], signals = new Set<() => void>(), clockListeners = new Set<() => void>()
  const stored = new Map<string, string>(), storage = options.storage ?? { getItem: (k: string) => stored.get(k) ?? null,
    setItem: (k: string, v: string) => { stored.set(k, v) }, removeItem: (k: string) => { stored.delete(k) } }
  const actor = options.owner ? owner : recipient
  let lost = false, held = false
  const auth = createSessionController({ eventId: event, cookiePolicy: { available: true, onInvalidate: () => () => {}, run: (_k, w) => w(), dispose() {} }, fetch: async input => {
    const r = input as Request; if (r.method === 'GET' && new URL(r.url).pathname === '/api/auth/enrollment/admission') { admissionRequests.push(r.clone()); return Response.json(admission()) }; requests.push(r.clone()); const path = new URL(r.url).pathname
    if (r.method !== 'GET') {
      if (lost) throw new TypeError('Synthetic response lost')
      held = true; return Response.json(reservation({ recipient_id: actor, state: 'HELD' }))
    }
    if (path.endsWith('/commands')) return Response.json({ receipt_id: ids.receipt, event_id: event, actor_id: actor,
      operation_id: new URL(r.url).searchParams.get('operation_id'), idempotency_key: new URL(r.url).searchParams.get('idempotency_key'),
      state: 'PENDING', resource_id: null, resource_revision: null, failure_reason: null, created_at: '2026-11-03T01:00:00Z',
      expires_at: '2026-11-08T00:00:00Z', result_status: null })
    const g = { ...gift(), owner: { ...gift().owner, user_id: owner }, available: held ? 0 : 1 }
    if (path.endsWith('/pickup')) return Response.json({ reservation: reservation(), meeting: meeting() })
    if (path.endsWith('/management')) return Response.json({ giveaway: g, reserved_unclaimed: 1, quantity_claimed: 1,
      reservations: [reservation()], next_cursor: null })
    if (path.includes('/reservations/')) return Response.json(reservation())
    if (path.includes('/giveaways/')) return Response.json(g)
    if (path.endsWith('/members')) return Response.json({ items: [{ event_id: event, party_id: ids.party,
      user_id: actor, role: 'OWNER', state: 'ACTIVE', revision: 1, joined_at: '2026-11-01T00:00:00Z' }], next_cursor: null })
    if (path.endsWith('/applications')) return Response.json({ items: [], next_cursor: null })
    if (path.endsWith('/rally')) return Response.json({ party_id: ids.party, event_id: event, meeting_revision: 1, beacon_id: null, entries: [] })
    return Response.json(party({ event_id: event, owner: profile(actor) }))
  } })
  auth.session.replace(0, { userId: actor, eventId: event, accessToken: 'fixture-token' })
  const scope = auth.privacy.capture(), lifetime = new AbortController()
  const capabilities = projectCapabilities(Object.fromEntries(CAPABILITIES.map(k => [k, { available: true, reason: null }])))
  let access: PartyAccess | null = { scope, phase: 'OPEN', capabilities }, context: GiftContext | null = { scope, eventPhase: 'OPEN', capabilities }
  let clockScope: PrivacyScope = scope, clockAvailable = true, placeScope = scope
  const subscribe = (fn: () => void) => { signals.add(fn); return () => { signals.delete(fn) } }
  const accessPort = { read: () => access, subscribe }, contextPort = { read: () => context, subscribe }
  const parties = createPartyController({ auth, access: accessPort, now: () => now })
  const gifts = createGiftDiscoveryController({ auth, context: contextPort })
  const journal = createSessionIntentJournal(storage)
  const release: CommunityEntryRelease = { scope, signal: lifetime.signal, parties, gifts, access: accessPort, context: contextPort,
    clock: { read: () => clockAvailable ? { scope: clockScope, nowMs: now } : null, subscribe(fn) { clockListeners.add(fn); return () => { clockListeners.delete(fn) } } },
    places: { read: () => ({ scope: placeScope, items: [{ id: place, label: 'SYNTHETIC qualified gift place' }, { id: ids.place, label: 'SYNTHETIC party place' }] }), subscribe }, journal }
  return { auth, scope, release, lifetime, requests, admissionRequests, stored, journal, parties, gifts,
    loseResponse() { lost = true },
    clock(available: boolean, s = scope) { clockAvailable = available; clockScope = s; Array.from(clockListeners).forEach(fn => fn()) },
    places(s: PrivacyScope) { placeScope = s; Array.from(signals).forEach(fn => fn()) },
    context(value: GiftContext | null) { context = value; Array.from(signals).forEach(fn => fn()) },
    access(value: PartyAccess | null) { access = value; Array.from(signals).forEach(fn => fn()) },
    subscriptions: () => ({ ports: signals.size, clock: clockListeners.size }),
    dispose() { parties.dispose(); gifts.dispose(); auth.dispose() },
  }
}
