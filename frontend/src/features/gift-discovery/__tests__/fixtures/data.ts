import { admission } from '../../../auth/__tests__/enrollment-consent/fixtures'
import { autoAdmit } from '../../../auth/__tests__/fixtures/admission'
import management from '../../../../../../contracts/fixtures/domain/giveaway-management.json'
import { createSessionController } from '../../../auth/model/session'
import { CAPABILITIES, projectCapabilities } from '../../../../app/capabilities'
import type { GiftContext } from '../../model/controller'
import { createGiftDiscoveryController, type GiftClock } from '../../model/controller'
import type { Giveaway, Reservation } from '../../model/contract'
export { deferred } from '../../../../test-support/fixtures'

// Synthetic data only. Avatar metadata is never rendered as product artwork.
export const uid = (number: number) => `00000000-0000-4000-8000-${number.toString(16).padStart(12, '0')}`
export const eventId = uid(1), actorId = uid(3), otherActor = uid(4)
export const now = Date.parse('2026-11-07T07:00:00Z')
export function giveaway(patch: Partial<Giveaway> = {}): Giveaway { return { ...structuredClone(management.giveaway) as Giveaway, ...patch } }
export function reservation(patch: Partial<Reservation> = {}): Reservation { return { ...structuredClone(management.reservations[0]!) as Reservation, ...patch } }
export const allCapabilities = () => projectCapabilities(Object.fromEntries(CAPABILITIES.map(key => [key, { available: true, reason: null }])))
export function fixture(options: { handler?: (request: Request) => Response | Promise<Response>; clock?: GiftClock; limit?: number } = {}) {
  const admissionRequests: Request[] = [], requests: Request[] = []
  const auth = createSessionController({ eventId, timeoutMs: 2000, cookiePolicy: {
    available: true, onInvalidate: () => () => {}, run: (_kind, work) => work(), dispose() {},
  }, fetch: async input => {
    const request = input as Request; if (request.method === 'GET' && new URL(request.url).pathname === '/api/auth/enrollment/admission') { admissionRequests.push(request.clone()); return Response.json(admission()) }; requests.push(request)
    if (options.handler) return options.handler(request)
    const path = new URL(request.url).pathname
    return Response.json(path.endsWith('/me/giveaways') ? { giving: { items: [giveaway({ owner: { ...giveaway().owner, user_id: auth.session.read().userId! }, state: 'DRAFT' })], next_cursor: null },
      receiving: { items: [reservation({ recipient_id: auth.session.read().userId! })], next_cursor: null } }
      : path.endsWith('/giveaways') ? { items: [giveaway()], next_cursor: null } : giveaway())
  } })
  auth.session.replace(0, { userId: actorId, eventId, accessToken: 'synthetic-memory-token' }); autoAdmit(auth)
  let context: GiftContext | null = { scope: auth.privacy.capture(), capabilities: allCapabilities(), eventPhase: 'OPEN' }
  const listeners = new Set<() => void>()
  const controller = createGiftDiscoveryController({ auth, context: { read: () => context,
    subscribe(listener) { listeners.add(listener); return () => { listeners.delete(listener) } } },
    clock: options.clock ?? { now: () => now, setTimeout: () => 1, clearTimeout() {} }, ...(options.limit ? { limit: options.limit } : {}) })
  return { auth, controller, requests, admissionRequests,
    context: () => context,
    publish(value: GiftContext | null) { context = value; listeners.forEach(listener => listener()) },
    rebind() { context = { scope: auth.privacy.capture(), capabilities: allCapabilities(), eventPhase: 'OPEN' }; listeners.forEach(listener => listener()) },
    listeners: () => listeners.size,
    dispose() { controller.dispose(); auth.dispose() },
  }
}
