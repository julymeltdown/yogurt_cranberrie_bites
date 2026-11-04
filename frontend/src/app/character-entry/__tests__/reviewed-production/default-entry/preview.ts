// TEST ONLY: actual main.tsx, canonical Auth/login UI and default production loader.
// HTTP/socket simulation adapted from app/__tests__/cozy-backgrounds-v1/preview.tsx.
// Selected character originals stay unchanged; map admission remains synthetic.
import { wholeData, eventId, userId, frame } from '../../../../../../e2e/reference-integrated/whole/data'
import { manifest, pin, preset, rehash } from '../../../../../features/avatar/__tests__/reviewed-production/fixtures'
import { object } from '../../../../../features/auth/model/contract'
import { decodeReviewedMotionConfig } from '../../../../../entities/avatar/runtime/motion-adoption/reviewed-config'
import { domainResponse } from '../../../../../../e2e/reference-integrated/data'
import { response as socialResponse } from '../../../../social-composition/__tests__/fixtures'
import { capabilities, authResponse } from '../../../../../features/auth/__tests__/fixtures/data'
import { rooms } from '../../../../world-entry/__tests__/data'
import { ticket } from '../../../../../world/composition/__tests__/fixtures/data'
import { MockSocket } from '../../../../../world/network/mock'
import type { WorldTicketRequest } from '../../../../../world/network/WorldTransport'

