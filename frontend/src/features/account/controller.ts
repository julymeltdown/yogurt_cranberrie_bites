import type { SessionController } from '../auth/model/session'
import type { PrivacyScope } from '../../shared/query/privacy'
import { freeze,sameScope } from '../safety/contract'
import { decodeDeletion,type DeletionBinding,type DeletionProgress } from './contract'
/** Implemented by canonical Auth owner only. No feature fetch, token, cookie or alternate session.
 * Must enforce inherited deleteAccount/getAccountDeletionStatus, browser intent, no-store,
 * original actor/generation and no mutation replay. Status alone may operate in suspended scope.
 */
export interface AccountDeletionPort {
 request(scope:PrivacyScope,signal:AbortSignal):Promise<{readonly scope:PrivacyScope;readonly status:202;readonly data:unknown}>
 status(scope:PrivacyScope,signal:AbortSignal):Promise<{readonly scope:PrivacyScope;readonly status:200;readonly data:unknown}>
}
export interface AccountOptions {auth:SessionController;deletion?:AccountDeletionPort;now?:()=>number;onLogout?:(confirmed:boolean)=>void;onDeletionRestricted?:()=>void;onDeletionComplete?:()=>void}
export interface AccountState {readonly phase:'ready'|'requesting'|'unknown'|'checking'|'pending'|'partial-failure'|'complete'|'closed';readonly progress:DeletionProgress|null;readonly message:string;readonly confirmation:string}
export function createAccountController(options:AccountOptions){
 const {auth}=options,now=options.now??Date.now
 let scope=auth.privacy.capture(),state:AccountState=freeze({phase:'ready',progress:null,message:'',confirmation:''}),disposed=false,flight:AbortController|null=null,sequence=0,submitted=false,restrictionNotified=false
 let binding:DeletionBinding={actorId:scope.userId??'',sagaId:null,updatedAt:null}
 const listeners=new Set<()=>void>()
 const publish=(patch:Partial<AccountState>)=>{state=freeze({...state,...patch});for(const f of Array.from(listeners)){try{f()}catch{/* Isolate subscribers. */}}}
 const current=()=>!disposed&&scope.userId!==null&&sameScope(scope,auth.privacy.capture())&&auth.session.isCurrent(scope)&&['open','suspended'].includes(scope.gate)
 const cleanup=()=>{++sequence;flight?.abort();flight=null;binding={actorId:'',sagaId:null,updatedAt:null};publish({phase:'closed',confirmation:'',progress:null,message:'Private account views were cleared.'})}
 const unsubscribe=auth.privacy.registerCleanup(cleanup)
 function restrictPrivateViews(){
  // Restriction is intentional; no ordinary response can renew a revoked lease.
  const previous=scope
  if(scope.gate==='open'){
   auth.privacy.changePolicy(scope,'suspended')
   const next=auth.privacy.capture()
   if(disposed||!auth.session.isCurrent(previous)||next.gate!=='suspended'||next.policyGeneration!==previous.policyGeneration+1)return false
   scope=next
  }
  return current()
 }
 function notifyRestriction(){
  if(!restrictionNotified&&current()){restrictionNotified=true;try{options.onDeletionRestricted?.()}catch{/* Restriction is already committed. */}}
 }
 function accept(value:ReturnType<typeof decodeDeletion>){
  if(!restrictPrivateViews())return
  binding=value.binding
  if(value.progress.state==='complete'){
   // Completion revokes the canonical identity too; do not retain deleted-account credentials.
   void auth.logout()
   const anonymous=auth.session.read()
   if(disposed||anonymous.userId!==null||!auth.session.isCurrent(anonymous))return
   publish({phase:'complete',progress:value.progress,confirmation:'',message:'Account deletion confirmed by the Auth owner.'})
   if(auth.session.isCurrent(anonymous)){try{options.onDeletionComplete?.()}catch{/* Completion is already verified. */}}
   return
  }
  publish({phase:value.progress.state,progress:value.progress,confirmation:'',message:value.progress.state==='partial-failure'?'Deletion needs recovery. It has not completed. Check progress or contact support.':'Deletion is pending. Private views are closed; completion is not confirmed.'})
  notifyRestriction()
 }
 async function run(kind:'request'|'status'){
  if(!current()||!options.deletion||flight||kind==='request'&&(submitted||state.confirmation!=='DELETE_MY_ACCOUNT'||scope.gate!=='open')||kind==='status'&&state.phase==='complete')return
  if(kind==='request')submitted=true
  const expected=scope,bound={...binding,actorId:scope.userId!},ticket=++sequence,controller=new AbortController();flight=controller
  publish({phase:kind==='request'?'requesting':'checking',message:kind==='request'?'Requesting deletion once…':'Checking deletion progress…',confirmation:''})
  try{
   if(!current()||ticket!==sequence||controller.signal.aborted)return
   const result=await options.deletion[kind](expected,controller.signal)
   if(!current()||ticket!==sequence||controller.signal.aborted)return
   if(!sameScope(result.scope,expected)||result.status!==(kind==='request'?202:200))throw new Error('Unbound deletion result')
   const value=decodeDeletion(result.data,result.status,bound,now())
   if(!current()||ticket!==sequence)return
   accept(value)
  }catch{if(current()&&ticket===sequence){
   // An uncertain DELETE may have executed: close private views even without completion proof.
   if(kind==='request'&&!restrictPrivateViews())return
   binding=bound
   publish({phase:'unknown',progress:null,message:'Deletion is not confirmed. Check progress; no deletion request will be replayed.'})
   if(kind==='request')notifyRestriction()
  }}
  finally{if(flight===controller)flight=null}
 }
 return {read:()=>state,subscribe(f:()=>void){listeners.add(f);return()=>{listeners.delete(f)}},deletionAvailable:Boolean(options.deletion),canRequestDeletion:()=>Boolean(options.deletion)&&current()&&scope.gate==='open'&&!submitted&&state.phase==='ready',
  editConfirmation(value:string){if(current()&&!submitted&&state.phase==='ready')publish({confirmation:value})},
  requestDeletion:()=>run('request'),checkDeletion:()=>run('status'),
  async logout(){
   if(!current())return
   const pending=auth.logout(),anonymous=auth.session.read()
   const confirmed=await pending
   if(disposed||anonymous.userId!==null||!auth.session.isCurrent(anonymous))return
   if(auth.read().logout!==(confirmed?'confirmed':'unconfirmed'))return
   try{options.onLogout?.(confirmed)}catch{/* Canonical revocation already completed. */}
  },
  dispose(){if(disposed)return;cleanup();disposed=true;unsubscribe();listeners.clear()},
 }
}
export type AccountController=ReturnType<typeof createAccountController>
