import { admission } from '../../auth/__tests__/enrollment-consent/fixtures'
import fixture from '../../../../../contracts/fixtures/domain/giveaway-management.json'
import {CAPABILITIES,projectCapabilities} from '../../../app/capabilities'
import {createSessionController} from '../../auth/model/session'
import type {GiftContext} from '../../gift-discovery/model/controller'
import type {Giveaway,Reservation} from '../../gift-discovery/model/contract'
import {createGiftPickup,type GiftPickupOptions,type PickupPlaceAuthority} from '../model/controller'
import {type IntentJournal,type PickupIntent} from '../model/journal'
export const gift=():Giveaway=>structuredClone(fixture.giveaway) as Giveaway
export const reservation=(patch:Partial<Reservation>={}):Reservation=>({...structuredClone(fixture.reservations[0]!) as Reservation,state:'READY',...patch})
export const event=fixture.giveaway.event_id,owner=fixture.giveaway.owner.user_id,recipient=fixture.reservations[0]!.recipient_id,reservationId=fixture.reservations[0]!.reservation_id,giftId=fixture.giveaway.giveaway_id,place=fixture.giveaway.public_place_id,now=Date.parse('2026-11-07T08:05:00Z'),receiptId='00000000-0000-4000-8000-000000000081'
export const meeting=()=>({place_id:place,map_revision:1,place_revision:2,instructions:'Private pickup fixture instructions',identification:'Fixture badge',expires_at:'2026-11-07T09:15:00Z'})
export function memoryJournal(){let stored:PickupIntent|null=null;return{read:(actor:string,event:string)=>stored?.actor===actor&&stored.event===event?structuredClone(stored):null,write(i:PickupIntent){stored=structuredClone(i)},clear(){stored=null},peek:()=>stored} satisfies IntentJournal&{peek():PickupIntent|null}}
export function harness(options:{handler?:(r:Request)=>Response|Promise<Response>;owner?:boolean;reserveOnly?:boolean;journal?:IntentJournal;clock?:()=>number|null}={}){
 const requests:Request[]=[],admissionRequests:Request[]=[],listeners=new Set<()=>void>(),placeListeners=new Set<()=>void>(),actor=options.owner?owner:recipient,journal=options.journal??memoryJournal()
 const auth=createSessionController({eventId:event,cookiePolicy:{available:true,onInvalidate:()=>()=>{},run:(_k,w)=>w(),dispose(){}},fetch:async input=>{const r=input as Request;if(r.method==='GET'&&new URL(r.url).pathname==='/api/auth/enrollment/admission'){admissionRequests.push(r.clone());return Response.json(admission())}requests.push(r.clone());if(options.handler)return options.handler(r);const p=new URL(r.url).pathname;return Response.json(p.endsWith('/pickup')?{reservation:reservation(),meeting:meeting()}:p.endsWith(giftId)?gift():reservation())}})
 auth.session.replace(0,{userId:actor,eventId:event,accessToken:'fixture-token'})
 let context:GiftContext|null={scope:auth.privacy.capture(),eventPhase:'OPEN',capabilities:projectCapabilities(Object.fromEntries(CAPABILITIES.map(k=>[k,{available:true,reason:null}])))},authority:PickupPlaceAuthority|null={scope:auth.privacy.capture(),placeId:place,placeRevision:2,mapRevision:1,expiresAt:now+7200000},key=90
 const config:GiftPickupOptions={auth,context:{read:()=>context,subscribe(f){listeners.add(f);return()=>{listeners.delete(f)}}},giveawayId:giftId,...options.reserveOnly?{}:{reservationId},journal,now:options.clock??(()=>now),newKey:()=>`00000000-0000-4000-8000-${String(++key).padStart(12,'0')}`,placeAuthority:{read:()=>authority,subscribe(f){placeListeners.add(f);return()=>{placeListeners.delete(f)}}}}
 const controller=createGiftPickup(config)
 return{auth,controller,requests,admissionRequests,config,journal,actor,publish(c:GiftContext|null){context=c;Array.from(listeners).forEach(f=>f())},place(a:PickupPlaceAuthority|null){authority=a;Array.from(placeListeners).forEach(f=>f())},dispose(){controller.dispose();auth.dispose()}}
}
