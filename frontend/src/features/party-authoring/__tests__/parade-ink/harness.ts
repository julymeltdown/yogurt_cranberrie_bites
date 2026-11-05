import { admission } from '../../../auth/__tests__/enrollment-consent/fixtures'
// TEST_ONLY PI05 admission adaptation of frontend/src/features/party-authoring/__tests__/fixtures/harness.ts SHA256 36e2406a21237f533adc5f51d2bd0f3b095bafc18c7b9e1292bb72b1ae5807b9.
import { CAPABILITIES, projectCapabilities } from '../../../../app/capabilities'
import { createSessionController } from '../../../auth/model/session'
import { createPartyAuthoring } from '../../model/controller'
import type { PartyAccess } from '../../../party-discovery/model/controller'
import { ids, party, profile } from '../../../party-discovery/__tests__/fixtures/data'
export { ids, party, profile, errorResponse, pendingReceipt } from '../../../party-discovery/__tests__/fixtures/data'
export const draft = () => ({ title:'Encore friends',description:'Share a plan.',ticket_section:'A',capacity:5,meeting_starts_at:'2026-11-07T08:00:00Z',meeting_ends_at:'2026-11-07T09:00:00Z',requirements:'Be considerate',public_place_id:ids.place,state:'OPEN' as const })
export function harness(handle?: (r:Request)=>Response|Promise<Response>, editing=false) {
  const requests:Request[]=[], listeners=new Set<()=>void>()
  const auth=createSessionController({eventId:ids.event,cookiePolicy:{available:true,onInvalidate:()=>()=>{},run:(_k,w)=>w(),dispose(){}},fetch:async input=>{const r=input as Request;if(r.method==='GET'&&new URL(r.url).pathname==='/api/auth/enrollment/admission')return Response.json(admission());requests.push(r.clone());return handle ? handle(r):Response.json(party({owner:profile(ids.actor)}))}})
  auth.session.replace(0,{userId:ids.actor,eventId:ids.event,accessToken:'fixture-token'})
  let access:PartyAccess|null={scope:auth.privacy.capture(),phase:'OPEN',capabilities:projectCapabilities(Object.fromEntries(CAPABILITIES.map(k=>[k,{available:true,reason:null}])))}
  let key=50
  const controller=createPartyAuthoring({auth,access:{read:()=>access,subscribe(f){listeners.add(f);return()=>{listeners.delete(f)}}},places:()=>[{id:ids.place,label:'Qualified test place · fixture only'}],...(editing?{partyId:ids.party}:{}),newKey:()=>`00000000-0000-4000-8000-${String(++key).padStart(12,'0')}`,now:()=>Date.parse('2026-11-04T00:00:00Z')})
  return {auth,controller,requests,update(a:PartyAccess|null){access=a;[...listeners].forEach(f=>f())},dispose(){controller.dispose();auth.dispose()}}
}
