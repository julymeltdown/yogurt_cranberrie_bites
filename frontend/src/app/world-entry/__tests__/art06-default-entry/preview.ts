// Development-only deterministic HTTP/C05 input; actual main/Auth/default providers remain intact.
import original from '../map-presentation-v1/source-inputs/operator-bootstrap.json'
import roomData from '../../../../../../contracts/fixtures/realtime/rooms.json'
import movement from '../../../../../../contracts/fixtures/realtime/movement-heartbeat.json'
import { authResponse, capabilities } from '../../../../features/auth/__tests__/fixtures/data'
import { MockSocket } from '../../../../world/network/mock'
import { validate } from '../../../../world/network/transport/contract'
import type { Snapshot, WorldTicketRequest } from '../../../../world/network/WorldTransport'
import { sha256 } from '../../../../world/map/json'
import { inspectInputReadiness } from '../map-presentation-v1/input-readiness.mjs'
import { eligibleAttendeeFixture, eligibleAttendeeScenario } from '../map-presentation-v1/eligible-attendee.mjs'
import '../../../../shared/styles/base.css'
import '../../../shell.css'

if (!import.meta.env.DEV) throw Error('Development-only fixture')
const originalFetch = window.fetch.bind(window)
const parameters = new URLSearchParams(location.search)
const scenario = parameters.get('scenario')
if (scenario !== null && scenario !== eligibleAttendeeScenario.scenario) throw Error('Unknown Map4 input scenario')
const selectedInput = scenario === eligibleAttendeeScenario.scenario ? eligibleAttendeeFixture() : structuredClone(original)
const readiness = inspectInputReadiness(selectedInput)
Object.assign(window, { __MAP4_INPUT__: readiness })
if (readiness.status === 'INPUT_NOT_READY') throw Error('Map4 source input not ready: ' + readiness.reasons.join(', '))
const bootstrap = selectedInput, eventId = bootstrap.event.event_id, userId = bootstrap.participant.user_id
// Model the selected PARADE preference seen in retained IN01. All manifest DTOs stay byte-identical.
const requestedEra = parameters.get('era')
bootstrap.participant.preferences.theme = requestedEra === 'REVENGE' || requestedEra === 'DANGER' ? requestedEra : 'PARADE'
const rooms = { ...structuredClone(roomData), event_id: eventId }
const ticketTiming: {serverTimeMs:number;expiresTimeMs:number;lifetimeMs:number}[]=[]
const requests: { path: string; method: string }[] = [], sockets: Socket[] = [], urls = { created: [] as string[], revoked: [] as string[] }
const createURL = URL.createObjectURL.bind(URL), revokeURL = URL.revokeObjectURL.bind(URL)
URL.createObjectURL = blob => { const url = createURL(blob); urls.created.push(url); return url }
URL.revokeObjectURL = url => { urls.revoked.push(url); revokeURL(url) }
function snapshot(roomId: string) {
  const g = bootstrap.map_manifest.geometry, spawn = g.spawns.find(s => s.spawn_id === g.default_spawn_id)!
  const presets = bootstrap.avatar_manifest.presets
  const positions: { x: number; y: number }[] = []
  for (let y = spawn.cell_y; y < g.height_cells && positions.length < presets.length; y++)
    for (let x = spawn.cell_x; x < g.width_cells && positions.length < presets.length; x++)
      if (!g.collision_grid[y]![x]) positions.push({ x: (x + .5) * 16, y: (y + .5) * 16 })
  return validate<Snapshot>('Snapshot', { ...movement.messages[1]!.message, eventId, roomId,
    mapId: bootstrap.map_manifest.map_id, mapRevision: bootstrap.map_manifest.map_revision,
    avatarManifestId: bootstrap.avatar_manifest.manifest_id, avatarManifestVersion: bootstrap.avatar_manifest.manifest_version,
    accessExpiresAt: new Date(Date.now() + 240_000).toISOString(), selfUserId: userId,
    players: presets.map((preset, i) => ({ userId: i === 0 ? userId : `00000000-0000-4000-8000-00000000090${i}`,
      displayName: i === 0 ? bootstrap.participant.profile.display_name : `Controlled visitor ${i + 1}`,
      position: positions[i]!, facing: 'down', moving: false,
      avatar: { revision: 1, config: preset.config } })),
  })
}
window.fetch = async (input,init) => {
  const request = input instanceof Request ? input : new Request(new URL(String(input),location.origin),init)
  const path = new URL(request.url).pathname
  if (!path.startsWith('/api/')) return originalFetch(input,init)
  requests.push({ path, method: request.method })
  if (path === '/api/auth/capabilities') return Response.json(capabilities())
  if (path === '/api/auth/login') return Response.json({ ...authResponse(), userId })
  if (path === '/api/auth/logout') return new Response(null, { status: 204 })
  if (path.endsWith('/bootstrap')) return Response.json(bootstrap)
  if (path.endsWith('/rooms')) return Response.json(rooms)
  if (path.endsWith('/world-tickets')) {
    const body = await request.json() as WorldTicketRequest, now=Date.now()
    ticketTiming.push({serverTimeMs:now,expiresTimeMs:now+30000,lifetimeMs:30000})
    return Response.json({ ...body, protocol_version: '1.0.0', event_id: eventId, ticket: 'A'.repeat(43),
      server_time: new Date(now).toISOString(), expires_at: new Date(now + 30_000).toISOString(), ws_path: '/world-ws', auth_timeout_ms: 3000 })
  }
  if (path.endsWith('/places')) return Response.json({ event_id: eventId, revision: 1, items: bootstrap.map_manifest.places })
  if (path.includes('/places/')) return Response.json({ event_id: eventId, revision: 1, place: bootstrap.map_manifest.places.find(p => p.place_id === path.split('/').at(-1)) })
  if (path.endsWith('/me')) return Response.json(bootstrap.participant)
  return Response.json({ code: 'RESOURCE_UNAVAILABLE', message: 'Controlled capability unavailable', details: { reason: 'RESOURCE_UNAVAILABLE' } }, { status: 404 })
}
class Socket extends MockSocket {
 heartbeat: ReturnType<typeof setInterval> | null = null
 override close(){if(this.heartbeat)clearInterval(this.heartbeat);this.heartbeat=null;super.close()}
  constructor() { super(); sockets.push(this); queueMicrotask(() => this.open()) }
  override send(text: string) {
    super.send(text); const message = JSON.parse(text)
    if (message.type === 'authenticate') queueMicrotask(() => { if (this.readyState === 1) {this.receive(JSON.stringify(snapshot(message.roomId)));this.heartbeat=setInterval(()=>{if(this.readyState===1)this.receive(JSON.stringify({protocolVersion:'1.0.0',type:'ping',heartbeatId:crypto.randomUUID()}))},5000)} })
  }
}
Object.defineProperty(window, 'WebSocket', { configurable: true, writable: true, value: Socket })

