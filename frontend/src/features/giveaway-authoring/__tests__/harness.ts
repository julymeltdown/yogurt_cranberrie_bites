import fixture from '../../../../../contracts/fixtures/domain/giveaway-management.json'
import {CAPABILITIES,projectCapabilities} from '../../../app/capabilities'
import {createSessionController} from '../../auth/model/session'
import type {GiftContext} from '../../gift-discovery/model/controller'
import {createGiveawayAuthoring,type GiveawayAuthoringOptions} from '../model/controller'
import type {GiftDraft,Management,QualifiedImagePort} from '../model/contract'
export const management=():Management=>structuredClone(fixture) as Management
export const event=fixture.giveaway.event_id,actor=fixture.giveaway.owner.user_id,giftId=fixture.giveaway.giveaway_id,place=fixture.giveaway.public_place_id
export const key='00000000-0000-4000-8000-000000000091'
export const draft=():GiftDraft=>({title:'Encore postcards',description:'Made to share.',quantity_total:10,pickup_starts_at:'2026-11-07T08:00:00Z',pickup_ends_at:'2026-11-07T10:00:00Z',public_place_id:place,state:'DRAFT'})
export function harness(handler?:(r:Request)=>Response|Promise<Response>,editing=false,image?:QualifiedImagePort){
 const requests:Request[]=[],listeners=new Set<()=>void>();const auth=createSessionController({eventId:event,cookiePolicy:{available:true,onInvalidate:()=>()=>{},run:(_k,w)=>w(),dispose(){}},fetch:async input=>{const r=input as Request;requests.push(r.clone());if(handler)return handler(r);const m=management();return Response.json(r.method==='GET'?m:{...m.giveaway,state:'DRAFT'})}})
 auth.session.replace(0,{userId:actor,eventId:event,accessToken:'fixture-token'})
 let context:GiftContext|null={scope:auth.privacy.capture(),eventPhase:'OPEN',capabilities:projectCapabilities(Object.fromEntries(CAPABILITIES.map(k=>[k,{available:true,reason:null}])))}
 const options:GiveawayAuthoringOptions={auth,context:{read:()=>context,subscribe(f){listeners.add(f);return()=>{listeners.delete(f)}}},places:()=>[{id:place,label:'Qualified fixture place'}],now:()=>Date.parse('2026-11-04T00:00:00Z'),newKey:()=>key,...editing?{giveawayId:giftId}:{},...image?{image}:{}}
 const controller=createGiveawayAuthoring(options)
 return{auth,controller,requests,options,publish(c:GiftContext|null){context=c;Array.from(listeners).forEach(f=>f())},dispose(){controller.dispose();auth.dispose()}}
}
