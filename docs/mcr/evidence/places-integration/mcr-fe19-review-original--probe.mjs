import {createRequire} from 'node:module'
import {writeFile} from 'node:fs/promises'
import assert from 'node:assert/strict'
const root='<worktrees>/fe-places-review/frontend'
const require=createRequire(root+'/package.json')
const {createServer}=await import(require.resolve('vite'))
const server=await createServer({root,logLevel:'error',server:{middlewareMode:true}})
globalThis.location={origin:'https://mcr.example.invalid'}
const results=[]
try {
 const {createPlacesController}=await server.ssrLoadModule('/src/features/places/model/controller.ts')
 const {createMemorySession,createHttpClient}=await server.ssrLoadModule('/src/shared/api/http.ts')
 const f=await server.ssrLoadModule('/src/features/places/__tests__/fixtures/data.ts')
 const wait=async predicate=>{for(let i=0;i<100;i++){if(predicate())return;await new Promise(r=>setTimeout(r,5))}throw Error('probe wait expired')}
 for(const [status,reason] of [[404,'RESOURCE_UNAVAILABLE'],[410,'RESOURCE_UNAVAILABLE'],[409,'MAP_VERSION_MISMATCH']]) {
  const session=createMemorySession();session.replace(0,{userId:f.userId,eventId:f.eventId,accessToken:'fixture-token'})
  const release=await f.release(),target=f.places()[1].place_id
  const cameraCalls=[]
  const http=createHttpClient({session,fetch:async request=>new URL(request.url).pathname.endsWith('/places')?Response.json(f.catalog()):Response.json({code:'SYNTHETIC',message:'private',details:{reason}},{status})})
  const c=createPlacesController({session,http,map:{read:()=>release,subscribe:()=>()=>{}},camera:{read:()=>({lookAt:p=>cameraCalls.push(p),recenter:()=>cameraCalls.push('recenter'),setZoom:()=>{},read:()=>({camera:{zoom:1}})})}})
  c.start();await wait(()=>c.read().list.phase==='ready');assert.equal(c.canShow(target),true)
  c.select(target);await wait(()=>c.read().detail.phase==='error')
  const atError={reason:c.read().detail.reason,mapMismatch:c.read().mapMismatch,canShow:c.canShow(target),recenter:c.recenter()}
  c.select(null) // PlaceDetail cleanup / PlacesOverview effect on All places or browser Back.
  const afterBack={listPhase:c.read().list.phase,mapMismatch:c.read().mapMismatch,canShow:c.canShow(target),show:c.showOnMap(target)}
  results.push({status,reason,atError,afterBack,cameraCalls})
  c.dispose()
 }
 console.log(JSON.stringify({candidate:'1ba3f7f6b0bb568524fd88ede22558ad6fd163fc',results},null,2))
 await writeFile('<tmp>/mcr-fe19-review/probe.json',JSON.stringify(results,null,2)+'\n')
} finally {await server.close()}
