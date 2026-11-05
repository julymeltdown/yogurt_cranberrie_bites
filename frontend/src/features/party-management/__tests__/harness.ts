import { CAPABILITIES,projectCapabilities } from '../../../app/capabilities'
import {createSessionController} from '../../auth/model/session'
import {admission} from '../../auth/__tests__/enrollment-consent/fixtures'
import type {PartyAccess} from '../../party-discovery/model/controller'
import {createPartyManagement,type MeetingAuthority} from '../model/controller'
import {ids,party,profile,application,pendingReceipt,errorResponse} from '../../party-discovery/__tests__/fixtures/data'
export {ids,party,profile,application,pendingReceipt,errorResponse}
export const member=(user_id=ids.actor)=>({event_id:ids.event,party_id:ids.party,user_id,role:user_id===ids.actor?'OWNER' as const:'MEMBER' as const,state:'ACTIVE' as const,revision:1,joined_at:'2026-11-01T00:00:00Z'})
export function harness(handle?:(r:Request)=>Response|Promise<Response>){
 const requests:Request[]=[],admissionRequests:Request[]=[],listeners=new Set<()=>void>(),authorityListeners=new Set<()=>void>()
 const auth=createSessionController({eventId:ids.event,cookiePolicy:{available:true,onInvalidate:()=>()=>{},run:(_k,w)=>w(),dispose(){}},fetch:async input=>{const r=input as Request;if(r.method==='GET'&&new URL(r.url).pathname==='/api/auth/enrollment/admission'){admissionRequests.push(r.clone());return Response.json(admission())}requests.push(r.clone());return handle?handle(r):response(r)}})
 auth.session.replace(0,{userId:ids.actor,eventId:ids.event,accessToken:'fixture-token'})
 let access:PartyAccess|null={scope:auth.privacy.capture(),phase:'OPEN',capabilities:projectCapabilities(Object.fromEntries(CAPABILITIES.map(k=>[k,{available:true,reason:null}])))}
 let authority:MeetingAuthority|null={scope:auth.privacy.capture(),partyId:ids.party,meetingRevision:1,expiresAt:Date.now()+60000,description:'Qualified physical fixture meeting. No production venue authority.'},key=80
 const controller=createPartyManagement({auth,partyId:ids.party,access:{read:()=>access,subscribe(f){listeners.add(f);return()=>{listeners.delete(f)}}},authority:{read:()=>authority,subscribe(f){authorityListeners.add(f);return()=>{authorityListeners.delete(f)}}},newKey:()=>`00000000-0000-4000-8000-${String(++key).padStart(12,'0')}`})
 return{auth,controller,requests,admissionRequests,updateAccess(a:PartyAccess|null){access=a;Array.from(listeners).forEach(f=>f())},updateAuthority(a:MeetingAuthority|null){authority=a;Array.from(authorityListeners).forEach(f=>f())},dispose(){controller.dispose();auth.dispose()}}
}
export function response(r:Request):Response{const path=new URL(r.url).pathname;return Response.json(path.endsWith('/members')?{items:[member(),member(ids.other)],next_cursor:null}:path.endsWith('/applications')?{items:[application({applicant:profile(ids.other)})],next_cursor:null}:path.endsWith('/rally')?{party_id:ids.party,event_id:ids.event,meeting_revision:1,beacon_id:null,entries:[]}:party({owner:profile(ids.actor)}))}
