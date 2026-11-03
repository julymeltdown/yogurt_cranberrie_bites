import {createRoot} from 'react-dom/client'
import {PartyManagement,PartyRally} from '../ui/PartyManagement'
import {harness,response,ids,party,profile,errorResponse} from './harness'
let full=false,revision=1,mode='ok'
const h=harness(r=>{const p=new URL(r.url).pathname
 if(r.method!=='GET'){if(mode==='lost')throw new Error('lost fixture response');full=true;return errorResponse(409,'VERSION_CONFLICT')}
 if(p.endsWith(ids.party))return Response.json(party({owner:profile(ids.actor),member_count:full?5:2,revision:full?4:3,meeting_revision:revision}))
 return response(r)})
Object.assign(window,{__FE23__:{mode(v:string){mode=v},moved:async()=>{revision=2;await h.controller.load()},read:h.controller.read,requests:()=>h.requests.map(r=>({method:r.method,key:r.headers.get('Idempotency-Key')})),logout:()=>h.auth.session.replace(h.auth.session.read().generation,null)}})
createRoot(document.getElementById('root')!).render(<><PartyManagement controller={h.controller}/><PartyRally controller={h.controller}/></>)
