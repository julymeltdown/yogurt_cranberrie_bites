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
import { qualifiedData } from './qualified-data'
import type { WorldTicketRequest } from '../../../world/network/WorldTransport'
import '../../../shared/styles/base.css'
import '../../shell.css'
const data=await qualifiedData(),requests:{path:string;method:string}[]=[],sockets:MockSocket[]=[],gates=new Set<()=>void>()
let mode='valid',writes=0,serial=0,mounts=0,games=0,peak=0,disposals=0,closes=0,originalCamera:unknown=null,sceneClosed=0
const auth=createSessionController({eventId,cookiePolicy:isolatedCookiePolicy(),fetch:async input=>{
 const r=input as Request,path=new URL(r.url).pathname;requests.push({path,method:r.method})
 if(path==='/api/auth/capabilities')return Response.json(capabilities())
 if(path==='/api/auth/login')return Response.json(authResponse())
 if(path==='/api/auth/logout')return new Response(null,{status:204})
 if(path.endsWith('/bootstrap'))return Response.json(data.bootstrap)
 if(path.endsWith('/rooms'))return Response.json(rooms())
 if(path.endsWith('/world-tickets'))return Response.json(ticket(await r.json() as WorldTicketRequest,++serial))
 if(path.endsWith('/places'))return Response.json({event_id:eventId,revision:1,items:data.bootstrap.map_manifest.places})
 if(path.includes('/places/'))return Response.json({event_id:eventId,revision:1,place:data.bootstrap.map_manifest.places.find(p=>p.place_id===path.split('/').at(-1))})
 return Response.json({code:'TEST',message:'Unavailable.',details:{reason:'RESOURCE_UNAVAILABLE'}},{status:404})
}})
auth.start();while(auth.read().capabilities==='loading')await new Promise(r=>setTimeout(r,5))
if(!await auth.login('fixture@example.test','fixture-password'))throw new Error('Test login failed')
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
}})
let composition:ReturnType<typeof createWorldComposition>|null=null
const router=createMemoryRouter(createAppRoutes(ports),{initialEntries:[new URLSearchParams(location.search).get('path')??'/world']})
const root=createRoot(document.getElementById('root')!);root.render(<RouterProvider router={router}/>)
const fixture={state:()=>{
 const p=composition?.presentation.read();if(p&&!originalCamera)originalCamera=p.camera
 return {path:router.state.location.pathname,phase:composition?.read().phase,published:!!p,theme:p?.theme.read()??null,camera:p?.camera.read(),sameCamera:p?.camera===originalCamera,mapHash:p?.map.expected.contentHash,geometry:p?.map.manifest.geometry_hash,room:composition?.replication.read()?.baseline?.roomId,outfit:composition?.replication.read()?.entities.find(e=>e.self)?.player.avatar,mounts,games,peak,disposals,closes,sceneClosed,writes,gates:gates.size,sockets:sockets.length,activeSockets:sockets.filter(s=>s.readyState<2).length,requests,canvases:document.querySelectorAll('.shell-world-host canvas').length}
},mode(value:string){mode=value},release(){const current=[...gates];gates.clear();current.forEach(resolve=>resolve())},navigate:(path:string)=>router.navigate(path),withdraw(){composition?.dispose()},policy(){auth.privacy.changePolicy(auth.privacy.capture(),'block')},async dispose(){root.unmount();router.dispose();ports.dispose?.();await composition?.settled();auth.dispose()}}
Object.assign(window,{__DEFAULT_QUALIFIED__:fixture})
declare global{interface Window{__DEFAULT_QUALIFIED__:typeof fixture}}
