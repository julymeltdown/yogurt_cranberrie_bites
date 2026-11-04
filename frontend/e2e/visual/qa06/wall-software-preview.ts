import { admission } from '../../../src/features/auth/__tests__/enrollment-consent/fixtures'
// QA06 headless fixture derivative: original Wall transport behavior unchanged,
// canonical current Auth admission fixture read added; no production gate bypass.
// TEST ONLY: actual main, canonical login, production Wall factory and incumbent Phaser.
// HTTP/C05 below are deterministic simulations; no live/backend/IN04 claim.
import { wholeData, eventId, userId, frame, changedSelf } from '../../reference-integrated/whole/data'
import { domainResponse } from '../../reference-integrated/data'
import { response as socialResponse } from '../../../src/app/social-composition/__tests__/fixtures'
import { capabilities, authResponse } from '../../../src/features/auth/__tests__/fixtures/data'
import { rooms } from '../../../src/app/world-entry/__tests__/data'
import { ticket } from '../../../src/world/composition/__tests__/fixtures/data'
import { MockSocket } from '../../../src/world/network/mock'
import type { WorldTicketRequest } from '../../../src/world/network/WorldTransport'
import { island } from '../../../src/features/mural/editor/__tests__/wall-tools-b/fixture'
import { previewWallAction } from '../../../src/features/mural/editor/tools'
import placement from '../../../src/app/world-entry/mural-production/placement.json'

if (!import.meta.env.DEV) throw Error('Wall fixture is development-only')
const data = await wholeData(), originalFetch = window.fetch.bind(window)
const requests: { path: string; method: string; key: string | null; authenticated: boolean }[] = [], sockets: Socket[] = []
const blobs = new Set<string>(), createURL = URL.createObjectURL.bind(URL), revokeURL = URL.revokeObjectURL.bind(URL)
URL.createObjectURL = blob => { const url = createURL(blob); blobs.add(url); return url }
URL.revokeObjectURL = url => { blobs.delete(url); revokeURL(url) }
let mural = island(), peak = 0, serial = 0, snapshots = 0, toolDenied = false, loseNext = false
type Body = { expected_revision: number; x?: number; y?: number; color_index?: number; target_action_id?: string }
type Action = { action_id: string; receipt_id: string; event_id: string; actor_id: string;
  command: { operation_id: string; body: Body }; idempotency_key: string; kind: string; revision: number;
  changed_cells: number; committed_at: string; next_allowed_at: string; undo_expires_at: string; reset_required: boolean }
