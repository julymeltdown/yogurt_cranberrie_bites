// TEST ONLY. Real browser router, canonical Auth and default production World/Phaser.
// HTTP and C05 producers are explicitly synthetic. No product admission or real arrival.
import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { RouterProvider } from 'react-router-dom'
import { createAppRouter } from '../../router'
import { createProductionPorts } from '../../ports'
import { createSessionController } from '../../../features/auth/model/session'
import { isolatedCookiePolicy, capabilities, authResponse } from '../../../features/auth/__tests__/fixtures/data'
import { wholeData, eventId, userId, frame, participant } from '../../../../e2e/reference-integrated/whole/data'
import { rooms } from '../../world-entry/__tests__/data'
import { ticket } from '../../../world/composition/__tests__/fixtures/data'
import { MockSocket } from '../../../world/network/mock'
import type { WorldTicketRequest } from '../../../world/network/WorldTransport'
import type PhaserType from 'phaser'
import management from '../../../../../contracts/fixtures/domain/giveaway-management.json'
import '../../../shared/styles/base.css'
import '../../shell.css'

if (!import.meta.env.DEV) throw Error('FE07 evidence fixture is development-only')
const params = new URLSearchParams(location.search), data = await wholeData()
const giftId = management.giveaway.giveaway_id, giftPath = `/gifts/${giftId}`
const requests: { path: string; method: string; protected: boolean }[] = [], sockets: Socket[] = []
let serial = 0, peakSockets = 0, bootstrapHold = params.has('hold-bootstrap'), bootstrapRelease: (() => void) | null = null
let giftMode = 'ready', firstGame: PhaserType.Game | null = null, firstCamera: object | null = null
const knownGames = new Set<PhaserType.Game>(), liveGames = new Set<PhaserType.Game>()
let peakGames = 0
const gifts = Array.from({ length: 18 }, (_, index) => ({ ...structuredClone(management.giveaway),
  event_id: eventId, giveaway_id: index ? `00000000-0000-4000-8000-${(500 + index).toString(16).padStart(12, '0')}` : giftId,
  title: `Synthetic shell gift ${index + 1}`, description: 'Synthetic read-only giveaway. No stock, reservation, pickup or physical arrival proof.',
  created_at: new Date(Date.parse(management.giveaway.created_at) - index * 1000).toISOString(),
}))
const auth = createSessionController({ eventId, cookiePolicy: isolatedCookiePolicy(), fetch: async input => {
  const request = input as Request, path = new URL(request.url).pathname
  requests.push({ path, method: request.method, protected: request.headers.has('Authorization') })
  if (path === '/api/auth/capabilities') return Response.json(capabilities())
  if (path === '/api/auth/login') return Response.json(authResponse())
  if (path === '/api/auth/logout') return new Response(null, { status: 204 })
  if (path.endsWith('/bootstrap')) {
    if (bootstrapHold) await new Promise<void>(resolve => { bootstrapRelease = resolve })
    return Response.json(data.bootstrap)
  }
  if (path.endsWith('/rooms')) return Response.json(rooms())
  if (path.endsWith('/world-tickets')) return Response.json(ticket(await request.json() as WorldTicketRequest, ++serial))
  if (path.endsWith('/places')) return Response.json({ event_id: eventId, revision: 1, items: data.bootstrap.map_manifest.places })
  if (path.endsWith('/me')) return Response.json(participant(data, data.avatar))
  if (path.endsWith('/giveaways')) return Response.json({ items: gifts, next_cursor: null })
  if (path.includes('/giveaways/')) {
    if (giftMode === 'denied') return Response.json({ code: 'FORBIDDEN', message: 'PRIVATE_DIAGNOSTIC', details: { reason: 'PERMISSION_DENIED' } }, { status: 403 })
    const gift = gifts.find(item => path.endsWith(`/${item.giveaway_id}`))
    if (gift) return Response.json(gift)
  }
  return Response.json({ code: 'UNAVAILABLE', message: 'Synthetic producer has no resource', details: { reason: 'RESOURCE_UNAVAILABLE' } }, { status: 404 })
} })
class Socket extends MockSocket {
  constructor() { super(); sockets.push(this); peakSockets = Math.max(peakSockets, sockets.filter(socket => socket.readyState < 2).length); queueMicrotask(() => this.open()) }
  override send(text: string) {
    super.send(text); const message = JSON.parse(text)
    if (['authenticate', 'switchRoom', 'resync'].includes(message.type)) queueMicrotask(() => {
      if (this.readyState !== 1) return
      this.receive(JSON.stringify(frame(data, message.roomId ?? rooms().default_room_id, data.avatar,
        message.type === 'switchRoom' ? 'ROOM_SWITCH' : message.type === 'resync' ? 'RESYNC' : 'INITIAL', message.requestId ?? null)))
    })
  }
}
Object.defineProperty(window, 'WebSocket', { configurable: true, writable: true, value: Socket })
auth.start()
if (!params.has('anonymous')) {
  while (auth.read().capabilities === 'loading') await new Promise(resolve => setTimeout(resolve, 5))
  if (!await auth.login('fixture@example.test', 'fixture-password')) throw Error('Synthetic canonical login failed')
}
const initial = params.get('entry') ?? '/world'
if (params.has('foreign')) { history.replaceState({ foreign: true }, '', '/foreign-history-entry'); history.pushState(null, '', initial) }
else history.replaceState(null, '', initial)
// No load/assetFetch/importEngine/renderer/factory override and no panel overrides.
const ports = createProductionPorts(auth, { origin: location.origin, localInput: {
  revision: 'TEST_ONLY_FE07_not_movement_authority', reviewedBy: 'Synthetic shell journey',
  movementPolicy: { speedPxPerSecond: 96, footWidthPx: 12, footHeightPx: 8, interactionRadiusPx: 96 }, zoom: 1,
} })
const router = createAppRouter(ports), root = createRoot(document.getElementById('root')!)
root.render(<StrictMode><RouterProvider router={router} /></StrictMode>)
async function state() {
  if (document.querySelector('.shell-world-host canvas')) {
    const Phaser = (await import('phaser')).default
    const pool = Reflect.get(Phaser.Display.Canvas.CanvasPool, 'pool') as { canvas: HTMLCanvasElement; parent: PhaserType.Game | null }[]
    for (const entry of pool) {
      const game = entry.parent
      if (!game || game.canvas !== entry.canvas || !entry.canvas.closest('.shell-world-host') || knownGames.has(game)) continue
      knownGames.add(game); liveGames.add(game); peakGames = Math.max(peakGames, liveGames.size)
      if (!firstGame) firstGame = game
      game.events.once('destroy', () => liveGames.delete(game))
    }
  }
  const binding = ports.worldAccess?.read(), presentation = binding?.presentation?.read(), replication = binding?.replication.read()
  if (presentation && !firstCamera) firstCamera = presentation.camera
  return { path: router.state.location.pathname, search: router.state.location.search, action: router.state.historyAction,
    historyLength: history.length, auth: auth.read().readiness, bootstrapHeld: !!bootstrapRelease, requests,
    games: liveGames.size, knownGames: knownGames.size, peakGames, sameGame: [...liveGames][0] === firstGame,
    sockets: sockets.length, activeSockets: sockets.filter(socket => socket.readyState < 2).length, peakSockets,
    sameCamera: presentation ? presentation.camera === firstCamera : null, camera: presentation?.camera.read(),
    room: replication?.baseline?.roomId, position: replication?.entities.find(entity => entity.self)?.player.position,
    phase: replication?.phase, canvases: document.querySelectorAll('.shell-world-host canvas').length,
    inputTypes: sockets.flatMap(socket => socket.sent.map(text => JSON.parse(text).type as string)), userId,
  }
}
const api = { giftPath, state, navigate: (path: string) => router.navigate(path),
  releaseBootstrap() { bootstrapHold = false; bootstrapRelease?.(); bootstrapRelease = null },
  giftMode(value: string) { giftMode = value }, camera() { ports.worldAccess?.read()?.presentation?.read()?.camera.lookAt({ x: 250, y: 460 }) },
  async dispose() { root.unmount(); router.dispose(); ports.dispose?.(); auth.dispose(); await new Promise(resolve => setTimeout(resolve, 30)) },
}
Object.assign(window, { __FE07_JOURNEY__: api })
declare global { interface Window { __FE07_JOURNEY__: typeof api } }
