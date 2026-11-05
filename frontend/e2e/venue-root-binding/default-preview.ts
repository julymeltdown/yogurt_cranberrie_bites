// Derived from exact canonical frontend/src/app/character-entry/__tests__/reviewed-production/default-entry/preview.ts; import paths normalized, explicit test admission/policy added.
// TEST ONLY: actual main.tsx, canonical Auth/login UI and default production loader.
// HTTP/socket simulation adapted from app/__tests__/cozy-backgrounds-v1/preview.tsx.
// Selected character originals stay unchanged; map admission remains synthetic.
import { wholeData, eventId, userId, frame } from '/e2e/reference-integrated/whole/data'
import { manifest, pin, preset, rehash } from '/src/features/avatar/__tests__/reviewed-production/fixtures'
import { object } from '/src/features/auth/model/contract'
import { decodeReviewedMotionConfig } from '/src/entities/avatar/runtime/motion-adoption/reviewed-config'
import { domainResponse } from '/e2e/reference-integrated/data'
import { response as socialResponse } from '/src/app/social-composition/__tests__/fixtures'
import { capabilities, authResponse } from '/src/features/auth/__tests__/fixtures/data'
import { rooms } from '/src/app/world-entry/__tests__/data'
import { ticket } from '/src/world/composition/__tests__/fixtures/data'
import { MockSocket } from '/src/world/network/mock'
import type { WorldTicketRequest } from '/src/world/network/WorldTransport'

