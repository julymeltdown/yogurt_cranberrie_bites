import {useEffect,useSyncExternalStore} from 'react'
import type {GiftPickupController} from '../model/controller'
import {kstTime} from '../../party-authoring/model/time'
import './pickup.css'
export function GiftPickup({controller}:{controller:GiftPickupController}){
 const s=useSyncExternalStore(controller.subscribe,controller.read,controller.read)
 useEffect(()=>{void controller.load()},[controller])
 const locked=!controller.retryReady()||!['ready','failed','succeeded','code-unavailable'].includes(s.phase),r=s.reservation,g=s.gift
 if(!s.access||s.phase==='denied')return <section className="gift-pickup"><h1>Pickup unavailable</h1><p>Current reservation or owner access is required.</p></section>
 return <section className="gift-pickup"><header><p>GIFTS / YOUR PICKUP</p><h1>{g?.title??'Reservation and pickup'}</h1><p>A reservation holds an item. Only a server-confirmed collection completes pickup.</p></header>
 <p role="status" aria-live="polite">{s.message}</p>
 <button disabled={['loading','sending','checking'].includes(s.phase)} onClick={()=>void controller.load()}>Read current reservation</button>
 {g&&!r&&<section><h2>Reserve an item</h2><p>{g.available} available · {g.state}</p><p>Pickup window: {kstTime(g.pickup_starts_at)} to {kstTime(g.pickup_ends_at)}.</p><button disabled={locked||!controller.canWrite()||g.available<=0||g.state!=='OPEN'||controller.isOwner()} onClick={()=>void controller.reserve()}>Confirm reservation</button></section>}
 {r&&<section><h2>Reservation status</h2><p>{r.state}{['HELD','READY'].includes(r.state)&&!controller.active()?' · Pickup currently unavailable or expired':''}</p><p>Expiry: {kstTime(r.expires_at)}.</p>
 {controller.isRecipient()&&controller.active()&&<button disabled={locked} onClick={()=>void controller.loadPickup()}>Read private pickup details</button>}
 {controller.active()&&<><label><input type="checkbox" checked={s.cancelConfirmed} disabled={locked} onChange={e=>controller.confirmCancel(e.target.checked)}/>Cancel this reservation and release the held item.</label><button disabled={locked||!s.cancelConfirmed} onClick={()=>void controller.cancel()}>Confirm reservation cancellation</button></>}
 </section>}
 {s.pickup&&controller.physicalReady()&&<section aria-label="Authorized pickup details"><h2>Current pickup plan</h2><p>{s.pickup.meeting.instructions}</p><p>How to identify the giver: {s.pickup.meeting.identification}</p><p>Valid until {kstTime(s.pickup.meeting.expires_at)}.</p><p>Virtual movement does not establish physical arrival.</p></section>}
 {controller.isRecipient()&&r?.state==='READY'&&<section><h2>Confirmation code</h2><p>Request a short-lived code after checking the current pickup plan. A code does not confirm collection.</p><button disabled={locked||!controller.canWrite()||!controller.physicalReady()} onClick={()=>void controller.issueCode()}>{s.code?'Rotate pickup code':'Request pickup code'}</button>{s.code&&<><output aria-label="Pickup confirmation code">{s.code.confirmation_code}</output><p>Code expires {kstTime(s.code.expires_at)}. Share it only at pickup.</p></>}</section>}
 {controller.isOwner()&&r?.state==='READY'&&controller.active()&&<section><h2>Confirm physical collection</h2><label>Recipient’s six-digit code<input aria-label="Recipient’s six-digit code" type="password" inputMode="numeric" autoComplete="off" maxLength={6} value={s.confirmationCode} disabled={locked} onChange={e=>controller.setClaimCode(e.target.value)}/></label><button disabled={locked||!controller.canWrite()||!/^\d{6}$/.test(s.confirmationCode)} onClick={()=>void controller.claim()}>Confirm one-time collection</button></section>}
 {s.receipt&&<section><h2>Collection receipt</h2><p>Claimed at {kstTime(s.receipt.claimed_at)}. The server confirmed one collection.</p></section>}
 {['pending','uncertain'].includes(s.phase)&&<button disabled={!controller.canCheckStatus()} onClick={()=>void controller.checkStatus()}>Check original command</button>}
 {s.phase==='storage-unavailable'&&<p>New writes are disabled until command recovery storage is available.</p>}
 </section>
}
