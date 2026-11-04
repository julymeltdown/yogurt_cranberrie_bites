// CONTROLLED HTTP/C05 harness; positive mode requires an explicit contract-fixture selector.
import { createRoot } from 'react-dom/client'
import { createMemoryRouter, RouterProvider } from 'react-router-dom'
import type PhaserType from 'phaser'
import original from './source-inputs/operator-bootstrap.json'
import localInput from './source-inputs/local-input.json'
import roomData from '../../../../../../contracts/fixtures/realtime/rooms.json'
import movement from '../../../../../../contracts/fixtures/realtime/movement-heartbeat.json'
import { createAppRoutes } from '../../../router'
import { createProductionPorts } from '../../../ports'
import { createSessionController } from '../../../../features/auth/model/session'
import { authResponse, capabilities, isolatedCookiePolicy } from '../../../../features/auth/__tests__/fixtures/data'
import { MockSocket } from '../../../../world/network/mock'
import { validate } from '../../../../world/network/transport/contract'
import type { Snapshot, WorldTicketRequest } from '../../../../world/network/WorldTransport'
import { sha256 } from '../../../../world/map/json'
import { inspectInputReadiness } from './input-readiness.mjs'
import { eligibleAttendeeFixture, eligibleAttendeeScenario } from './eligible-attendee.mjs'
import '../../../../shared/styles/base.css'
import '../../../shell.css'

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
const auth = createSessionController({ eventId, qualifiedRefresh: true, cookiePolicy: isolatedCookiePolicy(), fetch: async input => {
  const request = input as Request, path = new URL(request.url).pathname; requests.push({ path, method: request.method })
  if (path === '/api/auth/capabilities') return Response.json(capabilities())
  if (path === '/api/auth/login') return Response.json({ ...authResponse(), userId })
  if (path === '/api/auth/logout') return new Response(null, { status: 204 })
  if (path.endsWith('/bootstrap')) return Response.json(bootstrap)
  if (path.endsWith('/rooms')) return Response.json(rooms)
  if (path.endsWith('/world-tickets')) {
    const body = await request.json() as WorldTicketRequest
    return Response.json({ ...body, protocol_version: '1.0.0', event_id: eventId, ticket: 'A'.repeat(43),
      server_time: new Date().toISOString(), expires_at: new Date(Date.now() + 30_000).toISOString(), ws_path: '/world-ws', auth_timeout_ms: 3000 })
  }
  if (path.endsWith('/places')) return Response.json({ event_id: eventId, revision: 1, items: bootstrap.map_manifest.places })
  if (path.includes('/places/')) return Response.json({ event_id: eventId, revision: 1, place: bootstrap.map_manifest.places.find(p => p.place_id === path.split('/').at(-1)) })
  if (path.endsWith('/me')) return Response.json(bootstrap.participant)
  return Response.json({ code: 'RESOURCE_UNAVAILABLE', message: 'Controlled capability unavailable', details: { reason: 'RESOURCE_UNAVAILABLE' } }, { status: 404 })
} })
class Socket extends MockSocket {
  constructor() { super(); sockets.push(this); queueMicrotask(() => this.open()) }
  override send(text: string) {
    super.send(text); const message = JSON.parse(text)
    if (message.type === 'authenticate') queueMicrotask(() => { if (this.readyState === 1) this.receive(JSON.stringify(snapshot(message.roomId))) })
  }
}
Object.defineProperty(window, 'WebSocket', { configurable: true, writable: true, value: Socket })
auth.start(); while (auth.read().capabilities === 'loading') await new Promise(resolve => setTimeout(resolve, 5))
if (!await auth.login('controlled@example.test', 'fixture-password')) throw Error('Controlled login failed')
// NO load, resolveRelease, renderer, admission or asset-fetch override.
const ports = createProductionPorts(auth, { origin: location.origin, localInput })
const router = createMemoryRouter(createAppRoutes(ports), { initialEntries: ['/world/map'] })
const root = createRoot(document.getElementById('root')!); root.render(<RouterProvider router={router} />)
let disposed = false, firstCamera: unknown = null
async function game() {
  const Phaser = (await import('phaser')).default
  const pool = Reflect.get(Phaser.Display.Canvas.CanvasPool, 'pool') as { parent: unknown; canvas: HTMLCanvasElement }[]
  return pool.map(p => p.parent as PhaserType.Game).find(g => g?.canvas?.closest?.('.shell-world-host'))
}
function state() {
  const presentation = ports.worldAccess?.read()?.presentation?.read()
  if (presentation && !firstCamera) firstCamera = presentation.camera
  return { qualification: 'WORLD_POSITIVE_CONTRACT_FIXTURE_DEFAULT_PRODUCTION_GUARDS', productionAdmitted: false,
    scenario: eligibleAttendeeScenario.scenario,
    inputScenario: eligibleAttendeeScenario, inputReadiness: readiness, capabilities: structuredClone(bootstrap.capabilities),
    path: router.state.location.pathname, requests, map: presentation?.map.expected, overview: presentation?.overview?.read(),
    camera: presentation?.camera.read(), sameCamera: !!presentation && presentation.camera === firstCamera,
    canvases: document.querySelectorAll('.shell-world-host canvas').length,
    sockets: sockets.length, activeSockets: sockets.filter(s => s.readyState < 2).length,
    entities: ports.worldAccess?.read()?.replication.read()?.entities, urls: structuredClone(urls), disposed }
}
async function worldPixels() {
  const g = await game(); if (!g) throw Error('Missing default Phaser game')
  const image = await new Promise<HTMLImageElement>((resolve, reject) => g.renderer.snapshot(value => {
    if (!(value instanceof HTMLImageElement)) reject(Error('Unexpected renderer snapshot')); else resolve(value)
  }))
  const canvas = document.createElement('canvas'); canvas.width = image.naturalWidth; canvas.height = image.naturalHeight
  const context = canvas.getContext('2d')!; context.drawImage(image, 0, 0)
  const rgba = new Uint8Array(context.getImageData(0, 0, canvas.width, canvas.height).data.buffer)
  const scene = g.scene.getScenes(true).find(s => s.scene.key === 'mcr-map')
  const objects = scene?.children.list.map(o => ({ type: o.type, x: Reflect.get(o, 'x'), y: Reflect.get(o, 'y'),
    depth: Reflect.get(o, 'depth'), scaleX: Reflect.get(o, 'scaleX'), scaleY: Reflect.get(o, 'scaleY'),
    texture: Reflect.get(o, 'texture')?.key, frame: Reflect.get(o, 'frame')?.name }))
  const result = { width: canvas.width, height: canvas.height, rgba: await sha256(rgba), objects }
  canvas.width = canvas.height = 0; return result
}
const api = { state, worldPixels, navigate: (path: string) => router.navigate(path),
  back: () => router.navigate(-1),
  async dispose() { if (disposed) return; disposed = true; root.unmount(); router.dispose(); ports.dispose?.(); auth.dispose() },
}
Object.assign(window, { __MAP4_DEFAULT__: api })