if (!import.meta.env.DEV) throw Error('Reviewed default-entry fixture is development-only')
const data = await wholeData(), originalFetch = window.fetch.bind(window)
const bundle = object(structuredClone(data.bootstrap))
let avatar = { revision: 7, config: preset(0) }, participantRevision = Number(object(bundle.participant).revision)
bundle.avatar_manifest = structuredClone(manifest)
bundle.avatar = structuredClone(avatar)
object(object(bundle.participant).profile).avatar = structuredClone(avatar)
for (const raw of bundle.theme_manifests as unknown[]) {
  const theme = object(raw)
  Object.assign(theme, { avatar_manifest_id: pin.manifest_id, avatar_manifest_version: pin.manifest_version, avatar_content_hash: pin.content_hash })
  await rehash(theme)
}
const blobs = new Set<string>(), createURL = URL.createObjectURL.bind(URL), revokeURL = URL.revokeObjectURL.bind(URL)
URL.createObjectURL = blob => { const url = createURL(blob); blobs.add(url); return url }
URL.revokeObjectURL = url => { blobs.delete(url); revokeURL(url) }
function me() {
  const value = structuredClone(object(bundle.participant)); value.revision = participantRevision
  object(value.profile).avatar = structuredClone(avatar); return value
}
const requests: { path: string; method: string }[] = [], sockets: Socket[] = []
let serial = 0, peak = 0, holdSnapshot = false, snapshots = 0
function bind(value: unknown): unknown {
  if (Array.isArray(value)) return value.map(bind)
  if (value && typeof value === 'object') return Object.fromEntries(Object.entries(value).map(([key, item]) =>
    [key, key === 'event_id' ? eventId : ['owner_id', 'actor_id'].includes(key) ? userId : key === 'avatar' ? structuredClone(avatar) : bind(item)]))
  return value
}
window.fetch = async (input, init) => {
  const request = input instanceof Request ? input : new Request(new URL(String(input), location.origin), init)
  const path = new URL(request.url).pathname
  if (!path.startsWith('/api/')) return originalFetch(input, init)
  requests.push({ path, method: request.method })
  if (path === '/api/auth/capabilities') return Response.json(capabilities())
  if (path === '/api/auth/login') return Response.json(authResponse())
  if (path === '/api/auth/logout') return new Response(null, { status: 204 })
  if (path.endsWith('/bootstrap')) return Response.json(bundle)
  if (path.endsWith('/me/avatar') && request.method === 'PUT') {
    const body = object(await request.json())
    if (body.expected_revision !== avatar.revision) return Response.json({ code: 'CONFLICT', message: 'Changed', details: { reason: 'VERSION_CONFLICT' } }, { status: 409 })
    avatar = { revision: avatar.revision + 1, config: decodeReviewedMotionConfig(body.config) }; participantRevision++
    bundle.avatar = structuredClone(avatar); object(object(bundle.participant).profile).avatar = structuredClone(avatar)
    queueMicrotask(() => sockets.at(-1)?.updateSelf())
    return Response.json(avatar)
  }
  if (path.endsWith('/rooms')) return Response.json(rooms())
  if (path.endsWith('/world-tickets')) return Response.json(ticket(await request.json() as WorldTicketRequest, ++serial))
  if (path.endsWith('/places')) return Response.json({ event_id: eventId, revision: 1, items: data.bootstrap.map_manifest.places })
  if (path.includes('/places/')) return Response.json({ event_id: eventId, revision: 1, place: data.bootstrap.map_manifest.places.find(p => p.place_id === path.split('/').at(-1)) })
  if (path.endsWith('/me')) return Response.json(me())
  const response = domainResponse(request, eventId, userId, data.bootstrap.map_manifest.places[0]!.place_id) ?? await socialResponse(request)
  const value = bind(await response.json()) as Record<string, unknown>
  if (path.endsWith('/me/age-status')) value.user_id = userId
  if (path.includes('/people/')) value.user_id = path.split('/').at(-1)
  return Response.json(value, { status: response.status })
}
class Socket extends MockSocket {
  room = rooms().default_room_id
  snapshot: ReturnType<typeof frame> | null = null
  sequence = 0
  constructor() { super(); sockets.push(this); peak = Math.max(peak, sockets.filter(s => s.readyState < 2).length); queueMicrotask(() => this.open()) }
  override send(text: string) {
    super.send(text); const message = JSON.parse(text)
    if (['authenticate', 'switchRoom', 'resync'].includes(message.type)) {
      this.room = message.roomId ?? this.room
      if (!holdSnapshot) queueMicrotask(() => this.admit(message.type === 'switchRoom' ? 'ROOM_SWITCH' : message.type === 'resync' ? 'RESYNC' : 'INITIAL', message.requestId ?? null))
    }
  }
  admit(cause: 'INITIAL' | 'ROOM_SWITCH' | 'RESYNC' = 'INITIAL', requestId: string | null = null) {
    if (this.readyState !== 1) return
    const snapshot = { ...frame(data, this.room, data.avatar, cause, requestId),
      streamId: `00000000-0000-4000-8000-${String(++snapshots).padStart(12, '0')}` }
    Object.assign(snapshot, { avatarManifestId: pin.manifest_id, avatarManifestVersion: pin.manifest_version,
      players: snapshot.players.map((player, index) => ({ ...player, avatar: index === 0 ? avatar : { revision: 1, config: preset(index % manifest.presets.length) } })) })
    this.snapshot = snapshot; this.sequence = 0
    // Synthetic server snapshot through the real transport; no client state injection.
    this.receive(JSON.stringify(snapshot))
  }
  updateSelf() {
    if (!this.snapshot || this.readyState !== 1) return
    const self = this.snapshot.players.find(player => player.userId === userId)!
    this.receive(JSON.stringify({ protocolVersion: '1.0.0', type: 'delta', streamId: this.snapshot.streamId,
      streamSeq: ++this.sequence, roomEpoch: this.snapshot.roomEpoch, serverTick: this.snapshot.serverTick + this.sequence,
      ackInputSeq: 0, upserts: [{ ...self, avatar: structuredClone(avatar) }], removes: [] }))
  }
}
Object.defineProperty(window, 'WebSocket', { configurable: true, writable: true, value: Socket })
// Test-only observation of the incumbent Phaser scene. No owner, clock or renderer is created.
async function worldProof() {
  const Phaser = (await import('phaser')).default
  const pool = Reflect.get(Phaser.Display.Canvas.CanvasPool, 'pool') as {parent:unknown}[]
  const game = pool.map(item => item.parent as import('phaser').Game).find(value => value?.canvas?.closest?.('.shell-world-host'))
  if (!game) throw Error('Actual incumbent game unavailable')
  const scene = game.scene.getScene('mcr-map'), camera = scene.cameras.main
  const players = sockets.at(-1)?.snapshot?.players ?? []
  const digest = async (canvas: HTMLCanvasElement) => {
    const context=canvas.getContext('2d')!, pixels=context.getImageData(0,0,canvas.width,canvas.height).data
    let alpha=0;for(let i=3;i<pixels.length;i+=4)if(pixels[i])alpha++
    const sha256=[...new Uint8Array(await crypto.subtle.digest('SHA-256',pixels))].map(byte=>byte.toString(16).padStart(2,'0')).join('')
    return {sha256,alpha,width:canvas.width,height:canvas.height}
  }
  const actors=[]
  for(const player of players) {
    const sprite=scene.children.list.find(object => object instanceof Phaser.GameObjects.Sprite && object.texture.key.startsWith('mcr-avatar-') && object.x===player.position.x && object.y===player.position.y) as import('phaser').GameObjects.Sprite | undefined
    if(!sprite) throw Error('Actual actor sprite missing: '+player.userId)
    const frame=sprite.frame, source=sprite.texture.getSourceImage() as HTMLCanvasElement
    const crop=document.createElement('canvas');crop.width=crop.height=64
    crop.getContext('2d')!.drawImage(source,frame.cutX,frame.cutY,64,64,0,0,64,64)
    const cell=await digest(crop);crop.width=crop.height=0
    const bounds=sprite.getBounds(),view=camera.worldView
    actors.push({userId:player.userId,self:player.userId===userId,x:sprite.x,y:sprite.y,texture:sprite.texture.key,frame:frame.name,sourceWidth:source.width,sourceHeight:source.height,visible:sprite.visible,alpha:sprite.alpha,onCamera:bounds.right>view.left&&bounds.left<view.right&&bounds.bottom>view.top&&bounds.top<view.bottom,cell})
  }
  const profile=document.querySelector<HTMLCanvasElement>('.me-avatar--presentation canvas')
  return {actors,profile:profile?await digest(profile):null,sceneKey:scene.scene.key,canvasCount:document.querySelectorAll('.shell-world-host canvas').length}
}

const api = {
  worldProof,
  state: () => ({ sockets: sockets.length, active: sockets.filter(s => s.readyState < 2).length, peak,
    canvases: document.querySelectorAll('.shell-world-host canvas').length, requests, snapshots, avatar: structuredClone(avatar), blobURLs: blobs.size, manifestPin: pin, qualification: 'SYNTHETIC_REVIEWED_DEFAULT_APP_NOT_LIVE' }),
  offline(value: boolean) { Object.defineProperty(navigator, 'onLine', { configurable: true, get: () => !value }); dispatchEvent(new Event(value ? 'offline' : 'online')) },
  close() { sockets.at(-1)?.serverClose() },
  hold(value: boolean) { holdSnapshot = value },
  admit() { sockets.at(-1)?.admit() },
}
Object.assign(window, { __REVIEWED_ROOT_ENTRY__: api })
declare global { interface Window { __REVIEWED_ROOT_ENTRY__: typeof api } }
// No replacement controller, ports, router, auth/session, renderer or panel.
await import('../../../../../main')
