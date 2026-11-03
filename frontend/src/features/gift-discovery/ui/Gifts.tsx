import { useEffect, useId, useState, useSyncExternalStore, type ReactNode } from 'react'
import { ShellLink } from '../../../app/navigation'
import { Button } from '../../../shared/ui'
import type { Giveaway, GiftFilters } from '../model/contract'
import { giveawayStatus, reservationEligibility, type GiftDiscoveryController, type GiftDiscoveryState, type GiftPage, type GiftResource } from '../model/controller'
import './gifts.css'

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
  return <li className="gift-row">
    <div className="gift-row-heading"><h3>{gift.state === 'DRAFT' ? gift.title : <ShellLink id={`gift-${gift.giveaway_id}`} to={`/gifts/${gift.giveaway_id}`}>{gift.title}</ShellLink>}</h3>
      <span className="gift-status">{giveawayStatus(gift, now)}</span></div>
    <p className="gift-meta">{own ? 'Your giveaway' : `From ${gift.owner.display_name}`} · <strong>{gift.available} remaining</strong> of {gift.quantity_total}</p>
    <p className="gift-meta">Pickup <GiftTime value={gift.pickup_starts_at} /> – <GiftTime value={gift.pickup_ends_at} /></p>
    {gift.state === 'DRAFT' && <p className="gift-meta">Only you can see this draft.</p>}
  </li>
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
export function GiftDiscovery({ controller, places = [] }: { controller: GiftDiscoveryController; places?: readonly GiftPlaceOption[] }) {
  const state = useGifts(controller)
  return <section className="gifts-view" aria-label="Giveaway discovery"><GiftAccess state={state}>
    <header className="gift-intro"><div><p className="gift-eyebrow">Fan to fan</p><h2>Find a little something</h2></div>
      <ShellLink id="gift-own" to="/gifts/mine">My gifts →</ShellLink></header>
    <p>Browse gifts shared at this event. Stock and pickup times can change.</p>
    <GiftFilterForm key={JSON.stringify(state.context?.scope)} controller={controller} state={state} places={places} />
    <GiftResult resource={state.list} now={state.now} label="giveaways" retry={() => void controller.retryList()}>{page => <>
      {page.items.length ? <ul className="gift-list">{page.items.map(gift => <GiftRow key={gift.giveaway_id} gift={gift} now={state.now} />)}</ul>
        : <p className="gift-empty">No giveaways on this page. Try different filters{page.next_cursor ? ' or continue to the next page' : ''}.</p>}
    </>}</GiftResult>
    <GiftPagination page={state.list} label="Giveaways" turn={direction => controller.pageList(direction)} />
  </GiftAccess></section>
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
        <Button disabled aria-describedby="gift-reserve-note">Reserve unavailable</Button><p id="gift-reserve-note" className="gift-meta">Reservations and pickup details are not available from this view.</p></section>
      <ShellLink id="gift-detail-own" to="/gifts/mine">Check my gifts →</ShellLink>
    </>}</GiftResult>
  </GiftAccess></section>
}
