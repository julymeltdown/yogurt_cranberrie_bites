import type { ReactNode } from 'react'
import { ShellLink } from '../../../app/navigation'
import type { Giveaway, Reservation } from '../model/contract'
import { giveawayStatus, type GiftContext, type GiftDiscoveryState } from '../model/controller'

/** Root owns authorized reads, shared caching and synchronous privacy eviction. */
export interface GiftImageRequest {
  readonly mediaId: string
  readonly giveawayId: string
  readonly scope: GiftContext['scope']
  readonly alt: string
}
export type RenderGiftImage = (request: GiftImageRequest) => ReactNode
export type RenderGiftOwnerPortrait = (owner: Giveaway['owner']) => ReactNode

const shortTime = new Intl.DateTimeFormat('en-GB', { timeZone: 'Asia/Seoul', hour: '2-digit', minute: '2-digit', hour12: false })
const longTime = new Intl.DateTimeFormat('en-GB', { timeZone: 'Asia/Seoul', month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit', hour12: false })
export function canReviewReservation(gift: Giveaway, state: GiftDiscoveryState) {
  const context = state.context
  return !!context && !state.disposed && context.capabilities.gifts.available && context.capabilities.mutations.available
    && (context.eventPhase === 'OPEN' || context.eventPhase === 'LIVE') && gift.owner.user_id !== context.scope.userId
    && gift.state === 'OPEN' && gift.available > 0 && state.now < Date.parse(gift.pickup_ends_at)
}
export function ReferenceGiftRow({ gift, state, place, renderGiftImage, own = false }: {
  readonly gift: Giveaway; readonly state: GiftDiscoveryState; readonly place?: string
  readonly renderGiftImage?: RenderGiftImage | undefined; readonly own?: boolean
  readonly renderOwnerPortrait?: RenderGiftOwnerPortrait | undefined
}) {
  const status = giveawayStatus(gift, state.now), published = gift.state !== 'DRAFT'
  const reservation = state.receiving.data?.items.find(row => row.giveaway_id === gift.giveaway_id
    && (row.state === 'HELD' || row.state === 'READY') && Date.parse(row.expires_at) > state.now)
  const image = gift.image_media_id && state.context && renderGiftImage
    ? renderGiftImage({ mediaId: gift.image_media_id, giveawayId: gift.giveaway_id, scope: state.context.scope, alt: gift.title }) : null
  return <li className="gift-reference-row">
    <div className="gift-reference-image" key={`${JSON.stringify(state.context?.scope)}:${gift.giveaway_id}:${gift.image_media_id}`}>
      {image ?? <span className="gift-reference-no-photo">No photo</span>}
    </div>
    <div className="gift-reference-copy">
      <h3>{published ? <ShellLink id={`gift-${gift.giveaway_id}`} to={`/gifts/${gift.giveaway_id}`}>{gift.title}</ShellLink> : gift.title}</h3>
      <p className="gift-reference-stock"><strong>{gift.available} left</strong><span className="gift-reference-sr"> of {gift.quantity_total}</span></p>
      <div className="gift-reference-owner">
        <p className="gift-reference-byline">{own ? 'Your giveaway' : <>by {gift.owner.display_name}</>}</p>
      </div>
      <div className="gift-reference-location"><p className="gift-reference-place"><svg viewBox="0 0 16 20" aria-hidden="true"><path d="M8 19S1 10 1 7a7 7 0 0 1 14 0c0 3-7 12-7 12Z" fill="currentColor"/><circle cx="8" cy="7" r="2.4" fill="var(--mcr-paper)"/></svg><span>{place ?? 'Event place reference'}</span></p>
      <p className="gift-reference-time"><time dateTime={gift.pickup_starts_at} aria-label={longTime.format(Date.parse(gift.pickup_starts_at)) + ' KST'} title={longTime.format(Date.parse(gift.pickup_starts_at))}>{shortTime.format(Date.parse(gift.pickup_starts_at))}</time><span aria-hidden="true">–</span><time dateTime={gift.pickup_ends_at} aria-label={longTime.format(Date.parse(gift.pickup_ends_at)) + ' KST'} title={longTime.format(Date.parse(gift.pickup_ends_at))}>{shortTime.format(Date.parse(gift.pickup_ends_at))}</time> KST</p></div>
      {!status.startsWith('Open') && <p className="gift-reference-status">{status}</p>}
      {own && gift.state === 'DRAFT' && <p className="gift-reference-status">Only you can see this draft.</p>}
    </div>
    <div className="gift-reference-action">
      {reservation ? <ShellLink id={`gift-pickup-${reservation.reservation_id}`} to={`/gifts/pickups/${reservation.reservation_id}`} aria-label={`View pickup for ${gift.title}`}>My pickup</ShellLink>
        : own && published ? <ShellLink id={`gift-manage-${gift.giveaway_id}`} to={`/gifts/${gift.giveaway_id}/manage`} aria-label={`Manage ${gift.title}`}>Manage</ShellLink>
          : canReviewReservation(gift, state) ? <ShellLink id={`gift-reserve-${gift.giveaway_id}`} to={`/gifts/${gift.giveaway_id}/reservation`} aria-label={`Review reservation for ${gift.title}`}>Reserve</ShellLink>
            : !own && <button type="button" disabled aria-label={`Reservations unavailable for ${gift.title}`}>Reserve</button>}
    </div>
  </li>
}
export function ReferencePickupRow({ reservation, now }: { readonly reservation: Reservation; readonly now: number }) {
  const active = (reservation.state === 'HELD' || reservation.state === 'READY') && now < Date.parse(reservation.expires_at)
  const status = active ? reservation.state === 'READY' ? 'Ready for pickup' : 'Held' : ['HELD', 'READY'].includes(reservation.state) ? 'Expiry reached · refresh status' : reservation.state.toLowerCase()
  return <li className="gift-reference-pickup"><div><h3>Reservation</h3><p>{status}</p><p>Expires <time dateTime={reservation.expires_at}>{longTime.format(Date.parse(reservation.expires_at))} KST</time></p></div>
    <ShellLink id={`gift-receiving-${reservation.reservation_id}`} to={`/gifts/pickups/${reservation.reservation_id}`}>View pickup</ShellLink>
  </li>
}
