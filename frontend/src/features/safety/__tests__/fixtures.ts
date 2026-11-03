import {createSessionController} from '../../auth/model/session'
import {isolatedCookiePolicy} from '../../auth/__tests__/fixtures/data'
import {createSafetyController,type SafetyOptions} from '../controller'
export const ids={actor:'11111111-1111-4111-8111-111111111111',event:'22222222-2222-4222-8222-222222222222',target:'33333333-3333-4333-8333-333333333333',key:'44444444-4444-4444-8444-444444444444',report:'55555555-5555-4555-8555-555555555555',receipt:'66666666-6666-4666-8666-666666666666',media:'77777777-7777-4777-8777-777777777777'}
export const now=Date.parse('2026-11-04T01:00:00Z')
export const target={target_type:'POST' as const,target_id:ids.target}
export const link=`?targetType=POST&targetId=${ids.target}`
export const report=()=>({report_id:ids.report,event_id:ids.event,...target,state:'OPEN' as const,revision:1,created_at:'2026-11-04T00:59:00Z'})
export const receipt=(patch:Record<string,unknown>={})=>({receipt_id:ids.receipt,event_id:ids.event,actor_id:ids.actor,operation_id:'createConcertReport',idempotency_key:ids.key,state:'PENDING',resource_id:null,resource_revision:null,failure_reason:null,created_at:'2026-11-04T00:59:00Z',expires_at:'2026-11-04T02:00:00Z',result_status:null,...patch})
export function deferred<T>(){let resolve!:(value:T)=>void;const promise=new Promise<T>(r=>{resolve=r});return{promise,resolve}}
export function harness(handle:(r:Request)=>Response|Promise<Response>=()=>Response.json(report()),options:Partial<Omit<SafetyOptions,'auth'>>={}){
 const requests:Request[]=[],auth=createSessionController({eventId:ids.event,cookiePolicy:isolatedCookiePolicy(),fetch:async input=>{const r=input as Request;requests.push(r.clone());return handle(r)}})
 auth.session.replace(0,{userId:ids.actor,eventId:ids.event,accessToken:'SYNTHETIC_ONLY'})
 const controller=createSafetyController({auth,blocksAvailable:true,reportsAvailable:true,now:()=>now,newKey:()=>ids.key,authorization:{async check(target,evidenceMediaIds,scope){return{scope,target,evidenceMediaIds,expiresAt:now+10000,allowed:true}}},...options})
 return{auth,controller,requests,dispose(){controller.dispose();auth.dispose()}}
}
