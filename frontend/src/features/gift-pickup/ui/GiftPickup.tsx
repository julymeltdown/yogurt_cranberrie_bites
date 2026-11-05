import {useEffect,useSyncExternalStore,type ReactNode} from 'react'
import type {GiftPickupController} from '../model/controller'
import {kstTime} from '../../party-authoring/model/time'
import '../../gift-discovery/ui/parade-ink/family.css'
import './pickup.css'
export interface PickupImageRequest { readonly giveawayId:string; readonly mediaId:string; readonly alt:string }
export type RenderPickupImage=(request:PickupImageRequest)=>ReactNode
const pickupDate=new Intl.DateTimeFormat('en-GB',{timeZone:'Asia/Seoul',day:'numeric',month:'short'})
const pickupTime=new Intl.DateTimeFormat('en-GB',{timeZone:'Asia/Seoul',hour:'2-digit',minute:'2-digit',hourCycle:'h23'})
function compactWindow(start:string,end:string){const a=new Date(start),b=new Date(end),sameDay=pickupDate.format(a)===pickupDate.format(b);return `${pickupDate.format(a)} ${pickupTime.format(a)}–${sameDay?'':`${pickupDate.format(b)} `}${pickupTime.format(b)} KST`}
export function GiftPickup({controller,renderImage}:{readonly controller:GiftPickupController;readonly renderImage?:RenderPickupImage|undefined}){
 const s=useSyncExternalStore(controller.subscribe,controller.read,controller.read)
 useEffect(()=>{void controller.load()},[controller])
 const locked=!controller.retryReady()||!['ready','failed','succeeded','code-unavailable'].includes(s.phase),r=s.reservation,g=s.gift
 if(!s.access||s.phase==='denied')return <section className="gift-pickup"><h1>Pickup unavailable</h1><p>Current reservation or owner access is required.</p></section>
 const photo=g&&<div className="gift-pickup-photo" key={`${s.epoch}:${g.giveaway_id}:${g.image_media_id}`} aria-label="Giveaway photo">{g.image_media_id&&renderImage?renderImage({giveawayId:g.giveaway_id,mediaId:g.image_media_id,alt:g.title})??<p>Image access unavailable.</p>:<p>{g.image_media_id?'Image access unavailable.':'No photo'}</p>}</div>
 return <section className={`gift-pickup${g&&!r?' gift-pickup--reserve':''}`} data-phase={s.phase}><header className="gift-family-heading"><p>GIVEAWAY / RESERVATION &amp; PICKUP</p><h1>{g?.title??'Reservation and pickup'}</h1><p>One item per reservation. Collection requires server confirmation.</p></header>

 <p role="status" aria-live="polite">{s.message}</p>
 {g&&!r&&<><section className="gift-reserve-card"><div className="gift-reserve-summary"><h2>{g.title}</h2><p className="gift-reserve-stock">{g.available} available · {g.state}</p></div>{photo}<p className="gift-reserve-window" title={`Pickup window: ${kstTime(g.pickup_starts_at)} to ${kstTime(g.pickup_ends_at)}`}>{compactWindow(g.pickup_starts_at,g.pickup_ends_at)}</p><div className="gift-reserve-actions"><div className="gift-reserve-quantity" role="group" aria-label="Reservation quantity"><button type="button" disabled aria-label="Decrease quantity">−</button><output aria-label="Quantity">1</output><button type="button" disabled aria-label="Increase quantity">+</button></div><button className="gift-family-primary" aria-label="Confirm reservation" disabled={locked||!controller.canWrite()||g.available<=0||g.state!=='OPEN'||controller.isOwner()} onClick={()=>void controller.reserve()}>Confirm</button></div></section><p className="gift-family-note">One item per reservation. Quantity changes are unavailable.</p></>}
 <button className="gift-pickup-refresh" disabled={['loading','sending','checking'].includes(s.phase)} onClick={()=>void controller.load()}>Read current reservation</button>
 {r&&<section className="gift-reservation-status"><h2>Reservation status</h2>{photo}<p>{r.state}{['HELD','READY'].includes(r.state)&&!controller.active()?' · Pickup currently unavailable or expired':''}</p><p>Expiry: {kstTime(r.expires_at)}.</p>
 {controller.isRecipient()&&controller.active()&&<button disabled={locked} onClick={()=>void controller.loadPickup()}>Read private pickup details</button>}
 {controller.active()&&<><label className="gift-family-check"><input type="checkbox" checked={s.cancelConfirmed} disabled={locked} onChange={e=>controller.confirmCancel(e.target.checked)}/>Cancel this reservation and release the held item.</label><button disabled={locked||!s.cancelConfirmed} onClick={()=>void controller.cancel()}>Confirm reservation cancellation</button></>}
 </section>}
 {s.pickup&&controller.physicalReady()&&<section aria-label="Authorized pickup details"><h2>Current pickup plan</h2><p>{s.pickup.meeting.instructions}</p><p>How to identify the giver: {s.pickup.meeting.identification}</p><p>Valid until {kstTime(s.pickup.meeting.expires_at)}.</p><p>Virtual movement does not establish physical arrival.</p></section>}
 {controller.isRecipient()&&r?.state==='READY'&&<section><h2>Confirmation code</h2><p>Request a short-lived code after checking the current pickup plan. A code does not confirm collection.</p><button className="gift-family-primary" disabled={locked||!controller.canWrite()||!controller.physicalReady()} onClick={()=>void controller.issueCode()}>{s.code?'Rotate pickup code':'Request pickup code'}</button>{s.code&&<><output aria-label="Pickup confirmation code">{s.code.confirmation_code}</output><p>Code expires {kstTime(s.code.expires_at)}. Share it only at pickup.</p></>}</section>}
 {controller.isOwner()&&r?.state==='READY'&&controller.active()&&<section><h2>Confirm physical collection</h2><label>Recipient’s six-digit code<input aria-label="Recipient’s six-digit code" type="password" inputMode="numeric" autoComplete="off" maxLength={6} value={s.confirmationCode} disabled={locked} onChange={e=>controller.setClaimCode(e.target.value)}/></label><button className="gift-family-primary" disabled={locked||!controller.canWrite()||!/^\d{6}$/.test(s.confirmationCode)} onClick={()=>void controller.claim()}>Confirm one-time collection</button></section>}
 {s.receipt&&<section><h2>Collection receipt</h2><p>Claimed at {kstTime(s.receipt.claimed_at)}. The server confirmed one collection.</p></section>}
 {['pending','uncertain'].includes(s.phase)&&<button disabled={!controller.canCheckStatus()} onClick={()=>void controller.checkStatus()}>Check original command</button>}
 {s.phase==='storage-unavailable'&&<p>New writes are disabled until command recovery storage is available.</p>}
 </section>
}
