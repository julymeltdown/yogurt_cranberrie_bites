// PI12 COMPOSITE_SOURCE_FIXTURE derived from PI01 927cf0d01838ec9e937f2aea02b618fb8d34d80d.
// Canonical Auth/default ports/real router/Phaser; controlled HTTP/C05/service inputs, never live proof.
import { servicePolicyFixture } from '../../pre-world-policy/__tests__/fixtures'
import { admission } from '../../../features/auth/__tests__/enrollment-consent/fixtures'
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
import { MockWorldClock, MockSocket } from '../../../world/network/mock'
import { eventId, rooms } from '../../world-entry/__tests__/data'
import { snapshot, ticket } from '../../../world/composition/__tests__/fixtures/data'
import { response as socialResponse } from '../../social-composition/__tests__/fixtures'
import { qualifiedData } from '../default-shell/qualified-data'
import type { WorldTicketRequest } from '../../../world/network/WorldTransport'
import { createConnectionObserver, type ServiceDecision, type ValuePort, type RecoveryAction } from '../../errors/observer'
import { createDefaultConnectionStatus } from '../../errors/default-entry'
import { post, id } from '../../social-composition/__tests__/fixtures'
import '../../../shared/styles/base.css'
import '../../shell.css'
const params=new URLSearchParams(location.search),socialEnabled=true
const reportMode=params.get('reportMode')??'ready'
let reportResult: Record<string,unknown>|null=null
let reportKey:string|null=null, holdSockets=false
function valuePort<T>(initial:T) {
 let value=initial;const listeners=new Set<()=>void>()
 return {read:()=>value,subscribe(fn:()=>void){listeners.add(fn);return()=>{listeners.delete(fn)}},set(next:T){value=next;for(const fn of listeners)fn()}}
}
let socialFactories=0
const data=await qualifiedData(),requests:{path:string;method:string;key:string|null}[]=[],sockets:MockSocket[]=[],gates=new Set<()=>void>()
function receipt(state:'PENDING'|'FAILED'){return {receipt_id:id(502),event_id:eventId,actor_id:data.bootstrap.participant.user_id,operation_id:'createConcertReport',idempotency_key:reportKey,state,resource_id:null,resource_revision:null,failure_reason:state==='FAILED'?'RESOURCE_UNAVAILABLE':null,created_at:new Date(Date.now()-1000).toISOString(),expires_at:new Date(Date.now()+3600000).toISOString(),result_status:null}}
let acceptedPolicy:number|null=null
let mode='valid',writes=0,serial=0,mounts=0,games=0,peak=0,disposals=0,closes=0,originalCamera:unknown=null,sceneClosed=0
const auth=createSessionController({eventId,cookiePolicy:isolatedCookiePolicy(),fetch:async input=>{
 const r=input as Request,path=new URL(r.url).pathname;requests.push({path,method:r.method,key:r.headers.get('Idempotency-Key')})
 if(path==='/api/auth/capabilities')return Response.json(capabilities())
 if(path==='/api/auth/login')return Response.json(authResponse())
 if(path==='/api/auth/enrollment/admission')return Response.json(admission())
 if(path==='/api/auth/logout')return new Response(null,{status:204})
 // TEST ONLY normal operational input; actual default transport/decoder remain in use.
 if(path===`/api/concert/events/${eventId}/service-policy`)return Response.json(servicePolicyFixture('NORMAL',{ttl:30_000}))
 if(path.endsWith('/bootstrap'))return auth.privacy.capture().policyGeneration===acceptedPolicy?Response.json(data.bootstrap):Response.json({code:'TEST',message:'Policy fixture denies reentry',details:{reason:'RESOURCE_UNAVAILABLE'}},{status:503})
 if(path.endsWith('/rooms'))return Response.json(rooms())
 if(path.endsWith('/world-tickets'))return Response.json(ticket(await r.json() as WorldTicketRequest,++serial))
 if(path.endsWith('/places'))return Response.json({event_id:eventId,revision:1,items:data.bootstrap.map_manifest.places})
 if(path.includes('/places/'))return Response.json({event_id:eventId,revision:1,place:data.bootstrap.map_manifest.places.find(p=>p.place_id===path.split('/').at(-1))})
 if(path.endsWith('/me/blocks'))return Response.json({items:[],next_cursor:null})
 if(path.endsWith('/me/reports')){
  if(reportMode==='unavailable'||reportMode==='permission')return Response.json({code:'RESOURCE_UNAVAILABLE',message:'PRIVATE_PAYLOAD_MUST_NOT_RENDER',details:{reason:'RESOURCE_UNAVAILABLE'}},{status:reportMode==='permission'?403:503})
  return Response.json({items:reportResult?[reportResult]:[],next_cursor:null})
 }
 if(path.endsWith('/reports')&&r.method==='POST'){
  reportKey=r.headers.get('Idempotency-Key')
  if(reportMode==='unknown')throw new TypeError('PRIVATE_REPORT_TRANSPORT_DETAIL')
  const body=await r.json() as {target_type:string;target_id:string}
  reportResult={report_id:id(501),event_id:eventId,target_type:body.target_type,target_id:body.target_id,state:'OPEN',revision:1,created_at:new Date().toISOString()}
  if(reportMode==='pending'||reportMode==='failed')return Response.json(receipt('PENDING'),{status:202})
  return Response.json(reportResult)
 }
 if(path.endsWith('/commands'))return Response.json(receipt(reportMode==='failed'?'FAILED':'PENDING'))
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
if(!await auth.checkAdmission())throw new Error('Test canonical admission failed')
acceptedPolicy=auth.privacy.capture().policyGeneration
class Socket extends MockSocket {
 constructor(){super();sockets.push(this);if(!holdSockets)queueMicrotask(()=>this.open())}
 override send(text:string){super.send(text);const message=JSON.parse(text);if(!holdSockets&&(message.type==='authenticate'||message.type==='switchRoom'))queueMicrotask(()=>this.receive(JSON.stringify(snapshot(message.roomId,serial>1?id(1000+serial):undefined,{cause:message.type==='switchRoom'?'ROOM_SWITCH':'INITIAL',requestId:message.requestId??null}))))}
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
}},socialEnabled?{availability(context){socialFactories++;const value={bootstrap:context.bootstrap,scope:context.scope,services:{feed:true,people:true,intro:true,age:true,inbox:true}};return {read:()=>value,subscribe:()=>()=>{}}}}:undefined)
let composition:ReturnType<typeof createWorldComposition>|null=null
const connectivity=valuePort<'online'|'offline'|'unknown'>('online'),service=valuePort<ServiceDecision|null>(null),displayClock=new MockWorldClock()
const recovery:ValuePort<RecoveryAction|null>={read(){const binding=ports.worldAccess?.read();return binding?.reconnect?{release:binding.release,reconnect(){return binding.reconnect!()}}:null},subscribe:()=>()=>{}}
const connection=createConnectionObserver({auth,world:ports.worldAccess!,connectivity,service,recovery,scheduler:displayClock,pendingMs:1000});connection.start()
const defaultConnection=createDefaultConnectionStatus({auth,world:ports.worldAccess!,browser:window,reload(){reloads++}});defaultConnection.start()
const displayConnection=params.get('connectionOwner')==='default'?defaultConnection.controller:connection
const fixturePorts={...ports,connectionStatus:{controller:displayConnection,reload(){reloads++}}}
let reloads=0
function HttpFailureFixture():never {throw new Response('PRIVATE_HTTP_PAYLOAD_MUST_NOT_RENDER',{status:Number(params.get('httpStatus')??503)})}
const router=createMemoryRouter(createAppRoutes(fixturePorts,params.has('httpStatus')?{panels:{'/world/help':<HttpFailureFixture/>}}:undefined),{initialEntries:[new URLSearchParams(location.search).get('path')??'/world']})
const root=createRoot(document.getElementById('root')!);root.render(<RouterProvider router={router}/>)
const fixture={state:()=>{
 const p=composition?.presentation.read();if(p&&!originalCamera)originalCamera=p.camera
 return {qualification:'COMPOSITE_SOURCE_FIXTURE',connection:connection.read(),displayConnection:displayConnection.read(),defaultConnection:defaultConnection.controller.read(),reloads,reportMode,socialFactories,path:router.state.location.pathname,phase:composition?.read().phase,problem:composition?.read().problem,published:!!p,theme:p?.theme.read()??null,camera:p?.camera.read(),sameCamera:p?.camera===originalCamera,mapHash:p?.map.expected.contentHash,geometry:p?.map.manifest.geometry_hash,room:composition?.replication.read()?.baseline?.roomId,outfit:composition?.replication.read()?.entities.find(e=>e.self)?.player.avatar,mounts,games,peak,disposals,closes,sceneClosed,writes,gates:gates.size,sockets:sockets.length,activeSockets:sockets.filter(s=>s.readyState<2).length,requests,canvases:document.querySelectorAll('.shell-world-host canvas').length}
},connection(kind:string){
 const binding=ports.worldAccess?.read();if(!binding)throw new Error('Actual World binding is required')
 if(kind==='offline')connectivity.set('offline')
 else if(kind==='maintenance'||kind==='version'||kind==='unavailable')service.set({release:binding.release,kind})
 else if(kind==='reconnecting'){holdSockets=true;sockets.at(-1)!.serverClose(1006);void binding.reconnect?.()}
 else if(kind==='suspended'){Object.defineProperty(document,'hidden',{configurable:true,value:true});document.dispatchEvent(new Event('visibilitychange'))}
 else if(kind==='resync'||kind==='default-version'){const baseline=composition?.replication.read()?.baseline;if(!baseline)throw new Error('Actual baseline required');sockets.at(-1)!.receive(JSON.stringify({protocolVersion:'1.0.0',type:'resyncRequired',streamId:baseline.streamId,reason:kind==='resync'?'GAP':'MAP_VERSION_MISMATCH'}))}
 else if(kind==='expire')displayClock.advance(1000)
 else throw new Error('Unknown controlled input')
},reportPath:()=>'/report?targetType=POST&targetId='+post.post_id,back:()=>router.navigate(-1),mode(value:string){mode=value},release(){const current=[...gates];gates.clear();current.forEach(resolve=>resolve())},navigate:(path:string)=>router.navigate(path),withdraw(){composition?.dispose()},policy(){auth.privacy.changePolicy(auth.privacy.capture(),'block')},async dispose(){root.unmount();connection.dispose();defaultConnection.dispose();router.dispose();ports.dispose?.();await composition?.settled();auth.dispose()}}
Object.assign(window,{__PI12__:fixture})
