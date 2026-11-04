// TEST ONLY. Current default Auth/production ports/resolver/renderer with synthetic HTTP/C05 authority.
// Canonical public application modules, original frozen source inputs, synthetic HTTP/C05 authority.
import { createRoot } from 'react-dom/client'
import { createMemoryRouter, RouterProvider } from 'react-router-dom'
import { createAppRoutes } from '../../src/app/router'
import { createProductionPorts } from '../../src/app/ports'
import { createSessionController } from '../../src/features/auth/model/session'
import { capabilities, authResponse, isolatedCookiePolicy } from '../../src/features/auth/__tests__/fixtures/data'
import { MockSocket } from '../../src/world/network/mock'
import { ticket } from '../../src/world/composition/__tests__/fixtures/data'
import { createCollision } from '../../src/world/input/simulation'
import { STEP_MS } from '../../src/world/input/model'
import { validate } from '../../src/world/network/transport/contract'
import type { Input, Snapshot, WorldTicketRequest } from '../../src/world/network/WorldTransport'
import { bootstrap, eventId, userId, snapshot, rooms, localInput } from './current-data'
import '../../src/shared/styles/base.css'
import '../../src/app/shell.css'

if (!import.meta.env.DEV && import.meta.env.MODE !== 'qa08') throw Error('QA08 requires its explicit isolated fixture mode')
const data = { bootstrap, map: bootstrap.map_manifest, avatar: bootstrap.avatar_manifest }
const objectUrls = { created: 0, revoked: 0, active: new Set<string>() }
const createUrl = URL.createObjectURL.bind(URL), revokeUrl = URL.revokeObjectURL.bind(URL)
URL.createObjectURL = value => { const url = createUrl(value); objectUrls.created++; objectUrls.active.add(url); return url }
URL.revokeObjectURL = url => { if (objectUrls.active.delete(url)) objectUrls.revoked++; revokeUrl(url) }
const requests: { path: string; method: string }[] = [], sockets: Socket[] = []
const collision = createCollision(data.map.geometry, localInput.movementPolicy)
let consumerDelayMs = 0, peakSockets = 0, disposed = false, pendingPeak = 0, dropRemaining = 0, droppedDeltas = 0, resyncs = 0
const losses: { at: number; inputSeq: number; streamSeq: number }[] = []
const delivery: { queuedAt: number; deliveredAt: number; inputSeq: number; serviceIntervalMs: number }[] = []
const phases: { at: number; phase: string; pending: number }[] = []
const auth = createSessionController({ eventId, qualifiedRefresh: true, cookiePolicy: isolatedCookiePolicy(), fetch: async input => {
  const request = input as Request, path = new URL(request.url).pathname; requests.push({ path, method: request.method })
  if (path === '/api/auth/capabilities') return Response.json(capabilities())
  if (path === '/api/auth/login') return Response.json({ ...authResponse(), userId })
  if (path === '/api/auth/logout') return new Response(null, { status: 204 })
  if (path.endsWith('/bootstrap')) return Response.json(data.bootstrap)
  if (path.endsWith('/rooms')) return Response.json(rooms())
  if (path.endsWith('/world-tickets')) return Response.json({ ...ticket(await request.json() as WorldTicketRequest, requests.length),
    ticket: crypto.randomUUID().replaceAll('-', '').repeat(2).slice(0, 43), event_id: eventId })
  if (path.endsWith('/places')) return Response.json({ event_id: eventId, revision: 1, items: data.map.places })
  if (path.includes('/places/')) return Response.json({ event_id: eventId, revision: 1, place: data.map.places.find(p => p.place_id === path.split('/').at(-1)) })
  if (path.endsWith('/me')) return Response.json(data.bootstrap.participant)
  return Response.json({ code: 'RESOURCE_UNAVAILABLE', message: 'QA08 fixture has no such resource', details: { reason: 'RESOURCE_UNAVAILABLE' } }, { status: 404 })
} })
class Socket extends MockSocket {
  last: Snapshot | null = null
  sequence = 0
  inputs = 0
  peakQueue = 0
  queue: { text: string; at: number; seq: number; serviceIntervalMs: number }[] = []
  timer: ReturnType<typeof setTimeout> | null = null
  heartbeat: ReturnType<typeof setInterval> | null = null
  constructor() { super(); sockets.push(this); peakSockets = Math.max(peakSockets, sockets.filter(s => s.readyState < 2).length); queueMicrotask(() => { if (this.readyState === 0) this.open() }) }
  drain() {
    if (this.timer !== null || !this.queue.length || this.readyState !== 1) return
    this.timer = setTimeout(() => {
      this.timer = null; const item = this.queue.shift()
      if (item && this.readyState === 1) {
        if (dropRemaining > 0) { dropRemaining--; droppedDeltas++; losses.push({ at: performance.now(), inputSeq: item.seq, streamSeq: JSON.parse(item.text).streamSeq }) }
        else { delivery.push({ queuedAt: item.at, deliveredAt: performance.now(), inputSeq: item.seq, serviceIntervalMs: item.serviceIntervalMs }); this.receive(item.text) }
      }
      this.drain()
    }, consumerDelayMs)
  }
  override send(text: string) {
    super.send(text); const message = JSON.parse(text)
    if (message.type === 'authenticate') {
      const initial = snapshot(message.roomId)
      this.last = initial
      queueMicrotask(() => { if (this.readyState === 1) this.receive(JSON.stringify(initial)) })
      this.heartbeat = setInterval(() => { if (this.readyState === 1) this.receive(JSON.stringify({ protocolVersion: '1.0.0', type: 'ping', heartbeatId: crypto.randomUUID() })) }, 10_000)
    }
    if (message.type === 'resync' && this.last) {
      resyncs++; this.clearQueue()
      this.last = validate<Snapshot>('Snapshot', { ...this.last, cause: 'RESYNC', requestId: message.requestId,
        streamId: crypto.randomUUID(), streamSeq: 0, serverTick: this.last.serverTick + this.sequence, ackInputSeq: 0 })
      this.sequence = 0
      queueMicrotask(() => { if (this.readyState === 1) this.receive(JSON.stringify(this.last)) })
    }
    if (message.type === 'input' && this.last) {
      const input = validate<Input>('Input', message); this.inputs++
      const self = this.last.players.find(p => p.userId === this.last!.selfUserId)!
      const player = { ...self, position: collision.move(self.position, { x: input.axisX, y: input.axisY }, STEP_MS / 1000), moving: input.axisX !== 0 || input.axisY !== 0 }
      this.last = { ...this.last, players: this.last.players.map(p => p.userId === player.userId ? player : p) }
      const delta = validate('Delta', { protocolVersion: '1.0.0', type: 'delta', streamId: this.last.streamId, roomEpoch: this.last.roomEpoch,
        streamSeq: ++this.sequence, serverTick: this.last.serverTick + this.sequence, ackInputSeq: input.seq, upserts: [player], removes: [] })
      this.queue.push({ text: JSON.stringify(delta), at: performance.now(), seq: input.seq, serviceIntervalMs: consumerDelayMs }); this.peakQueue = Math.max(this.peakQueue, this.queue.length); this.drain()
    }
  }
  clearQueue() { if (this.timer !== null) clearTimeout(this.timer); this.timer = null; this.queue = [] }
  clear() { this.clearQueue(); if (this.heartbeat !== null) clearInterval(this.heartbeat); this.heartbeat = null }
  override close() { this.clear(); super.close() }
  override serverClose(code = 4500) { this.clear(); super.serverClose(code) }
}
Object.defineProperty(window, 'WebSocket', { configurable: true, writable: true, value: Socket })
auth.start(); while (auth.read().capabilities === 'loading') await new Promise(resolve => setTimeout(resolve, 5))
if (!await auth.login('fixture@example.test', 'fixture-password')) throw Error('QA08 fixture login failed')
const ports = createProductionPorts(auth, { origin: location.origin, localInput }, undefined, eventId)
const router = createMemoryRouter(createAppRoutes(ports), { initialEntries: ['/world'] }), root = createRoot(document.getElementById('root')!)
root.render(<RouterProvider router={router} />)
const world = () => ports.worldAccess?.read()?.replication.read()
let stopReplication = () => {}
const observe = () => {
  const view = world(); if (!view) return
  pendingPeak = Math.max(pendingPeak, view.pendingInputs)
  if (phases.at(-1)?.phase !== view.phase) phases.push({ at: performance.now(), phase: view.phase, pending: view.pendingInputs })
}
const bind = () => { stopReplication(); stopReplication = ports.worldAccess?.read()?.replication.subscribe(observe) ?? (() => {}); observe() }
const stop = ports.worldAccess?.subscribe(bind); bind()
const P = (await import('phaser')).default
function game() {
  const pool = Reflect.get(P.Display.Canvas.CanvasPool, 'pool') as { parent: import('phaser').Game; canvas: HTMLCanvasElement }[]
  return pool.find(e => e.parent?.canvas === e.canvas && e.canvas.closest('.shell-world-host'))?.parent
}
let firstGame: WeakRef<import('phaser').Game> | null = null, gameCount = 0
const known = new WeakSet<import('phaser').Game>()
const api = {
  read() {
    const value = game(); if (value) { if (!known.has(value)) { known.add(value); gameCount++ }; firstGame ??= new WeakRef(value) }
    const view = world(), presentation = ports.worldAccess?.read()?.presentation?.read()
    return { phase: view?.phase, path: router.state.location.pathname, pending: view?.pendingInputs, ack: view?.ackInputSeq, pendingPeak,
      entities: view?.entities.length, baseline: view?.baseline, phases, sockets: sockets.length, activeSockets: sockets.filter(s => s.readyState < 2).length, peakSockets,
      receivedInputCount: sockets.reduce((n, s) => n + s.inputs, 0), injectorQueued: sockets.reduce((n, s) => n + s.queue.length, 0), injectorPeak: Math.max(0, ...sockets.map(s => s.peakQueue)),
      delivery, consumerDelayMs, droppedDeltas, dropRemaining, losses, resyncs,
      fixtureTimers: sockets.filter(s => s.timer !== null).length, heartbeatTimers: sockets.filter(s => s.heartbeat !== null).length,
      socketListeners: sockets.reduce((n, s) => n + [...s.listeners.values()].reduce((a, listeners) => a + listeners.size, 0), 0),
      canonicalIdentity: { userId: auth.session.read().userId, eventId: auth.session.read().eventId, generation: auth.session.read().generation },
      games: value ? 1 : 0, totalObservedGames: gameCount, sameGame: value ? value === firstGame?.deref() : null,
      gameLoopRunning: value?.loop.running, renderer: value?.renderer.type === P.WEBGL ? 'WEBGL' : value?.renderer.type === P.CANVAS ? 'CANVAS' : null,
      avatarSprites: value?.scene.getScenes(true).flatMap(s => s.children.list).filter(o => o.type === 'Sprite' && String(Reflect.get(o, 'texture')?.key).startsWith('mcr-avatar-')).length ?? 0,
      canvases: document.querySelectorAll('.shell-world-host canvas').length, cacheDiagnostics: 'Default production resolver cache internals are not exposed', objectUrls: { created: objectUrls.created, revoked: objectUrls.revoked, active: objectUrls.active.size },
      urlsInDOM: document.querySelectorAll('[src^="blob:"]').length, map: presentation?.map.expected,
      overviewPresentation: presentation?.overview?.read()?.presentationVersion ?? null, requests, disposed }
  },
  consumer(delayMs: number) { consumerDelayMs = delayMs },
  dropNext(count: number) { if (!Number.isSafeInteger(count) || count < 0 || count > 3) throw Error('Bounded fixture delta loss only'); dropRemaining = count },
  async navigate(path: '/' | '/world' | '/world/map') { await router.navigate(path) },
  markLifetime() { firstGame = null },
  disconnect() { const at = performance.now(); sockets.at(-1)?.serverClose(4500); return at },
  async sample(ms: number) {
    const value = game(); if (!value) throw Error('No incumbent Phaser game')
    const raf: number[] = [], renders: number[] = [], start = performance.now()
    let id = 0
    const tick = (at: number) => { raf.push(at); id = requestAnimationFrame(tick) }
    const render = () => { renders.push(performance.now()) }
    value.events.on(P.Core.Events.POST_RENDER, render); id = requestAnimationFrame(tick)
    await new Promise(resolve => setTimeout(resolve, ms))
    cancelAnimationFrame(id); value.events.off(P.Core.Events.POST_RENDER, render)
    return { start, end: performance.now(), requestedMs: ms, raf, renders, visibility: document.visibilityState, state: api.read() }
  },
  async dispose() { if (disposed) return; disposed = true; stop?.(); stopReplication(); root.unmount(); router.dispose(); ports.dispose?.(); auth.dispose(); sockets.forEach(s => s.clear()) },
}
Object.assign(window, { __QA08__: api })
declare global { interface Window { __QA08__: typeof api } }
