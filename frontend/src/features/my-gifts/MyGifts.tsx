import { useEffect, useState } from 'react'
import { ShellLink } from '../../app/navigation'
import { Button } from '../../shared/ui'
import { GiftAccess, GiftPagination, GiftResult, GiftRow, GiftTime, useGifts } from '../gift-discovery/ui/Gifts'
import type { GiftDiscoveryController } from '../gift-discovery/model/controller'

export function MyGifts({ controller }: { controller: GiftDiscoveryController }) {
  const state = useGifts(controller), [collection, setCollection] = useState<'giving' | 'receiving'>('giving')
  const key = JSON.stringify(state.context?.scope)
  useEffect(() => { if (state.context?.capabilities.gifts.available) void controller.loadActivity() }, [controller, key])
  return <section className="gifts-view" aria-label="My gift activity"><GiftAccess state={state}>
    <header className="gift-intro"><h2>My gifts</h2><ShellLink id="my-gifts-browse" to="/gifts">Browse giveaways →</ShellLink></header>
    <p>What you share. What you receive.</p>
    <div className="gift-tabs" role="group" aria-label="Gift activity collections">
      <Button variant="secondary" aria-pressed={collection === 'giving'} onClick={() => setCollection('giving')}>GIVING</Button>
      <Button variant="secondary" aria-pressed={collection === 'receiving'} onClick={() => setCollection('receiving')}>RECEIVING</Button>
    </div>
    {collection === 'giving' ? <section aria-label="Giving activity"><GiftResult resource={state.giving} now={state.now} label="giving activity" retry={() => void controller.retryActivity('giving')}>{page => <>
      {page.items.length ? <ul className="gift-list">{page.items.map(gift => <GiftRow key={gift.giveaway_id} gift={gift} now={state.now} own />)}</ul>
        : <p className="gift-empty">No giving activity on this page{page.next_cursor ? '. You can continue to the next page.' : '.'}</p>}
      <p className="gift-meta">Drafts stay private. Owner tools are unavailable here.</p>
    </>}</GiftResult><GiftPagination page={state.giving} label="Giving" turn={direction => controller.pageActivity('giving', direction)} /></section>
      : <section aria-label="Receiving activity"><GiftResult resource={state.receiving} now={state.now} label="receiving activity" retry={() => void controller.retryActivity('receiving')}>{page => <>
        {page.items.length ? <ul className="gift-list">{page.items.map(reservation => <li className="gift-row" key={reservation.reservation_id}>
          <div className="gift-row-heading"><h3>Reservation</h3><span className="gift-status">{reservation.state === 'HELD' || reservation.state === 'READY'
            ? state.now >= Date.parse(reservation.expires_at) ? 'Expiry reached · refresh status' : reservation.state === 'READY' ? 'Ready for pickup' : 'Held' : reservation.state.toLowerCase()}</span></div>
          <p className="gift-meta">Expires <GiftTime value={reservation.expires_at} /></p>
          <ShellLink id={`my-reservation-${reservation.reservation_id}`} to={`/gifts/${reservation.giveaway_id}`}>View giveaway</ShellLink>
          <p className="gift-meta">Private pickup instructions and confirmation codes are not shown here.</p>
        </li>)}</ul> : <p className="gift-empty">No receiving activity on this page{page.next_cursor ? '. You can continue to the next page.' : '.'}</p>}
      </>}</GiftResult><GiftPagination page={state.receiving} label="Receiving" turn={direction => controller.pageActivity('receiving', direction)} /></section>}
  </GiftAccess></section>
}
