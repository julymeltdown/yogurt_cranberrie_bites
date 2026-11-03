// MOCK_READY: actual root/default production loader; only Auth HTTP and C05 socket are deterministic.
import { createRoot } from 'react-dom/client'
import { createMemoryRouter, RouterProvider } from 'react-router-dom'
import { createAppRoutes } from '../../../src/app/router'
import { createProductionPorts } from '../../../src/app/ports'
import { createSessionController } from '../../../src/features/auth/model/session'
import { capabilities, authResponse, isolatedCookiePolicy } from '../../../src/features/auth/__tests__/fixtures/data'
import { rooms } from '../../../src/app/world-entry/__tests__/data'
import { ticket } from '../../../src/world/composition/__tests__/fixtures/data'
import { MockSocket } from '../../../src/world/network/mock'
import { validate } from '../../../src/world/network/transport/contract'
import { validateWholeCharacterConfig, type WholeCharacterRuntime } from '../../../src/entities/avatar/whole-character'
import { sha256 } from '../../../src/world/map/json'
import type { WorldTicketRequest, Snapshot, Delta } from '../../../src/world/network/WorldTransport'
import type PhaserType from 'phaser'
import { wholeData, eventId, participant, frame, changedSelf, CLOCK } from './data'
import '../../../src/shared/styles/base.css'
import '../../../src/app/shell.css'
const data=await wholeData(),requests:{path:string;method:string;body:unknown}[]=[],sockets:Socket[]=[],frames:{type:string;revision:number}[]=[]
let server=structuredClone(data.avatar),worldServer=structuredClone(server),serial=0,socketPeak=0,deltaCount=0
let retained:WholeCharacterRuntime|null=null,firstCamera:unknown=null,firstGame:object|null=null,peakGames=0,canvasPeak=0
const knownGames=new Set<object>(),activeGames=new Set<PhaserType.Game>()
let debugGame:PhaserType.Game|undefined
// Observe the real retained Phaser3.90 debug assignment; do not wrap its engine/factory/loader.
Object.defineProperty(window,'PHASER_GAME',{configurable:true,get:()=>debugGame,set(game:PhaserType.Game){debugGame=game;knownGames.add(game);activeGames.add(game);peakGames=Math.max(peakGames,activeGames.size);game.events.once('destroy',()=>activeGames.delete(game))}})
const mutation=new MutationObserver(()=>{canvasPeak=Math.max(canvasPeak,document.querySelectorAll('.shell-world-host canvas').length)})
mutation.observe(document.getElementById('root')!,{childList:true,subtree:true})
const auth=createSessionController({eventId,cookiePolicy:isolatedCookiePolicy(),fetch:async input=>{
 const request=input as Request,path=new URL(request.url).pathname,body=request.method==='PUT'?await request.clone().json():null
 requests.push({path,method:request.method,body})
 if(path==='/api/auth/capabilities')return Response.json(capabilities())
 if(path==='/api/auth/login')return Response.json(authResponse())
 if(path==='/api/auth/logout')return new Response(null,{status:204})
 if(path.endsWith('/bootstrap'))return Response.json({...data.bootstrap,avatar:server,participant:participant(data,server)})
 if(path.endsWith('/rooms'))return Response.json(rooms())
 if(path.endsWith('/world-tickets'))return Response.json(ticket(await request.json() as WorldTicketRequest,++serial))
 if(path.endsWith('/places'))return Response.json({event_id:eventId,revision:1,items:data.bootstrap.map_manifest.places})
 if(path.endsWith('/me')&&request.method==='GET')return Response.json(participant(data,server))
 if(path.endsWith('/me/avatar')&&request.method==='PUT'){
  if(body.expected_revision!==server.revision)return Response.json({code:'CONFLICT',message:'Fixture revision conflict',details:{reason:'VERSION_CONFLICT'}},{status:409})
  const config=validateWholeCharacterConfig(body.config);server={revision:server.revision+1,config}
  queueMicrotask(()=>{for(const socket of sockets)socket.broadcast()})
  return Response.json(server)
 }
 return Response.json({code:'RESOURCE_UNAVAILABLE',message:'No deterministic handler for this resource',details:{reason:'RESOURCE_UNAVAILABLE'}},{status:404})
}})
class Socket extends MockSocket {
 baseline:Snapshot|null=null
 sequence=0
 announced=0
 constructor(){super();sockets.push(this);socketPeak=Math.max(socketPeak,sockets.filter(s=>s.readyState<2).length);queueMicrotask(()=>this.open())}
 override send(text:string){
  super.send(text);const message=JSON.parse(text)
  if(['authenticate','switchRoom','resync'].includes(message.type))queueMicrotask(()=>{
   if(this.readyState!==1)return
   const room=message.roomId??this.baseline?.roomId??rooms().default_room_id,cause=message.type==='switchRoom'?'ROOM_SWITCH':message.type==='resync'?'RESYNC':'INITIAL'
   this.baseline=frame(data,room,worldServer,cause,message.requestId??null);this.sequence=0;this.announced=worldServer.revision
   this.receive(JSON.stringify(this.baseline));frames.push({type:'snapshot',revision:worldServer.revision})
   // An explicit server delta follows a stale-but-valid resume snapshot after a Save made while covered.
   queueMicrotask(()=>this.broadcast())
  })
 }
 broadcast(){
  if(this.readyState!==1||!this.baseline||this.announced>=server.revision)return
  const delta=validate<Delta>('Delta',{...changedSelf(this.baseline,server),streamSeq:++this.sequence,serverTick:this.baseline.serverTick+this.sequence})
  this.receive(JSON.stringify(delta));this.announced=server.revision;worldServer=structuredClone(server);deltaCount++;frames.push({type:'delta',revision:server.revision})
 }
}
Object.defineProperty(window,'WebSocket',{configurable:true,writable:true,value:Socket})
auth.start();while(auth.read().capabilities==='loading')await new Promise(resolve=>setTimeout(resolve,5))
if(!await auth.login('fixture@example.test','fixture-password'))throw Error('Canonical fixture login failed')
// Intentionally no load, assetFetch, renderer or importEngine override.
const ports=createProductionPorts(auth,{origin:location.origin,localInput:{revision:'QA_ONLY_not_C05_movement_authority',reviewedBy:'TEST ONLY fixture policy',movementPolicy:{speedPxPerSecond:96,footWidthPx:12,footHeightPx:8,interactionRadiusPx:96},zoom:1}})
const router=createMemoryRouter(createAppRoutes(ports),{initialEntries:['/world']}),root=createRoot(document.getElementById('root')!)
root.render(<RouterProvider router={router}/>)
function runtime(){
 const registry=Reflect.get(ports,'characters')
 const current=typeof registry?.read==='function'?registry.read(auth.privacy.capture(),data.pin) as WholeCharacterRuntime|null:null
 if(current&&!retained)retained=current
 return current
}
function state(){
 const binding=ports.worldAccess?.read(),presentation=binding?.presentation?.read(),view=binding?.replication.read(),current=runtime()
 if(presentation&&!firstCamera)firstCamera=presentation.camera
 return {clock:CLOCK,defaultProductionLoad:true,compositionOverride:false,path:router.state.location.pathname,bootstrapVersion:data.bootstrap.avatar_manifest.contract_version,manifestPin:data.pin,
  server:structuredClone(server),requests,frames,deltaCount,published:!!presentation,phase:view?.phase,entities:view?.entities.map(e=>({self:e.self,userId:e.player.userId,avatar:e.player.avatar})),
  sameCamera:presentation?presentation.camera===firstCamera:null,camera:presentation?.camera.read(),geometry:presentation?.map.manifest.geometry_hash,theme:presentation?.theme.read(),
  currentRuntime:!!current,sameRuntime:current?current===retained:null,runtime:retained?.diagnostics(),retainedCurrent:retained?.isCurrent()??false,
  sockets:sockets.length,socketPeak,activeSockets:sockets.filter(s=>s.readyState<2).length,canvasPeak,canvases:document.querySelectorAll('.shell-world-host canvas').length}
}
async function inspectWorld(){
 const Phaser=(await import('phaser')).default,games=[...activeGames]
 peakGames=Math.max(peakGames,games.length);for(const game of games)knownGames.add(game)
 const game=games[0];if(game&&!firstGame)firstGame=game
 const scene=game?.scene.getScenes(true).find(s=>s.scene.key==='mcr-map')
 const sprites=scene?.children.list.filter((object):object is import('phaser').GameObjects.Sprite=>object instanceof Phaser.GameObjects.Sprite&&object.texture.key.startsWith('mcr-avatar-'))??[]
 const hashes=[]
 for(const sprite of sprites){
  const source=sprite.texture.getSourceImage() as HTMLCanvasElement,canvas=document.createElement('canvas');canvas.width=canvas.height=64
  const context=canvas.getContext('2d')!;context.imageSmoothingEnabled=false;context.drawImage(source,0,0,64,64)
  hashes.push({x:sprite.x,y:sprite.y,visible:sprite.visible,frame:Number(sprite.frame.name),hash:await sha256(new Uint8Array(context.getImageData(0,0,64,64).data.buffer))});canvas.width=canvas.height=0
 }
 return {games:games.length,peakGames,totalObservedGames:knownGames.size,sameGame:game?game===firstGame:null,sprites:hashes,canvases:document.querySelectorAll('.shell-world-host canvas').length}
}
async function approvedPixels(){
 const current=runtime();if(!current)throw Error('Actual root registry has no current runtime')
 const hashes=[]
 for(const config of data.presets){const lease=await current.acquire(config,{signal:auth.session.read().signal}),render=lease.read();if(!render)throw Error('Actual reviewed render missing')
  const canvas=document.createElement('canvas');canvas.width=canvas.height=64;const context=canvas.getContext('2d')!;context.imageSmoothingEnabled=false
  context.drawImage(render.image,render.source.x,render.source.y,render.source.width,render.source.height,0,0,64,64)
  hashes.push({preset:config.preset_id,hash:await sha256(new Uint8Array(context.getImageData(0,0,64,64).data.buffer))});lease.dispose();canvas.width=canvas.height=0
 }
 return hashes
}
async function canvasPixels(selector:string){
 const element=document.querySelector<HTMLCanvasElement>(selector);if(!element)return null
 const canvas=document.createElement('canvas');canvas.width=canvas.height=64;const context=canvas.getContext('2d')!;context.imageSmoothingEnabled=false;context.drawImage(element,0,0,64,64)
 const hash=await sha256(new Uint8Array(context.getImageData(0,0,64,64).data.buffer));canvas.width=canvas.height=0;return hash
}
const api={state,inspectWorld,approvedPixels,canvasPixels,navigate:(path:string)=>router.navigate(path),privacy(){auth.privacy.changePolicy(auth.privacy.capture(),'block')},async dispose(){mutation.disconnect();root.unmount();router.dispose();ports.dispose?.();auth.dispose()}}
Object.assign(window,{__MCR_WHOLE_ROOT__:api})
declare global{interface Window{__MCR_WHOLE_ROOT__:typeof api}}
