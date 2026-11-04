import { useEffect, useId, useState, useSyncExternalStore, type ReactNode } from 'react'
import { ShellLink } from '../../../app/navigation'
import { Button, Tabs } from '../../../shared/ui'
import type { Giveaway, GiftFilters } from '../model/contract'
import { giveawayStatus, reservationEligibility, type GiftDiscoveryController, type GiftDiscoveryState, type GiftPage, type GiftResource } from '../model/controller'
import { ConcertRow, ConcertState } from '../../../shared/ui/concert-window'
import { canReviewReservation, ReferenceGiftRow, ReferencePickupRow, type RenderGiftImage } from './reference-presentation'
export type { GiftImageRequest, RenderGiftImage } from './reference-presentation'
import './gifts.css'
import './reference-gifts.css'

export interface GiftPlaceOption { readonly id: string; readonly label: string }
export function useGifts(controller: GiftDiscoveryController) {
  const state = useSyncExternalStore(controller.subscribe, controller.read)
  useEffect(() => { controller.start() }, [controller])
  return state
}
export function GiftAccess({ state, children }: { state: GiftDiscoveryState; children: ReactNode }) {
  if (!state.context || state.disposed) return <div className="gift-notice" role="status"><h2>Gift access is unavailable</h2>
    <p>Check your current account and event before continuing.</p></div>
  if (!state.context.capabilities.gifts.available) return <div className="gift-notice" role="status"><h2>Gifts are unavailable</h2>
    <p>{state.context.capabilities.gifts.reason.replaceAll('_', ' ').toLowerCase()}.</p></div>
  return children
}
export function GiftResult<T>({ resource, now, retry, children, label }: {
  resource: GiftResource<T>; now: number; retry(): void; children(data: T): ReactNode; label: string
}) {
  if (resource.phase === 'ready' && resource.data !== null) return children(resource.data)
  if (resource.phase === 'idle' || resource.phase === 'loading') return <p role="status">Loading {label}…</p>
  const title = { 'signed-out': 'Sign in to see gifts', denied: 'These gifts are not available to this account', missing: 'This giveaway is unavailable',
    'rate-limited': 'Please wait before retrying', invalid: 'Gift information could not be verified', unavailable: 'Gifts could not be loaded' }[resource.failure ?? 'unavailable']
  const cooling = resource.retryAt !== null && now < resource.retryAt
  return <section className="gift-notice" role="alert"><h2>{title}</h2>
    <p>{resource.failure === 'missing' ? 'It may have changed or may no longer be visible.' : 'No unverified gift information is shown. You can try again.'}</p>
    {resource.failure !== 'signed-out' && <Button variant="secondary" disabled={cooling} onClick={retry}>Try again</Button>}
  </section>
}
const timeFormat = new Intl.DateTimeFormat('en-GB', { timeZone: 'Asia/Seoul', month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit', hour12: false })
export function GiftTime({ value }: { value: string }) { return <time dateTime={value}>{timeFormat.format(Date.parse(value))} KST</time> }
export function GiftRow({ gift, now, own = false }: { gift: Giveaway; now: number; own?: boolean }) {
  return <ConcertRow title={gift.state === 'DRAFT' ? gift.title : <ShellLink id={`gift-${gift.giveaway_id}`} to={`/gifts/${gift.giveaway_id}`}>{gift.title}</ShellLink>}
    metadata={<><span className="gift-status">{giveawayStatus(gift, now)}</span><br />{own ? 'Your giveaway' : `From ${gift.owner.display_name}`} · <strong>{gift.available} remaining</strong> of {gift.quantity_total}</>}
    action={gift.state !== 'DRAFT' && <ShellLink className="gift-details" id={`gift-details-${gift.giveaway_id}`} to={`/gifts/${gift.giveaway_id}`} aria-label={`View details for ${gift.title}`}>Details</ShellLink>}>
    Pickup <GiftTime value={gift.pickup_starts_at} /> – <GiftTime value={gift.pickup_ends_at} />
    {gift.state === 'DRAFT' && <p className="gift-meta">Only you can see this draft.</p>}
  </ConcertRow>
}
export function GiftPagination<T>({ page, label, turn }: { page: GiftPage<T>; label: string; turn(direction: 'previous' | 'next'): void }) {
  return <nav className="gift-pagination" aria-label={`${label} pages`}>
    <Button variant="secondary" disabled={!page.canPrevious || page.phase !== 'ready'} onClick={() => turn('previous')}>Previous {label.toLowerCase()}</Button>
    <span aria-live="polite">Page {page.page}</span>
    <Button variant="secondary" disabled={page.phase !== 'ready' || !page.data?.next_cursor || page.page >= 20} onClick={() => turn('next')}>Next {label.toLowerCase()}</Button>
    {page.page >= 20 && page.data?.next_cursor && <p>Refresh to start a new browse session.</p>}
  </nav>
}
function localTime(value: string | undefined) {
  if (!value) return ''
  return new Date(Date.parse(value) + 9 * 60 * 60 * 1000).toISOString().slice(0, 16)
}
const utcTime = (value: string) => value ? new Date(`${value}:00+09:00`).toISOString() : undefined
function GiftFilterForm({ controller, state, places }: { controller: GiftDiscoveryController; state: GiftDiscoveryState; places: readonly GiftPlaceOption[] }) {
  const id = useId(), [draft, setDraft] = useState({ status: state.filters.state ?? '', stock: state.filters.in_stock === undefined ? '' : String(state.filters.in_stock),
    after: localTime(state.filters.pickup_after), before: localTime(state.filters.pickup_before), place: state.filters.public_place_id ?? '' })
  const [invalidTime, setInvalidTime] = useState(false)
  const change = (key: keyof typeof draft, value: string) => setDraft(previous => ({ ...previous, [key]: value }))
  return <form className="gift-filters" onSubmit={event => {
    event.preventDefault(); setInvalidTime(false)
    try {
      const after = utcTime(draft.after), before = utcTime(draft.before)
      const filters: GiftFilters = { ...(draft.status ? { state: draft.status as NonNullable<GiftFilters['state']> } : {}),
        ...(draft.stock ? { in_stock: draft.stock === 'true' } : {}), ...(draft.place ? { public_place_id: draft.place } : {}),
        ...(after ? { pickup_after: after } : {}), ...(before ? { pickup_before: before } : {}) }
      controller.setFilters(filters)
    } catch { setInvalidTime(true) }
  }}>
    <details><summary>Filter giveaways</summary><div className="gift-filter-fields">
      <label htmlFor={`${id}-status`}>Status<select aria-label="Status" id={`${id}-status`} value={draft.status} onChange={e => change('status', e.target.value)}>
        <option value="">All published</option><option value="OPEN">Open</option><option value="PAUSED">Paused</option><option value="CLOSED">Completed</option><option value="CANCELLED">Cancelled</option>
      </select></label>
      <label htmlFor={`${id}-stock`}>Stock<select aria-label="Stock" id={`${id}-stock`} value={draft.stock} onChange={e => change('stock', e.target.value)}>
        <option value="">Any stock</option><option value="true">Available</option><option value="false">Sold out</option>
      </select></label>
      <label htmlFor={`${id}-place`}>Public place<select aria-label="Public place" id={`${id}-place`} value={draft.place} disabled={places.length === 0} onChange={e => change('place', e.target.value)}>
        <option value="">All places</option>{places.map(place => <option key={place.id} value={place.id}>{place.label}</option>)}
      </select></label>
      <label htmlFor={`${id}-after`}>Pickup starts from (KST)<input id={`${id}-after`} type="datetime-local" value={draft.after} onChange={e => change('after', e.target.value)} /></label>
      <label htmlFor={`${id}-before`}>Pickup starts before (KST)<input id={`${id}-before`} type="datetime-local" value={draft.before} onChange={e => change('before', e.target.value)} /></label>
      <div className="gift-actions"><Button type="submit">Apply filters</Button><Button variant="quiet" onClick={() => {
        setDraft({ status: '', stock: '', place: '', after: '', before: '' }); setInvalidTime(false); controller.setFilters({})
      }}>Clear filters</Button></div>
      {(state.filterInvalid || invalidTime) && <p role="alert">Choose valid filters. The end of the range must be after its start.</p>}
    </div></details>
  </form>
}
export interface GiftDiscoveryProps {
  readonly controller: GiftDiscoveryController
  readonly places?: readonly GiftPlaceOption[]
  readonly renderGiftImage?: RenderGiftImage
}
export function GiftDiscovery({ controller, places = [], renderGiftImage }: GiftDiscoveryProps) {
  const state = useGifts(controller)
  return <section className="gifts-reference" aria-label="Giveaway discovery"><GiftAccess state={state}>
    <GiftCollections key={JSON.stringify(state.context?.scope)} controller={controller} state={state} places={places} renderGiftImage={renderGiftImage} />
  </GiftAccess></section>
}
function GiftCollections({ controller, state, places, renderGiftImage }: {
  controller: GiftDiscoveryController; state: GiftDiscoveryState; places: readonly GiftPlaceOption[]; renderGiftImage?: RenderGiftImage | undefined
}) {
  const [collection, setCollection] = useState('browse')
  useEffect(() => {
    if (collection !== 'browse' && state.context?.capabilities.gifts.available) {
      if (state.giving.phase === 'idle' && state.receiving.phase === 'idle') void controller.loadActivity()
    }
  }, [collection, controller, state.context, state.giving.phase, state.receiving.phase])
  const giftRows = (rows: readonly Giveaway[], own = false) => <ul className="gift-reference-list">{rows.map(gift => <ReferenceGiftRow key={gift.giveaway_id} gift={gift} state={state}
    {...(places.find(place => place.id === gift.public_place_id) ? { place: places.find(place => place.id === gift.public_place_id)!.label } : {})}
    renderGiftImage={renderGiftImage} own={own} />)}</ul>
  const browse = <>
    {state.context?.eventPhase === 'READ_ONLY' && <p className="gift-reference-note" role="status">Read-only event. New reservations are unavailable.</p>}
    <GiftResult resource={state.list} label="giveaways" retry={() => { void controller.retryList() }} now={state.now}>{data => data.items.length
      ? giftRows(data.items) : <ConcertState title="No matching gifts">No giveaways match these filters on this page. Try another filter or return later.</ConcertState>}</GiftResult>
    <div className="gift-reference-tools"><GiftFilterForm controller={controller} state={state} places={places} />
      {(state.list.canPrevious || state.list.data?.next_cursor || state.list.page > 1) && <GiftPagination page={state.list} label="Giveaways" turn={direction => { void controller.pageList(direction) }} />}</div>
  </>
  const giving = <><GiftResult resource={state.giving} label="giving activity" retry={() => { void controller.retryActivity('giving') }} now={state.now}>{data => data.items.length
    ? giftRows(data.items, true) : <ConcertState title="Nothing shared on this page">Your giveaways will appear here.</ConcertState>}</GiftResult>
    {(state.giving.canPrevious || state.giving.data?.next_cursor || state.giving.page > 1) && <GiftPagination page={state.giving} label="Giving" turn={direction => { controller.pageActivity('giving', direction) }} />}</>
  const receiving = <><GiftResult resource={state.receiving} label="pickup activity" retry={() => { void controller.retryActivity('receiving') }} now={state.now}>{data => data.items.length
    ? <ul className="gift-reference-list">{data.items.map(reservation => <ReferencePickupRow key={reservation.reservation_id} reservation={reservation} now={state.now} />)}</ul>
    : <ConcertState title="No pickups on this page">Your own reservation history will appear here.</ConcertState>}</GiftResult>
    {(state.receiving.canPrevious || state.receiving.data?.next_cursor || state.receiving.page > 1) && <GiftPagination page={state.receiving} label="Receiving" turn={direction => { controller.pageActivity('receiving', direction) }} />}
    <p className="gift-reference-note">Pickup instructions and codes are available only through your authorized pickup journey.</p></>
  return <><Tabs label="Gift collections" value={collection} onValueChange={setCollection} items={[
    { value: 'browse', label: 'Browse', content: collection === 'browse' ? browse : null },
    { value: 'giving', label: 'My giving', content: collection === 'giving' ? giving : null },
    { value: 'receiving', label: 'My pickups', content: collection === 'receiving' ? receiving : null },
  ]} /><footer className="gift-reference-footnote"><p>Availability is checked in the reservation journey. Stock and pickup windows can change.</p>
    <ShellLink id="gift-own" to="/gifts/mine">My gifts →</ShellLink></footer></>
}

export function GiftDetail({ controller, giveawayId, places = [] }: { controller: GiftDiscoveryController; giveawayId: string; places?: readonly GiftPlaceOption[] }) {
  const state = useGifts(controller), key = JSON.stringify(state.context?.scope)
  useEffect(() => { controller.select(giveawayId) }, [controller, giveawayId, key])
  return <section className="gifts-view" aria-label="Giveaway detail"><GiftAccess state={state}>
    <ShellLink id="gift-back" to="/gifts">← All giveaways</ShellLink>
    <GiftResult resource={state.selectedId === giveawayId ? state.detail : { phase: 'loading', data: null, failure: null, retryAt: null }} now={state.now} label="giveaway" retry={() => void controller.retryDetail()}>{gift => <>
      <header className="gift-detail-heading"><p className="gift-eyebrow">{giveawayStatus(gift, state.now)}</p><h2>{gift.title}</h2><p>Shared by {gift.owner.display_name}</p></header>
      <p className="gift-description">{gift.description || 'No item description was added.'}</p>
      <dl className="gift-facts"><div><dt>Stock</dt><dd><strong>{gift.available} remaining</strong> of {gift.quantity_total}</dd></div>
        <div><dt>Pickup window</dt><dd><GiftTime value={gift.pickup_starts_at} /><br />to <GiftTime value={gift.pickup_ends_at} /></dd></div>
        <div><dt>Public place</dt><dd>{places.find(place => place.id === gift.public_place_id)?.label ?? 'Event place reference'}<br />
          <ShellLink id={`gift-place-${gift.public_place_id}`} to={`/world/places/${gift.public_place_id}`}>View public place reference</ShellLink></dd></div></dl>
      <p className="gift-meta">Check the current place information before travelling. This public reference does not confirm arrival or disclose private pickup instructions.</p>
      <section className="gift-reservation" aria-label="Reservation availability"><h3>Reservation availability</h3><p>{reservationEligibility(gift, state)}</p>
        <Button disabled aria-describedby="gift-reserve-note">Reserve unavailable</Button>
        {canReviewReservation(gift, state) && <ShellLink id={`gift-detail-reserve-${gift.giveaway_id}`} to={`/gifts/${gift.giveaway_id}/reservation`}>Review reservation</ShellLink>}<p id="gift-reserve-note" className="gift-meta">Open the reservation journey to confirm current eligibility. No item has been reserved from this view.</p></section>
      <ShellLink id="gift-detail-own" to="/gifts/mine">Check my gifts →</ShellLink>
    </>}</GiftResult>
  </GiftAccess></section>
}