if (!import.meta.env.DEV) throw Error('Reviewed default-entry fixture is development-only')
const data = await wholeData(), originalFetch = window.fetch.bind(window)
const bundle = object(structuredClone(data.bootstrap))
// TEST ONLY: read-only deterministic server state; no avatar writes are part of this journey.
// No credential, controller, runtime, or avatar draft is restored from browser storage.
const persisted = object(await (await originalFetch('/__reviewed_persistence_motion/state')).json())
const persistedAvatar = object(persisted.avatar)
let avatar = { revision: Number(persistedAvatar.revision), config: decodeReviewedMotionConfig(persistedAvatar.config) }
let participantRevision = Number(persisted.participantRevision)
const documentId = crypto.randomUUID()
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
const inputs: { seq: number; axisX: number; axisY: number }[] = []
const serverUpdates: { sequence: number; ackInputSeq: number; userIds: string[] }[] = []
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
  if (path === '/api/auth/enrollment/admission') return Response.json({ decision: 'ADMITTED', policyRevision: '11111111-1111-4111-8111-111111111111', acceptedRevision: '11111111-1111-4111-8111-111111111111', serverTime: new Date().toISOString() })
  if (path.endsWith('/service-policy')) return Response.json({ event_revision: 1, policy_revision: 1, kind: 'NORMAL', minimum_client_version: '0.1.0', server_time: new Date().toISOString(), valid_until: new Date(Date.now()+5000).toISOString() })
  if (path === '/api/auth/capabilities') return Response.json(capabilities())
  if (path === '/api/auth/login') return Response.json(authResponse())
  if (path === '/api/auth/logout') return new Response(null, { status: 204 })
  if (path.endsWith('/bootstrap')) return Response.json(bundle)
  if (path.endsWith('/me/avatar') && request.method === 'PUT') {
    const body = object(await request.json())
    if (body.expected_revision !== avatar.revision) return Response.json({ code: 'CONFLICT', message: 'Changed', details: { reason: 'VERSION_CONFLICT' } }, { status: 409 })
    // Validate the normal request before forwarding it to the durable test HTTP authority.
    decodeReviewedMotionConfig(body.config)
    const response = await originalFetch('/__reviewed_persistence_motion/state', {
      method: 'PUT', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(body),
    })
    if (!response.ok) return response
    const result = object(await response.json()), saved = object(result.avatar)
    avatar = { revision: Number(saved.revision), config: decodeReviewedMotionConfig(saved.config) }
    participantRevision = Number(result.participantRevision)
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
    if (message.type === 'input' && this.snapshot) queueMicrotask(() => {
      // A real socket cannot acknowledge before sendInput records its sent sequence.
      if (!this.snapshot || this.readyState !== 1) return
      inputs.push({ seq: message.seq, axisX: message.axisX, axisY: message.axisY })
      const self = this.snapshot.players.find(player => player.userId === userId)!
      const moving = message.axisX !== 0 || message.axisY !== 0
      const facing = !moving ? self.facing : Math.abs(message.axisX) >= Math.abs(message.axisY)
        ? message.axisX > 0 ? 'right' : 'left' : message.axisY > 0 ? 'down' : 'up'
      // Controlled server-side fixture response to the actual transport's input.
      // This does not call prediction, renderer, controller, scene or frame clocks.
      this.upsert([{ ...self, facing, moving,
        position: { x: Math.max(108, Math.min(172, self.position.x + message.axisX * 2)), y: self.position.y + message.axisY * 2 } }], message.seq)
    })
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
  upsert(players: NonNullable<Socket['snapshot']>['players'], ackInputSeq = 0) {
    if (!this.snapshot || this.readyState !== 1) return
    const snapshot = this.snapshot
    this.snapshot = { ...snapshot, players: snapshot.players.map(player => players.find(next => next.userId === player.userId) ?? player) }
    serverUpdates.push({ sequence: this.sequence + 1, ackInputSeq, userIds: players.map(player => player.userId) })
    this.receive(JSON.stringify({ protocolVersion: '1.0.0', type: 'delta', streamId: snapshot.streamId,
      streamSeq: ++this.sequence, roomEpoch: snapshot.roomEpoch, serverTick: snapshot.serverTick + this.sequence,
      ackInputSeq, upserts: players, removes: [] }))
  }
  updateSelf() {
    const self = this.snapshot?.players.find(player => player.userId === userId)
    if (self) this.upsert([{ ...self, avatar: structuredClone(avatar) }], inputs.at(-1)?.seq ?? 0)
  }
  peerMotion(moving: boolean) {
    const peer = this.snapshot?.players.find(player => player.userId !== userId)
    if (!peer) throw Error('No admitted peer')
    this.upsert([{ ...peer, facing: 'left', moving,
      position: { x: peer.position.x + (moving ? 16 : 0), y: peer.position.y },
      avatar: moving ? { revision: peer.avatar.revision + 1, config: preset(2) } : peer.avatar,
    }], inputs.at(-1)?.seq ?? 0)
    return peer.userId
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
  const actors=[], used = new Set<import('phaser').GameObjects.Sprite>()
  for(const player of players) {
    // Replication interpolates positions. Observe the nearest unique incumbent sprite;
    // peers start72px apart and this narrow journey moves each less than40px.
    const sprite = (scene.children.list.filter(value => value instanceof Phaser.GameObjects.Sprite
      && value.texture.key.startsWith('mcr-avatar-') && !used.has(value)) as import('phaser').GameObjects.Sprite[])
      .sort((a,b) => Math.hypot(a.x-player.position.x,a.y-player.position.y)-Math.hypot(b.x-player.position.x,b.y-player.position.y))[0]
    if (sprite) used.add(sprite)
    if(!sprite) throw Error('Actual actor sprite missing: '+player.userId)
    const frame=sprite.frame, source=sprite.texture.getSourceImage() as HTMLCanvasElement
    const crop=document.createElement('canvas');crop.width=crop.height=64
    crop.getContext('2d')!.drawImage(source,frame.cutX,frame.cutY,64,64,0,0,64,64)
    const cell=await digest(crop);crop.width=crop.height=0
    const bounds=sprite.getBounds(),view=camera.worldView
    actors.push({userId:player.userId,self:player.userId===userId,authoritative:{position:player.position,facing:player.facing,moving:player.moving,avatarRevision:player.avatar.revision},x:sprite.x,y:sprite.y,texture:sprite.texture.key,frame:frame.name,sourceWidth:source.width,sourceHeight:source.height,visible:sprite.visible,alpha:sprite.alpha,onCamera:bounds.right>view.left&&bounds.left<view.right&&bounds.bottom>view.top&&bounds.top<view.bottom,cell})
  }
  const profile=document.querySelector<HTMLCanvasElement>('.me-avatar--presentation canvas')
  return {actors,profile:profile?await digest(profile):null,sceneKey:scene.scene.key,canvasCount:document.querySelectorAll('.shell-world-host canvas').length}
}

const api = {
  worldProof,
  peerMotion(moving: boolean) { return sockets.at(-1)?.peerMotion(moving) },
  state: () => ({ sockets: sockets.length, active: sockets.filter(s => s.readyState < 2).length, peak,
    canvases: document.querySelectorAll('.shell-world-host canvas').length, requests, snapshots, documentId, inputs, serverUpdates, avatar: structuredClone(avatar), blobURLs: blobs.size, manifestPin: pin, qualification: 'ACTUAL_MAIN_CONTROLLED_HTTP_C05_APPROVED_CHARACTER_ART_NOT_LIVE' }),
  offline(value: boolean) { Object.defineProperty(navigator, 'onLine', { configurable: true, get: () => !value }); dispatchEvent(new Event(value ? 'offline' : 'online')) },
  close() { sockets.at(-1)?.serverClose() },
  hold(value: boolean) { holdSnapshot = value },
  admit() { sockets.at(-1)?.admit() },
}
Object.assign(window, { __REVIEWED_ROOT_ENTRY__: api })
declare global { interface Window { __REVIEWED_ROOT_ENTRY__: typeof api } }
// No replacement controller, ports, router, auth/session, renderer or panel.
await import('/src/main')
