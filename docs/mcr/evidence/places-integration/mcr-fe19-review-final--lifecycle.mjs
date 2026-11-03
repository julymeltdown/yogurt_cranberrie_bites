import {createRequire} from 'node:module'
import {writeFile} from 'node:fs/promises'
import assert from 'node:assert/strict'
const root='<worktrees>/fe-places-review/frontend', require=createRequire(root+'/package.json')
const {createServer}=await import(require.resolve('vite'))
const server=await createServer({root,logLevel:'error',server:{middlewareMode:true}})
globalThis.location={origin:'https://mcr.example.invalid'}
const results=[]
try {
 const {createPlacesController}=await server.ssrLoadModule('/src/features/places/model/controller.ts')
 const {createMemorySession,createHttpClient}=await server.ssrLoadModule('/src/shared/api/http.ts')
 const f=await server.ssrLoadModule('/src/features/places/__tests__/fixtures/data.ts')
 const wait=async predicate=>{for(let i=0;i<200;i++){if(predicate())return;await new Promise(r=>setTimeout(r,5))}throw Error('probe wait expired')}
 for(const status of [404,410,409]) {
  const session=createMemorySession(); session.replace(0,{userId:f.userId,eventId:f.eventId,accessToken:'fixture-token'})
  let release=await f.release(), reject=true, requests=0
  const target=f.places()[1].place_id, other=f.places()[0].place_id, listeners=new Set(), calls=[]
  const http=createHttpClient({session,fetch:async request=>{
   requests++
   if(new URL(request.url).pathname.endsWith('/places'))return Response.json(f.catalog())
   return reject?Response.json({code:'SYNTHETIC',message:'private',details:{reason:status===409?'MAP_VERSION_MISMATCH':'RESOURCE_UNAVAILABLE'}},{status}):Response.json({event_id:f.eventId,revision:7,place:f.places()[1]})
  }})
  const c=createPlacesController({session,http,map:{read:()=>release,subscribe:l=>{listeners.add(l);return()=>listeners.delete(l)}},camera:{read:()=>({lookAt:p=>calls.push(p),recenter:()=>calls.push('recenter'),setZoom:z=>calls.push(z),read:()=>({camera:{zoom:1}})})}})
  c.start();await wait(()=>c.read().list.phase==='ready');c.select(target);await wait(()=>c.read().detail.phase==='error');c.select(null)
  assert.equal(c.canShow(target),false);assert.equal(c.showOnMap(target),false)
  await c.refresh();assert.equal(c.canShow(target),false)
  if(status!==409){assert.equal(c.canShow(other),true);assert.equal(c.cameraAvailable(),true);assert.deepEqual(c.read().unavailablePlaceIds,[target])}
  else {assert.equal(c.canShow(other),false);assert.equal(c.cameraAvailable(),false);assert.equal(c.recenter(),false);assert.equal(c.zoom(1),false);assert.equal(c.read().mapMismatch,true)}
  assert.deepEqual(calls,[])
  reject=false
  if(status===409){
   const wrong=f.places();wrong[0].label='Changed association'
   release=await f.release(wrong,2);listeners.forEach(l=>l());await wait(()=>c.read().list.phase==='ready');assert.equal(c.read().mapMismatch,true)
   release=await f.release(f.places(),2);listeners.forEach(l=>l());await wait(()=>c.read().list.phase==='ready');assert.equal(c.read().mapMismatch,false);assert.equal(c.canShow(target),true)
  }else{c.select(target);await wait(()=>c.read().detail.phase==='ready');c.select(null);assert.equal(c.canShow(target),true);assert.deepEqual(c.read().unavailablePlaceIds,[])}
  // Reinstall rejection, then prove invalidation does not transfer to a new canonical owner.
  reject=true;c.select(target);await wait(()=>c.read().detail.phase==='error');c.select(null)
  reject=false;session.replace(session.read().generation,{userId:f.otherUser,eventId:f.eventId,accessToken:'new-owner-token'})
  assert.equal(c.read().list.data,null);assert.deepEqual(c.read().unavailablePlaceIds,[])
  await wait(()=>c.read().list.phase==='ready');assert.equal(c.read().mapMismatch,true);assert.equal(c.canShow(target),false)
  release={...await f.release(),scope:session.read()};listeners.forEach(l=>l());await wait(()=>c.read().list.phase==='ready')
  assert.equal(c.canShow(target),true);assert.equal(c.read().mapMismatch,false)
  c.dispose();assert.equal(c.read().list.data,null);assert.deepEqual(c.read().unavailablePlaceIds,[]);assert.equal(listeners.size,0)
  const stopped=requests;await c.refresh();c.select(target);assert.equal(requests,stopped);assert.equal(c.showOnMap(target),false)
  results.push({status,result:'PASS',backAndRetry:'PASS',unrelatedTarget:status===409?'release-wide refusal':'available',reconciliation:'PASS',ownerChange:'PASS',disposal:'PASS'})
 }
 console.log(JSON.stringify(results,null,2));await writeFile('<tmp>/mcr-fe19-review-final/lifecycle.json',JSON.stringify(results,null,2)+'\n')
}finally{await server.close()}
