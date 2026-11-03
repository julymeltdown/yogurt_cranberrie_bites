import { createRoot } from 'react-dom/client'
import { PartyAuthoring } from '../../ui/PartyAuthoring'
import { harness, ids, party, profile, errorResponse, pendingReceipt } from './harness'
let mode='ok', key=''
const h=harness(r=>{if(r.method==='POST'){key=r.headers.get('Idempotency-Key')!;if(mode==='lost')throw new Error('fixture lost response');if(mode==='invalid')return errorResponse(400,'INVALID_INPUT');return Response.json(party({owner:profile(ids.actor)}))}
  if(new URL(r.url).pathname.endsWith('/commands'))return Response.json(pendingReceipt(key,{operation_id:'createConcertParty',state:'SUCCEEDED',resource_id:ids.party,resource_revision:3,result_status:200}))
  return Response.json(party({owner:profile(ids.actor)}))})
Object.assign(window,{__FE22__:{mode(v:string){mode=v},read:()=>h.controller.read(),requests:()=>h.requests.map(r=>({method:r.method,key:r.headers.get('Idempotency-Key'),query:new URL(r.url).search})),logout:()=>h.auth.session.replace(h.auth.session.read().generation,null)}})
createRoot(document.getElementById('root')!).render(<PartyAuthoring controller={h.controller}/> )
