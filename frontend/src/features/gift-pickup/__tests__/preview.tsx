import{createRoot}from'react-dom/client'
import{GiftPickup}from'../ui/GiftPickup'
import{createSessionIntentJournal}from'../model/journal'
import{harness,gift,reservation,meeting,now,event,recipient,owner,reservationId,giftId,receiptId}from'./harness'
import{pendingReceipt}from'../../party-discovery/__tests__/fixtures/data'
const params=new URLSearchParams(location.search),isOwner=params.has('owner'),ready=params.has('ready')||isOwner,mode=ready?'ready':'reserve'
// Synthetic server fixture state survives reload only to qualify the browser's recovery behavior.
if(sessionStorage.getItem('__FE26_FIXTURE_MODE')!==mode){sessionStorage.setItem('__FE26_FIXTURE_MODE',mode);sessionStorage.setItem('__FE26_FIXTURE_STAGE',ready?'ready':'none')}
const stage=()=>sessionStorage.getItem('__FE26_FIXTURE_STAGE'),row=()=>reservation({state:stage()==='claimed'?'CLAIMED':ready?'READY':'HELD',revision:stage()==='claimed'?3:stage()==='issued'?2:1,claim_receipt_id:stage()==='claimed'?receiptId:null})
const h=harness({owner:isOwner,reserveOnly:!ready,journal:createSessionIntentJournal(),handler:r=>{const p=new URL(r.url).pathname
 if(r.method!=='GET'){sessionStorage.setItem('__FE26_FIXTURE_WRITES',String(Number(sessionStorage.getItem('__FE26_FIXTURE_WRITES')??0)+1));sessionStorage.setItem('__FE26_FIXTURE_KEY',r.headers.get('Idempotency-Key')!)
  if(p.endsWith('/pickup-code')){sessionStorage.setItem('__FE26_FIXTURE_STAGE','issued');return Response.json({reservation_id:reservationId,confirmation_code:'654321',expires_at:new Date(now+120000).toISOString(),reservation_revision:2})}
  if(p.endsWith('/claim')){sessionStorage.setItem('__FE26_FIXTURE_STAGE','claimed');return Response.json({receipt_id:receiptId,reservation_id:reservationId,giveaway_id:giftId,state:'CLAIMED',revision:3,claimed_at:new Date(now).toISOString()})}
  sessionStorage.setItem('__FE26_FIXTURE_STAGE','held');throw new Error('fixture lost response')
 }
 if(p.endsWith('/commands'))return Response.json(pendingReceipt(sessionStorage.getItem('__FE26_FIXTURE_KEY')!,{event_id:event,actor_id:isOwner?owner:recipient,operation_id:'reserveConcertGiveaway',state:'SUCCEEDED',resource_id:reservationId,resource_revision:1,result_status:200}))
 return Response.json(p.endsWith(giftId)?gift():p.endsWith('/pickup')?{reservation:row(),meeting:meeting()}:row())}})
// Production key behavior in this native fixture, not the deterministic per-test key generator.
h.config.newKey=()=>crypto.randomUUID()
Object.assign(window,{__FE26__:{read:h.controller.read,writes:()=>Number(sessionStorage.getItem('__FE26_FIXTURE_WRITES')??0),journal:()=>Object.entries(sessionStorage).filter(([k])=>k.startsWith('mcr:pickup-intent:')),logout:()=>h.auth.session.replace(h.auth.session.read().generation,null)}})
createRoot(document.getElementById('root')!).render(<GiftPickup controller={h.controller}/> )
