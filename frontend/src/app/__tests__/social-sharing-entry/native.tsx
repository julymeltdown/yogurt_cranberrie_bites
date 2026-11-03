// TEST ONLY: actual defaults/canonical Auth/Phaser/resolver; deterministic in-process HTTP/C05 and geometric terrain.
import { createRoot } from 'react-dom/client'
import { createMemoryRouter, RouterProvider } from 'react-router-dom'
import { createAppRoutes } from '../../router'
import { createProductionPorts } from '../../ports'
import { createWorldComposition } from '../../../world/composition'
import { createWorldEntryResolver } from '../../world-entry/resolver'
import { createSessionController } from '../../../features/auth/model/session'
import { isolatedCookiePolicy, capabilities, authResponse } from '../../../features/auth/__tests__/fixtures/data'
import { loadPhaserEngine } from '../../../world/scenes/MapScene'
import { createLocalThemePreferences } from '../../../features/theme/model/preferences'
import { MockSocket } from '../../../world/network/mock'
import { eventId, rooms } from '../../world-entry/__tests__/data'
import { snapshot, ticket } from '../../../world/composition/__tests__/fixtures/data'
import { response as socialResponse } from '../../social-composition/__tests__/fixtures'
import { sharingService } from './service'
import { qualifiedData } from '../default-shell/qualified-data'
import type { WorldTicketRequest } from '../../../world/network/WorldTransport'
import '../../../shared/styles/base.css'
import '../../shell.css'
const params=new URLSearchParams(location.search),socialEnabled=true
let sharingReady=params.get('ready')!=='0';const sharingListeners=new Set<()=>void>()
let socialFactories=0
const data=await qualifiedData(),requests:{path:string;method:string;key:string|null}[]=[],sockets:MockSocket[]=[],gates=new Set<()=>void>()
if(params.get('readonly')==='1'){data.bootstrap.event.phase='READ_ONLY';data.bootstrap.capabilities.mutations={available:false,reason:'EVENT_READ_ONLY'}}
const sharing=sharingService(data.bootstrap.participant.user_id,eventId)
let acceptedPolicy:number|null=null
let mode='valid',writes=0,serial=0,mounts=0,games=0,peak=0,disposals=0,closes=0,originalCamera:unknown=null,sceneClosed=0
const auth=createSessionController({eventId,qualifiedRefresh:true,cookiePolicy:isolatedCookiePolicy(),fetch:async input=>{
 const r=input as Request,path=new URL(r.url).pathname;requests.push({path,method:r.method,key:r.headers.get('Idempotency-Key')})
 if(path==='/api/auth/capabilities')return Response.json(capabilities())
 if(path==='/api/auth/login')return Response.json(authResponse())
 if(path==='/api/auth/logout')return new Response(null,{status:204})
 if(path.endsWith('/bootstrap'))return auth.privacy.capture().policyGeneration===acceptedPolicy?Response.json(data.bootstrap):Response.json({code:'TEST',message:'Policy fixture denies reentry',details:{reason:'RESOURCE_UNAVAILABLE'}},{status:503})
 if(path.endsWith('/rooms'))return Response.json(rooms())
 if(path.endsWith('/world-tickets'))return Response.json(ticket(await r.json() as WorldTicketRequest,++serial))
 if(path.endsWith('/places'))return Response.json({event_id:eventId,revision:1,items:data.bootstrap.map_manifest.places})
 if(path.includes('/places/'))return Response.json({event_id:eventId,revision:1,place:data.bootstrap.map_manifest.places.find(p=>p.place_id===path.split('/').at(-1))})
 const sharingResponse=await sharing.respond(r);if(sharingResponse)return sharingResponse
 if(socialEnabled){
  if(mode==='uncertain'&&r.method!=='GET')throw new TypeError('Test lost social response')
  const response=await socialResponse(r)
  const bind=(value:unknown):unknown=>{
   if(Array.isArray(value))return value.map(bind)
   if(value&&typeof value==='object')return Object.fromEntries(Object.entries(value).map(([key,item])=>[key,key==='event_id'?eventId:['user_id','owner_id','actor_id'].includes(key)?data.bootstrap.participant.user_id:bind(item)]))
   return value
  }
  return Response.json(bind(await response.json()),{status:response.status})
 }
 return Response.json({code:'TEST',message:'Unavailable.',details:{reason:'RESOURCE_UNAVAILABLE'}},{status:404})
}})
auth.start();while(auth.read().capabilities==='loading')await new Promise(r=>setTimeout(r,5))
if(!await auth.login('fixture@example.test','fixture-password'))throw new Error('Test login failed')
acceptedPolicy=auth.privacy.capture().policyGeneration
class Socket extends MockSocket {
 constructor(){super();sockets.push(this);queueMicrotask(()=>this.open())}
 override send(text:string){super.send(text);const message=JSON.parse(text);if(message.type==='authenticate'||message.type==='switchRoom')queueMicrotask(()=>this.receive(JSON.stringify(snapshot(message.roomId,undefined,{cause:message.type==='switchRoom'?'ROOM_SWITCH':'INITIAL',requestId:message.requestId??null}))))}
}
Object.defineProperty(globalThis,'WebSocket',{configurable:true,writable:true,value:Socket})
const preferences=createLocalThemePreferences(localStorage)
const ports=createProductionPorts(auth,{localInput:{revision:'test-only',reviewedBy:'test-only',movementPolicy:{speedPxPerSecond:96,footWidthPx:12,footHeightPx:8,interactionRadiusPx:96},zoom:1},load:async options=>{
 const resolve=createWorldEntryResolver({...options,assetFetch:async input=>{
  const url=String(input),path=new URL(url).pathname.slice(1),f=data.releases.find(f=>f.theme.texture.path===path)??data.releases[0]!
  if(mode==='fail'&&path.includes('danger'))throw new Error('Test theme failure')
  if(mode==='pending')await new Promise<void>(resolve=>gates.add(resolve))
  const response=new Response(path.endsWith('.json')?f.tiledBytes:f.png,{headers:{'Content-Type':path.endsWith('.json')?'application/json':'image/png'}})
  Object.defineProperty(response,'url',{value:url});return response
 }})
 const owner=createWorldComposition({auth,resolveRelease:resolve,themePreferences:{read:preferences.read,write(scope,theme){preferences.write(scope,theme);writes++}},importEngine:async()=>{
  const factory=await loadPhaserEngine()
  return(host,map)=>{const engine=factory(host,map);mounts++;games++;peak=Math.max(peak,games);void engine.closed.then(()=>closes++)
   // Exercise only the independently labeled synthetic anchor seam; product Wall remains unavailable.
   void engine.ready.then(()=>{const scene=engine.muralScene?.({x:16,y:16,width:32,height:32,depth:1},'default-test-lifetime');scene?.onClose(()=>sceneClosed++)})
   return {...engine,destroy(){engine.destroy();games--;disposals++}}
  }
 }})
 composition=owner;return owner
}},socialEnabled?{availability(context){socialFactories++;const value={bootstrap:context.bootstrap,scope:context.scope,services:{feed:true,people:true,intro:true,age:true,inbox:true,requests:sharingReady}};return {read:()=>({...value,services:{...value.services,requests:sharingReady}}),subscribe:(fn:()=>void)=>{sharingListeners.add(fn);return()=>{sharingListeners.delete(fn)}}}}}:undefined)
let composition:ReturnType<typeof createWorldComposition>|null=null
const router=createMemoryRouter(createAppRoutes(ports),{initialEntries:['/me',params.get('path')??'/me/social']})
const root=createRoot(document.getElementById('root')!);root.render(<RouterProvider router={router}/>)
const fixture={state:()=>{
 const p=composition?.presentation.read();if(p&&!originalCamera)originalCamera=p.camera
 return {queryEntries:auth.queryClient.getQueryCache().getAll().length,policy:auth.privacy.capture().policyGeneration,socialFactories,path:router.state.location.pathname,phase:composition?.read().phase,published:!!p,theme:p?.theme.read()??null,camera:p?.camera.read(),sameCamera:p?.camera===originalCamera,mapHash:p?.map.expected.contentHash,geometry:p?.map.manifest.geometry_hash,room:composition?.replication.read()?.baseline?.roomId,outfit:composition?.replication.read()?.entities.find(e=>e.self)?.player.avatar,mounts,games,peak,disposals,closes,sceneClosed,writes,gates:gates.size,sockets:sockets.length,activeSockets:sockets.filter(s=>s.readyState<2).length,requests,canvases:document.querySelectorAll('.shell-world-host canvas').length}
},back:()=>router.navigate(-1),mode(value:string){mode=value;sharing.mode(value)},pending(){sharing.pending()},releaseReveal(){sharing.release()},sharingReady(value:boolean){sharingReady=value;sharingListeners.forEach(fn=>fn())},readOnly(){data.bootstrap.event.phase='READ_ONLY';data.bootstrap.capabilities.mutations={available:false,reason:'EVENT_READ_ONLY'};auth.privacy.changePolicy(auth.privacy.capture(),'revalidated');acceptedPolicy=auth.privacy.capture().policyGeneration},identity(){auth.session.replace(auth.session.read().generation,null)},release(){const current=[...gates];gates.clear();current.forEach(resolve=>resolve())},navigate:(path:string)=>router.navigate(path),withdraw(){composition?.dispose()},policy(){auth.privacy.changePolicy(auth.privacy.capture(),'block')},async dispose(){root.unmount();router.dispose();ports.dispose?.();await composition?.settled();auth.dispose()}}
Object.assign(window,{__SHARING_ENTRY__:fixture})
declare global{interface Window{__SHARING_ENTRY__:typeof fixture}}
