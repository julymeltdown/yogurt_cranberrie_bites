// TEST ONLY. Borrow the unchanged default Root/Phaser fixture; control its synthetic server outcomes.
import type PhaserType from 'phaser'
import { MockSocket } from '../../../../world/network/mock'
import type { Snapshot, WorldRooms } from '../../../../world/network/WorldTransport'
import { sha256 } from '../../../../world/map/json'
let availability: 'AVAILABLE' | 'FULL' | 'UNAVAILABLE' = 'AVAILABLE', deniedTicket = false, catalogFailure = false
const json = Response.json.bind(Response)
Response.json = (value: unknown, options?: ResponseInit) => {
  const object = value as Partial<WorldRooms> & { ticket?: unknown }
  if (object?.default_room_id && Array.isArray(object.items)) {
    if (catalogFailure) return json({ code: 'DEPENDENCY_UNAVAILABLE', message: 'Synthetic unavailable catalog', details: { reason: 'DEPENDENCY_UNAVAILABLE' } }, { status: 503 })
    return json({ ...object, items: object.items.map(room => ({ ...room, availability: room.room_id === object.default_room_id ? 'AVAILABLE' : availability })) }, options)
  }
  if (object?.ticket && deniedTicket) return json({ code: 'PERMISSION_DENIED', message: 'Synthetic denied target', details: { reason: 'PERMISSION_DENIED' } }, { status: 403 })
  return json(value, options)
}
type Switch = { type: 'switchRoom'; roomId: string; requestId: string; streamId: string }
let last: Snapshot | null = null, pending: Switch | null = null, sequence = 0, revision = 810
const sockets: MockSocket[] = [], frames: { type: string; roomId?: string; requestId?: string }[] = []
const define = Object.defineProperty
Object.defineProperty = function(target, key, descriptor) {
  if (target === window && key === 'WebSocket' && descriptor.value) {
    Object.defineProperty = define
    const Original = descriptor.value as typeof MockSocket
    class ControlledSocket extends Original {
      constructor() { super(); sockets.push(this) }
      override send(text: string) {
        super.send(text)
        const frame = JSON.parse(text) as Switch
        frames.push({ type: frame.type, roomId: frame.roomId, requestId: frame.requestId })
        if (frame.type === 'switchRoom') pending = frame
      }
      override receive(text: string) {
        const frame = JSON.parse(text) as Snapshot
        if (frame.type === 'snapshot') { last = frame; sequence = 0 }
        super.receive(text)
      }
    }
    return define(target, key, { ...descriptor, value: ControlledSocket })
  }
  return define(target, key, descriptor)
}
await import('../../../../app/world-entry/__tests__/reference-scene-v5/preview')
const root = Reflect.get(window, '__SCENE_V5_ROOT__') as { state(): { phase: string }; dispose(): Promise<void> }
const uuid = () => `00000000-0000-4000-8000-${String(revision++).padStart(12, '0')}`
function receive(value: unknown) { sockets.at(-1)!.receive(JSON.stringify(value)) }
async function game() {
  const Phaser = (await import('phaser')).default
  const pool = Reflect.get(Phaser.Display.Canvas.CanvasPool, 'pool') as { parent: PhaserType.Game; canvas: HTMLCanvasElement }[]
  const entry = pool.find(item => item.parent?.canvas === item.canvas && item.canvas.closest('.shell-world-host'))
  if (!entry) throw Error('Actual retained Phaser unavailable')
  return entry.parent
}
let probe: { id: string; foot: { x: number; y: number }; x: number; y: number; width: number; height: number; pixels: Uint8ClampedArray; hash: string } | null = null
async function actorProbe(id: string) {
  const g = await game(), scene = g.scene.getScene('mcr-map'), player = last?.players.find(p => p.userId === id)
  if (!player) throw Error('Actor is not in captured authorized fixture baseline')
  const sprite = scene.children.list.find(o => o.type === 'Sprite' && Reflect.get(o, 'x') === player.position.x && Reflect.get(o, 'y') === player.position.y) as PhaserType.GameObjects.Sprite | undefined
  if (!sprite?.visible) throw Error('Authorized actor has no visible actual Phaser sprite')
  const b = sprite.getBounds(), camera = scene.cameras.main
  const topLeft = camera.matrix.transformPoint(b.x - camera.scrollX, b.y - camera.scrollY)
  const bottomRight = camera.matrix.transformPoint(b.right - camera.scrollX, b.bottom - camera.scrollY)
  const x = Math.max(0, Math.floor(topLeft.x)), y = Math.max(0, Math.floor(topLeft.y))
  const width = Math.min(g.canvas.width - x, Math.ceil(bottomRight.x) - x), height = Math.min(g.canvas.height - y, Math.ceil(bottomRight.y) - y)
  if (width < 4 || height < 4) throw Error('Actor paint region is not visible on actual canvas')
  const pixels = g.canvas.getContext('2d')!.getImageData(x, y, width, height).data
  probe = { id, foot: player.position, x, y, width, height, pixels, hash: await sha256(new Uint8Array(pixels)) }
  return { id, foot: probe.foot, region: { x, y, width, height }, beforeHash: probe.hash, texture: sprite.texture.key }
}
async function removedPaint() {
  if (!probe) throw Error('Missing before-removal pixel sample')
  const g = await game(), scene = g.scene.getScene('mcr-map'), { x, y, width, height, pixels } = probe
  const after = g.canvas.getContext('2d')!.getImageData(x, y, width, height).data
  const sprites = scene.children.list.filter(o => o.type === 'Sprite' && String(Reflect.get(o, 'texture')?.key).startsWith('mcr-avatar-'))
  return { id: probe.id, beforeHash: probe.hash, afterHash: await sha256(new Uint8Array(after)),
    changedChannels: after.reduce((sum, value, i) => sum + Number(value !== pixels[i]), 0),
    removedActorSpritePresent: sprites.some(o => Reflect.get(o, 'x') === probe!.foot.x && Reflect.get(o, 'y') === probe!.foot.y),
    actualSpriteCount: sprites.length, canvas: { width: g.canvas.width, height: g.canvas.height } }
}
Object.assign(window, { __FE18_COMPLETION__: {
  state() { return { serverSnapshot: last && { roomId: last.roomId, streamId: last.streamId, roomEpoch: last.roomEpoch, players: last.players.map(p => ({ id: p.userId, name: p.displayName, foot: p.position })) }, pending, frames, availability, deniedTicket, catalogFailure, root: root.state() } },
  catalog(value: typeof availability) { availability = value }, denyTicket(value: boolean) { deniedTicket = value }, failCatalog(value: boolean) { catalogFailure = value },
  reject() { if (!pending) throw Error('No real outbound switch'); receive({ protocolVersion: '1.0.0', type: 'switchRejected', streamId: pending.streamId, requestId: pending.requestId, reason: 'ROOM_FULL' }); pending = null },
  admit(stale = false) { if (!pending || !last) throw Error('No real outbound switch/baseline'); receive({ ...last, streamSeq: 0, cause: 'ROOM_SWITCH', requestId: stale ? uuid() : pending.requestId, streamId: uuid(), roomEpoch: uuid(), roomId: pending.roomId }); if (!stale) pending = null },
  remove(id: string) { if (!last) throw Error('Missing authorized baseline'); receive({ protocolVersion: '1.0.0', type: 'delta', streamId: last.streamId, roomEpoch: last.roomEpoch, streamSeq: ++sequence, serverTick: last.serverTick + sequence, ackInputSeq: 0, upserts: [], removes: [id] }) },
  epochSelfOnly() { if (!last) throw Error('Missing authorized baseline'); receive({ ...last, streamSeq: 0, cause: 'EPOCH_CHANGED', requestId: null, streamId: uuid(), roomEpoch: uuid(), players: last.players.filter(p => p.userId === last!.selfUserId) }) },
  actorProbe, removedPaint,
  async dispose() { await root.dispose(); Response.json = json; Object.defineProperty = define },
} })
