import { createSessionController } from '../../src/features/auth/model/session'
import { createElement } from 'react'
import { createGiftPickup } from '../../src/features/gift-pickup/model/controller'
import { GiftPickup } from '../../src/features/gift-pickup/ui/GiftPickup'
import { mountPrivateView } from './mounted-ui'
import { giftActivityDecoder, type Reservation } from '../../src/features/gift-discovery/model/contract'
import type { GiftContext } from '../../src/features/gift-discovery/model/controller'
import type { PickupIntent } from '../../src/features/gift-pickup/model/journal'
import { CAPABILITIES, projectCapabilities } from '../../src/app/capabilities'
import fixture from '../../../contracts/fixtures/domain/giveaway-management.json'
import { check, assertAbsent, deferred, markers, roles, until, type Cell, type Role } from './oracle'

const id = (n: number) => `00000000-0000-4000-8000-${String(n).padStart(12, '0')}`
const epoch = Date.parse('2026-11-07T08:05:00Z'), gift = fixture.giveaway, eventId = gift.event_id
export async function recipientCells(role: Role): Promise<Cell[]> {
  const actor = role === 'owner' ? gift.owner.user_id : role === 'member' ? fixture.reservations[0]!.recipient_id : id(600 + roles.indexOf(role))
  let reservation = { ...fixture.reservations[0]!, recipient_id: role === 'expired' ? actor : fixture.reservations[0]!.recipient_id,
    state: role === 'expired' ? 'EXPIRED' : 'READY', expires_at: role === 'expired' ? '2026-11-07T08:00:00Z' : fixture.reservations[0]!.expires_at } as Reservation
  const meeting = { place_id: gift.public_place_id, map_revision: 1, place_revision: 2, instructions: markers[4], identification: 'Synthetic pickup badge', expires_at: reservation.expires_at }
  const late = deferred<Response>(), calls: string[] = [], listeners = new Set<() => void>()
  let delayed = false, pickupReads = 0, storedIntent: PickupIntent | null = null
  const receiptId = id(902)
  const auth = createSessionController({ eventId, fetch: async input => {
    const request = input as Request, path = new URL(request.url).pathname
    calls.push(request.method)
    if (request.method === 'GET') {
      if (path.endsWith('/pickup')) { pickupReads++; return delayed ? late.promise : Response.json({ reservation, meeting }) }
      return Response.json(path.endsWith(gift.giveaway_id) ? gift : reservation)
    }
    check(!!request.headers.get('Idempotency-Key'), 'pickup mutation has stable command identity')
    if (path.endsWith('/pickup-code')) {
      check(role === 'member', 'only synthetic recipient may issue code')
      reservation = { ...reservation, revision: 2 }
      return Response.json({ reservation_id: reservation.reservation_id, confirmation_code: '654321', expires_at: '2026-11-07T08:06:00Z', reservation_revision: 2 })
    }
    check(role === 'owner' && path.endsWith('/claim'), 'synthetic owner explicitly claims only through frozen operation')
    reservation = { ...reservation, state: 'CLAIMED', revision: 2, claim_receipt_id: receiptId }
    return Response.json({ receipt_id: receiptId, reservation_id: reservation.reservation_id, giveaway_id: gift.giveaway_id, state: 'CLAIMED', revision: 2, claimed_at: '2026-11-07T08:05:00Z' })
  } })
  auth.session.replace(0, { userId: actor, eventId, accessToken: 'qa04-recipient-fixture-token' })
  const capabilities = projectCapabilities(Object.fromEntries(CAPABILITIES.map(key => [key, { available: true, reason: null }])))
  let context: GiftContext | null = role === 'blocked' || role === 'suspended' ? null : { scope: auth.privacy.capture(), eventPhase: 'OPEN', capabilities }
  const publish = (next: GiftContext | null) => { context = next; listeners.forEach(listener => listener()) }
  const controller = createGiftPickup({ auth, giveawayId: gift.giveaway_id, reservationId: reservation.reservation_id, now: () => epoch,
    context: { read: () => context, subscribe(listener) { listeners.add(listener); return () => { listeners.delete(listener) } } },
    journal: { read: () => storedIntent, write: value => { storedIntent = value }, clear: () => { storedIntent = null } },
    placeAuthority: { read: () => ({ scope: auth.privacy.capture(), placeId: gift.public_place_id, placeRevision: 2, mapRevision: 1, expiresAt: epoch + 60000 }), subscribe: () => () => undefined },
  })
  const view = mountPrivateView('recipient', role, createElement(GiftPickup, { controller }))
  try {
    // SELF collections may be legitimately empty. A stolen recipient row must fail actual context validation.
    const scope = auth.privacy.capture(), path = `/api/concert/events/${eventId}/me/giveaways`
    const receiving = role === 'member' || role === 'expired' ? [reservation] : []
    const giving = role === 'owner' ? [gift] : []
    giftActivityDecoder(scope, {} )({ giving: { items: giving, next_cursor: null }, receiving: { items: receiving, next_cursor: null } }, { scope, path, status: 200 })
    let rejected = false
    try { giftActivityDecoder(scope, {})({ giving: { items: [], next_cursor: null }, receiving: { items: [{ ...reservation, recipient_id: id(990) }], next_cursor: null } }, { scope, path, status: 200 }) } catch { rejected = true }
    check(rejected, 'SELF recipient collection rejects another canonical recipient')
    await controller.load()
    // The actual mounted component starts its read in useEffect; join that read if already in flight.
    await until(() => controller.read().phase !== 'loading')
    await controller.loadPickup()
    if (role === 'member') {
      check(controller.read().pickup?.meeting.instructions === markers[4], 'authorized recipient guidance visible')
      view.visible(markers[4])
      delayed = true
      const pending = controller.loadPickup()
      await until(() => pickupReads === 2)
      publish(null)
      late.resolve(Response.json({ reservation, meeting })); await pending
      assertAbsent(controller.read())
      view.absent('after context loss and delayed pickup')
      check(controller.read().pickup === null && controller.read().reservation === null, 'late pickup cannot restore lost context')
      delayed = false; publish({ scope: auth.privacy.capture(), eventPhase: 'OPEN', capabilities })
      await controller.load(); await controller.loadPickup(); await controller.issueCode()
      check(controller.read().code?.confirmation_code === '654321', 'explicit recipient code issued through canonical mutation/readback')
      view.text('654321')
    } else if (role === 'owner') {
      check(controller.read().reservation?.recipient_id === reservation.recipient_id, 'own giveaway owner may retain authorized reservation')
      check(controller.read().pickup === null, 'owner does not automatically receive recipient-only pickup details')
      view.absent('owner has no recipient-only guidance')
      controller.setClaimCode('654321'); await controller.claim()
      check(controller.read().phase === 'succeeded' && controller.read().reservation?.state === 'CLAIMED', 'owner claim verifies current state')
    } else {
      await controller.issueCode(); controller.setClaimCode('654321'); await controller.claim()
      check(!calls.includes('POST'), 'unauthorized or expired pickup does not mutate')
      check(controller.read().pickup === null && controller.read().code === null, 'private pickup/code hidden')
      if (role === 'stranger' || role === 'admin') check(controller.read().reservation === null, 'malicious200 foreign reservation rejected')
      assertAbsent(controller.read())
      view.absent('foreign recipient or denied/expired authority')
    }
    auth.privacy.changePolicy(auth.privacy.capture(), 'block')
    check(controller.read().reservation === null && controller.read().code === null && controller.read().pickup === null && controller.read().confirmationCode === '', 'recipient state and draft code scrubbed')
    assertAbsent(controller.read())
    view.absent('after block', ['654321']); view.finish()
    return (['list', 'detail', 'mutation', 'cache'] as const).map(operation => ({ role, operation, status: 'PASS', provenance: 'SYNTHETIC_FIXTURE',
      evidence: 'Actual SELF activity decoder, pickup controller and mounted component; synthetic public context/HTTP/journal/place authority. Cross-recipient200 rejection, expired/code guards, explicit owner/recipient mutations/readback, context-loss late-response and private-state/DOM cleanup. No real reservation/pickup/backend-role/product-shell/WS certification.' }))
  } finally { view.dispose(); controller.dispose(); auth.dispose() }
}