async function game() {
 const Phaser=(await import('phaser')).default
 const pool=Reflect.get(Phaser.Display.Canvas.CanvasPool,'pool') as {parent:unknown}[]
 return pool.map(p=>p.parent as import('phaser').Game).find(g=>g?.canvas?.closest?.('.shell-world-host'))
}
async function worldProof() {
 const g=await game(); if(!g)throw Error('Missing incumbent Phaser')
 const scene=g.scene.getScene('mcr-map'), camera=scene.cameras.main
 const image=await new Promise<HTMLImageElement>((resolve,reject)=>g.renderer.snapshot(value=>value instanceof HTMLImageElement?resolve(value):reject(Error('Snapshot'))))
 const canvas=document.createElement('canvas');canvas.width=image.naturalWidth;canvas.height=image.naturalHeight
 const context=canvas.getContext('2d')!;context.drawImage(image,0,0)
 const rgba=await sha256(new Uint8Array(context.getImageData(0,0,canvas.width,canvas.height).data.buffer))
 const objects=scene.children.list.map(o=>({type:o.type,x:Reflect.get(o,'x'),y:Reflect.get(o,'y'),depth:Reflect.get(o,'depth'),scaleX:Reflect.get(o,'scaleX'),scaleY:Reflect.get(o,'scaleY'),texture:Reflect.get(o,'texture')?.key,frame:Reflect.get(o,'frame')?.name}))
 const result={width:canvas.width,height:canvas.height,rgba,objects,camera:{x:camera.scrollX,y:camera.scrollY,width:camera.width,height:camera.height,zoom:camera.zoom}}
 canvas.width=canvas.height=0;return result
}
const api={worldProof,camera:async()=>{const g=await game();const c=g?.scene.getScene('mcr-map').cameras.main;return c?{x:c.scrollX,y:c.scrollY,width:c.width,height:c.height,zoom:c.zoom}:null},state:()=>({qualification:'ACTUAL_MAIN_CANONICAL_AUTH_MAP4_EXPLICIT_HTTP_C05_FIXTURE_NOT_LIVE',productionAdmitted:false,scenario,readiness,ticketTiming,requests,sockets:sockets.length,activeSockets:sockets.filter(s=>s.readyState<2).length,canvases:document.querySelectorAll('.shell-world-host canvas').length,urls:structuredClone(urls),path:location.pathname,map:{id:bootstrap.map_manifest.map_id,revision:bootstrap.map_manifest.map_revision,contentHash:bootstrap.map_manifest.content_hash,geometryHash:bootstrap.map_manifest.geometry_hash},theme:bootstrap.participant.preferences.theme})}
Object.assign(window,{__ART06_DEFAULT__:api})
await import('../../../../main')
