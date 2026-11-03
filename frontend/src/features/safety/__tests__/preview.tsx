import {useState} from 'react'
import {createRoot} from 'react-dom/client'
import {createSessionController} from '../../auth/model/session'
import {isolatedCookiePolicy} from '../../auth/__tests__/fixtures/data'
import {createSafetyController} from '../controller'
import {ReportDialog,BlocksPanel,BlockUserButton} from '../panels'
import {createAccountController,type AccountDeletionPort} from '../../account/controller'
import {AccountPanel} from '../../account/panel'
import {deletionTargets} from '../../account/contract'
import {ids,link} from './fixtures'
const auth=createSessionController({eventId:ids.event,cookiePolicy:isolatedCookiePolicy()})
auth.session.replace(0,{userId:ids.actor,eventId:ids.event,accessToken:'SYNTHETIC_ONLY'})
let mode:'pending'|'failure'|'complete'|'slow'='pending',release:(()=>void)|null=null
let calls=0
const date='2026-11-04T00:59:00Z'
function saga(){const complete=mode==='complete';return{id:ids.receipt,userId:ids.actor,status:complete?'COMPLETED':mode==='failure'?'DEAD_LETTERED':'IN_PROGRESS',retentionUntil:'2027-11-04T00:59:00Z',createdAt:date,updatedAt:date,completedAt:complete?date:null,steps:deletionTargets.map(target=>({target,status:complete?'SUCCEEDED':mode==='failure'?'DEAD_LETTERED':'PENDING',attemptCount:1,nextAttemptAt:date,lastError:mode==='failure'?'SECRET synthetic provider':null,completedAt:complete?date:null}))}}
const deletion:AccountDeletionPort={async request(scope){calls++;if(mode==='slow')await new Promise<void>(yes=>{release=yes});return{scope,status:202,data:{userId:ids.actor,deleted:true,sagaId:ids.receipt,saga:saga()}}},async status(scope){return{scope,status:200,data:saga()}}}
const safety=createSafetyController({auth,blocksAvailable:true,reportsAvailable:true,authorization:{async check(target,evidenceMediaIds,scope){return{scope,target,evidenceMediaIds,expiresAt:Date.now()+10000,allowed:!new URLSearchParams(location.search).has('denied')}}}})
const account=createAccountController({auth,now:()=>Date.parse('2026-11-04T01:00:00Z'),...(new URLSearchParams(location.search).has('unavailable')?{}:{deletion})})
function App(){const [route,setRoute]=useState('home'),[report,setReport]=useState(false)
 return <><main className="mcr-safety-panel"><p>Synthetic FE36 qualification only</p><nav aria-label="Fixture routes"><button onClick={()=>setReport(true)}>Open report</button><button onClick={()=>setRoute('blocks')}>Blocked users</button><button onClick={()=>setRoute('account')}>Account</button></nav>
 {route==='home'&&<BlockUserButton controller={safety} userId={ids.target}/>}
 {route==='blocks'&&<BlocksPanel controller={safety} onBack={()=>setRoute('home')}/>}
 {route==='account'&&<AccountPanel controller={account} auth={auth} onBack={()=>setRoute('home')}/>}
 </main>{report&&<ReportDialog controller={safety} search={link} evidenceChoices={[{id:ids.media,label:'My ready report attachment'}]} onDismiss={()=>setReport(false)}/>}</>
}
declare global{interface Window{__FE36__:{mode:(value:typeof mode)=>void;release:()=>void;changeIdentity:()=>void;revoke:()=>void;deletionCalls:()=>number}}}
window.__FE36__={mode:value=>{mode=value},release:()=>release?.(),changeIdentity:()=>{auth.session.replace(auth.session.read().generation,{userId:ids.target,eventId:ids.event,accessToken:'NEXT_SYNTHETIC'})},revoke:()=>{auth.privacy.changePolicy(auth.privacy.capture(),'block')},deletionCalls:()=>calls}
createRoot(document.getElementById('root')!).render(<App/> )