const actions = new Map<string, { action: Action; before: number[]; pixel: Record<string, unknown> | null }>()
let latest: Action | null = null
const utc = () => new Date().toISOString()
const reject = (reason = 'RESOURCE_UNAVAILABLE', status = 404) => Response.json({ code: `MCR_${status}`, message: 'Unavailable.', details: { reason } }, { status })
function tools() {
  const undo = latest && latest.kind !== 'UNDO' && latest.revision === mural.revision
    ? { action_id: latest.action_id, revision: latest.revision, changed_cells: latest.changed_cells, expires_at: latest.undo_expires_at } : null
  return { version: 1, event_id: eventId, actor_id: userId, revision: mural.revision, server_time: utc(), background_index: 0,
    fill_limit: 256, cell_allowance: 512, window_seconds: 600, remaining_cells: 512, next_cell_available_at: null, next_allowed_at: null,
    editable: true, available: { pencil: true, erase: true, fill: true, undo: !!undo }, undo_window_seconds: 600,
    undo, latest_action: latest, unresolved: null }
}
function receipt(action: Action) {
  return { receipt_id: action.receipt_id, event_id: eventId, actor_id: userId, operation_id: action.command.operation_id,
    idempotency_key: action.idempotency_key, state: 'SUCCEEDED', resource_id: eventId, resource_revision: action.revision,
    failure_reason: null, result_status: 200, created_at: action.committed_at, expires_at: new Date(Date.parse(action.committed_at) + 86400000).toISOString() }
}
async function wall(request: Request, path: string, url: URL): Promise<Response> {
  if (!request.headers.get('Authorization')?.startsWith('Bearer ')) return reject('AUTHENTICATION_REQUIRED', 401)
  if (path.endsWith('/mural/tools')) return toolDenied ? reject('RESOURCE_UNAVAILABLE') : Response.json(tools())
  if (path.endsWith('/mural/snapshot')) return Response.json({ ...mural, event_id: eventId, server_time: utc() })
  if (path.endsWith('/mural/operations')) {
    const after = Number(url.searchParams.get('after_revision'))
    return Response.json({ event_id: eventId, from_revision: after, to_revision: mural.revision, reset_required: after !== mural.revision, operations: [] })
  }
  if (path.endsWith('/commands')) {
    const found = actions.get(url.searchParams.get('idempotency_key') ?? '')
    return found ? Response.json(receipt(found.action)) : reject()
  }
  if (path.includes('/mural/actions/')) {
    const found = actions.get(path.split('/').at(-1)!)
    return found ? Response.json(found.action) : reject()
  }
  if (path.endsWith('/result')) {
    const found = actions.get(path.split('/').at(-2)!)
    return found?.pixel ? Response.json({ receipt_id: found.action.receipt_id, idempotency_key: found.action.idempotency_key, result: found.pixel }) : reject()
  }
  if (request.method !== 'POST') return reject()
  const kind = path.split('/').at(-1)!, names: Record<string, string> = { pixels: 'placeConcertPixel', erase: 'eraseConcertMuralPixel', fill: 'fillConcertMuralRegion', undo: 'undoConcertMuralAction' }
  if (!names[kind]) return reject()
  const key = request.headers.get('Idempotency-Key')!, body = await request.json() as Body
  const repeated = actions.get(key)
  if (repeated) return Response.json(repeated.pixel ?? repeated.action)
  if (body.expected_revision !== mural.revision) return reject('VERSION_CONFLICT', 409)
  const before = [...mural.pixels], pixels = [...before]
  let changed = 1
  if (kind === 'undo') {
    if (!latest || latest.action_id !== body.target_action_id || latest.kind === 'UNDO' || latest.revision !== mural.revision) return reject('MURAL_UNDO_NOT_LATEST', 409)
    const previous = actions.get(latest.idempotency_key)!
    changed = previous.action.changed_cells
    pixels.splice(0, pixels.length, ...previous.before)
  } else if (kind === 'pixels') pixels[body.y! * 128 + body.x!] = body.color_index!
  else {
    const preview = previewWallAction(mural, kind as 'erase' | 'fill', body.x!, body.y!, kind === 'erase' ? 0 : body.color_index!)
    if (!preview.ok) return reject(preview.reason === 'FILL_TOO_LARGE' ? 'MURAL_FILL_TOO_LARGE' : 'MURAL_NO_CHANGE', 409)
    changed = preview.draft.count
    for (const cell of preview.draft.cells) pixels[cell] = kind === 'erase' ? 0 : body.color_index!
  }
  const committed = utc(), action: Action = { action_id: crypto.randomUUID(), receipt_id: crypto.randomUUID(), event_id: eventId, actor_id: userId,
    command: { operation_id: names[kind]!, body }, idempotency_key: key, kind: kind === 'pixels' ? 'PENCIL' : kind.toUpperCase(),
    revision: mural.revision + 1, changed_cells: changed, committed_at: committed, next_allowed_at: committed,
    undo_expires_at: new Date(Date.parse(committed) + 600000).toISOString(), reset_required: kind !== 'pixels' }
  const pixel = kind === 'pixels' ? { operation_id: action.action_id, event_id: eventId, revision: action.revision,
    x: body.x, y: body.y, color_index: body.color_index, committed_at: committed, next_allowed_at: committed } : null
  mural = { ...mural, revision: action.revision, pixels }; latest = action; actions.set(key, { action, before, pixel })
  if (loseNext) { loseNext = false; throw Error('TEST ONLY lost response after fixture commit') }
  return Response.json(pixel ?? action)
}
window.fetch = async (input, init) => {
  const request = input instanceof Request ? input : new Request(new URL(String(input), location.origin), init)
  const url = new URL(request.url), path = url.pathname
  if (!path.startsWith('/api/')) return originalFetch(input, init)
  requests.push({ path, method: request.method, key: request.headers.get('Idempotency-Key'), authenticated: !!request.headers.get('Authorization') })
  if (path === '/api/auth/capabilities') return Response.json(capabilities())
  if (path === '/api/auth/login') return Response.json(authResponse())
  if (path === '/api/auth/enrollment/admission') {
    if (request.method !== 'GET' || request.headers.get('Authorization') !== 'Bearer ' + authResponse().accessToken) throw Error('Canonical authenticated admission read required')
    return Response.json(admission())
  }
  if (path === '/api/auth/logout') return new Response(null, { status: 204 })
  if (path.endsWith('/bootstrap')) return Response.json(data.bootstrap)
  if (path.includes('/mural/') || path.endsWith('/commands')) return wall(request, path, url)
  if (path.endsWith('/rooms')) return Response.json(rooms())
  if (path.endsWith('/world-tickets')) return Response.json(ticket(await request.json() as WorldTicketRequest, ++serial))
  if (path.endsWith('/places')) return Response.json({ event_id: eventId, revision: 1, items: data.bootstrap.map_manifest.places })
  if (path.endsWith('/me')) return Response.json(data.bootstrap.participant)
  return domainResponse(request, eventId, userId, data.bootstrap.map_manifest.places[0]!.place_id) ?? socialResponse(request)
}
class Socket extends MockSocket {
  room = rooms().default_room_id
  baseline: ReturnType<typeof frame> | null = null
  sequence = 0
  tick() {
    const baseline = this.baseline
    if (!baseline || this.readyState !== 1) return false
    const delta = changedSelf(baseline, data.avatar)
    this.receive(JSON.stringify({ ...delta, streamSeq: ++this.sequence, serverTick: baseline.serverTick + this.sequence }))
    return true
  }
  constructor() { super(); sockets.push(this); peak = Math.max(peak, sockets.filter(socket => socket.readyState < 2).length); queueMicrotask(() => this.open()) }
  override send(text: string) {
    super.send(text); const message = JSON.parse(text)
    if (['authenticate', 'switchRoom', 'resync'].includes(message.type)) {
      this.room = message.roomId ?? this.room
      queueMicrotask(() => {
        this.sequence = 0
        this.baseline = { ...frame(data, this.room, data.avatar,
          message.type === 'switchRoom' ? 'ROOM_SWITCH' : message.type === 'resync' ? 'RESYNC' : 'INITIAL', message.requestId ?? null),
          streamId: `00000000-0000-4000-8000-${String(++snapshots).padStart(12, '0')}` }
        this.receive(JSON.stringify(this.baseline))
      })
    }
  }
}
Object.defineProperty(window, 'WebSocket', { configurable: true, writable: true, value: Socket })
async function worldProof() {
  const Phaser = (await import('phaser')).default
  const pool = Reflect.get(Phaser.Display.Canvas.CanvasPool, 'pool') as { parent: unknown }[]
  const game = pool.map(item => item.parent as import('phaser').Game).find(value => value?.canvas?.closest?.('.shell-world-host'))
  if (!game) return { mural: null, games: 0, canvases: document.querySelectorAll('.shell-world-host canvas').length }
  const scene = game.scene.getScene('mcr-map'), key = placement.textureKey
  if (!scene.textures.exists(key)) return { mural: null, games: 1, canvases: 1 }
  const image = scene.children.list.find(child => child instanceof Phaser.GameObjects.Image && child.texture.key === key) as import('phaser').GameObjects.Image | undefined
  const canvas = scene.textures.get(key).getSourceImage() as HTMLCanvasElement
  const rgba = canvas.getContext('2d')!.getImageData(0, 0, 128, 128).data
  const sample = (x: number, y: number) => Array.from(rgba.slice((y * 128 + x) * 4, (y * 128 + x) * 4 + 4))
  return { games: 1, canvases: 1, mural: { key, x: image?.x, y: image?.y, width: image?.displayWidth, height: image?.displayHeight,
    depth: image?.depth, island: sample(10, 10), background: sample(0, 0), objects: scene.children.list.filter(child => child instanceof Phaser.GameObjects.Image && child.texture.key === key).length } }
}
const api = { worldProof,
  state: () => ({ sockets: sockets.length, active: sockets.filter(socket => socket.readyState < 2).length, peak, snapshots,
    requests: [...requests], blobURLs: blobs.size, revision: mural.revision, island: mural.pixels[1290], qualification: 'ACTUAL_MAIN_SYNTHETIC_HTTP_C05_NOT_LIVE' }),
  ticks(count: number) { let sent = 0; for (let i = 0; i < count; i++) if (sockets.at(-1)?.tick()) sent++; return sent },
  denyTools(value: boolean) { toolDenied = value }, loseNext() { loseNext = true },
  hidden(value: boolean) { Object.defineProperty(document, 'hidden', { configurable: true, get: () => value }); document.dispatchEvent(new Event('visibilitychange')) },
  close() { sockets.at(-1)?.serverClose() },
}
Object.assign(window, { __MURAL_ROOT_ENTRY__: api })
declare global { interface Window { __MURAL_ROOT_ENTRY__: typeof api } }
// No replacement production controller/ports/router/Auth/rendering/clock or second engine.
await import('../../../src/main')
