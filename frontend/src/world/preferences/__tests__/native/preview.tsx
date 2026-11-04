import { preferencesSchema } from '../../../../features/me/model/contract'
// TEST ONLY: derived from app/world-entry/__tests__/reference-scene-v5/preview.tsx at c8a731e5.
// Same actual production ports/router/composition/default Phaser; controlled frozen C03/C05/C06 source fixture.
// Adds real ME preference PATCH behavior and independent actual renderer/tween observations; no product art or UI edits.
// TEST ONLY: explicit source resolver, real default MapScene/Phaser, actual root/Places providers.
import { createRoot } from 'react-dom/client'
import { createMemoryRouter, RouterProvider } from 'react-router-dom'
import type PhaserType from 'phaser'
import { createAppRoutes } from '../../../../app/router'
import { createProductionPorts } from '../../../../app/ports'
import { createSessionController } from '../../../../features/auth/model/session'
import { capabilities, authResponse, isolatedCookiePolicy } from '../../../../features/auth/__tests__/fixtures/data'
import { createWorldComposition, type WorldRendererPort } from '../../../composition'
import type { WorldCamera } from '../../../camera/port'
import { createWholeCharacterWorldRenderer } from '../../../avatar/whole-character'
import { isThemeId } from '../../../../features/theme/model/preferences'
import { spatialV5FixtureResources } from '../../../reference-scene/__tests__/spatial-v5/fixture'
import { getReferenceOverviewMetadata } from '../../../reference-scene'
import { validateManifests } from '../../../map/manifest'
import { sha256 } from '../../../map/json'
import { MockSocket } from '../../../network/mock'
import type { WorldTicketRequest } from '../../../network/WorldTransport'
import { eventId, ticket } from '../../../composition/__tests__/fixtures/data'
import { rooms } from '../../../../app/world-entry/__tests__/data'
import { createRootSceneOverview, preflightReferenceScene } from '../../../../app/world-entry/reference-scene-overview'
import { sceneData, snapshot, localInput } from '../../../../app/world-entry/__tests__/reference-scene-v5/data'
import '../../../../shared/styles/base.css'
import '../../../../app/shell.css'

const parameters = new URLSearchParams(location.search), initial = parameters.get('era')
const data = await sceneData(initial === 'REVENGE' || initial === 'DANGER' ? initial : 'PARADE'), requests: { path: string; method: string }[] = [], sockets: Socket[] = []
const caches: { diagnostics(): unknown; dispose(): void }[] = []
const sourceOnly = new URLSearchParams(location.search).get('mode') !== 'production-denied'
let participant = structuredClone(data.bootstrap.participant), preferenceWrites = 0
let disposed = false, factoryCalls = 0, socketPeak = 0, canvasPeak = 0, firstCamera: unknown = null
let firstGame: PhaserType.Game | null = null, peakGames = 0
const games = new Set<PhaserType.Game>(), known = new Set<PhaserType.Game>()
const urls = { created: [] as string[], revoked: [] as string[] }, createURL = URL.createObjectURL.bind(URL), revokeURL = URL.revokeObjectURL.bind(URL)
URL.createObjectURL = blob => { const value = createURL(blob); urls.created.push(value); return value }
URL.revokeObjectURL = url => { urls.revoked.push(url); revokeURL(url) }
async function inspectWorld() {
 const Phaser = (await import('phaser')).default
 const pool = Reflect.get(Phaser.Display.Canvas.CanvasPool, 'pool') as { parent: unknown; canvas: HTMLCanvasElement }[]
 for (const entry of pool) {
  const game = entry.parent as PhaserType.Game | null
  if (!game || game.canvas !== entry.canvas || !game.scene || !entry.canvas.closest('.shell-world-host')) continue
  if (!known.has(game)) { known.add(game); games.add(game); peakGames = Math.max(peakGames, games.size); game.events.once('destroy', () => games.delete(game)) }
 }
 const game = [...games][0]; if (game && !firstGame) firstGame = game
 const scene = game?.scene.getScenes(true).find(s => s.scene.key === 'mcr-map')
 const objects = scene?.children.list ?? []
 return { games: games.size, peakGames, totalGames: known.size, sameGame: game ? game === firstGame : null,
  sprites: objects.filter(o => o.type === 'Sprite' && String(Reflect.get(o, 'texture')?.key).startsWith('mcr-avatar-')).length,
  referenceObjects: objects.filter(o => String(Reflect.get(o, 'texture')?.key).startsWith('mcr-reviewed-map-')).length }
}
const mutation = new MutationObserver(() => { canvasPeak = Math.max(canvasPeak, document.querySelectorAll('.shell-world-host canvas').length); void inspectWorld() })
mutation.observe(document.getElementById('root')!, { childList: true, subtree: true })
const parkId = data.map.geometry.interactions.find(i => /park/.test(i.interaction_id))!.place_id
const catalogPlaces = () => data.map.places.map(p => parameters.get('markerStatus') === 'closed' && p.place_id === parkId ? { ...p, closure_state: 'CLOSED' as const } : p)
const auth = createSessionController({ eventId, qualifiedRefresh: true, cookiePolicy: isolatedCookiePolicy(), fetch: async input => {
 const request = input as Request, path = new URL(request.url).pathname; requests.push({ path, method: request.method })
 if (path === '/api/auth/capabilities') return Response.json(capabilities())
 if (path === '/api/auth/login') return Response.json(authResponse())
 if (path === '/api/auth/logout') return new Response(null, { status: 204 })
 if (path.endsWith('/bootstrap')) return Response.json(data.bootstrap)
 if (path.endsWith('/rooms')) return Response.json(rooms())
 if (path.endsWith('/world-tickets')) return Response.json(ticket(await request.json() as WorldTicketRequest, requests.length))
 if (path.endsWith('/places')) return Response.json({ event_id: eventId, revision: 1, items: parameters.get('catalog') === 'mismatch' ? data.map.places.map(p => ({ ...p, label: 'Synthetic mismatched catalog' })) : catalogPlaces() })
 if (path.includes('/places/')) {
  if (parameters.get('markerStatus') === 'unavailable' && path.endsWith(parkId)) return Response.json({ code: 'RESOURCE_UNAVAILABLE', message: 'Synthetic removed place', details: { reason: 'RESOURCE_UNAVAILABLE' } }, { status: 404 })
  return Response.json({ event_id: eventId, revision: 1, place: catalogPlaces().find(p => p.place_id === path.split('/').at(-1)) })
 }
 if (path.endsWith('/me')) {
  if (request.method === 'PATCH') {
   const body = await request.json()
   if (body.expected_revision !== participant.revision) throw Error('Unexpected controlled participant revision')
   participant = { ...participant, revision: participant.revision + 1, preferences: preferencesSchema.parse(body.preferences) }; preferenceWrites++
  }
  return Response.json(participant)
 }
 return Response.json({ code: 'RESOURCE_UNAVAILABLE', message: 'No source-fixture resource', details: { reason: 'RESOURCE_UNAVAILABLE' } }, { status: 404 })
} })
class Socket extends MockSocket {
 constructor() { super(); sockets.push(this); socketPeak = Math.max(socketPeak, sockets.filter(s => s.readyState < 2).length); queueMicrotask(() => this.open()) }
 override send(text: string) {
  super.send(text); const message = JSON.parse(text)
  if (message.type === 'authenticate') queueMicrotask(() => { if (this.readyState === 1) this.receive(JSON.stringify(snapshot(data, message.roomId))) })
 }
}
Object.defineProperty(window, 'WebSocket', { configurable: true, writable: true, value: Socket })
auth.start(); while (auth.read().capabilities === 'loading') await new Promise(resolve => setTimeout(resolve, 5))
if (!await auth.login('fixture@example.test', 'fixture-password')) throw Error('Fixture login failed')
let projectionCamera: WorldCamera | null = null
function captureCamera(renderer: WorldRendererPort): WorldRendererPort {
 return { mount(context) { projectionCamera = context.camera; return renderer.mount(context) } }
}
const ports = createProductionPorts(auth, { origin: location.origin, localInput,
 ...(sourceOnly ? { load: async ({ auth: canonical, characters }: Parameters<NonNullable<import('../../../../app/world-entry/production').ProductionWorldOptions['load']>>[0]) => createWorldComposition({
  auth: canonical, createOverview: () => { factoryCalls++; return createRootSceneOverview() },
  resolveRelease: async request => {
   for (const theme of data.bootstrap.theme_manifests.filter(t => t.texture.image_state === 'PROVIDED')) {
    const metadata = await validateManifests(JSON.stringify(data.map), JSON.stringify(theme), { ...data.pin, themeHash: theme.content_hash })
    preflightReferenceScene(metadata)
   }
   const files = spatialV5FixtureResources(location.origin, { signal: request.signal })
   caches.push(files); request.signal.addEventListener('abort', files.dispose, { once: true })
   const themeId = data.theme.theme_id
   if (!isThemeId(themeId)) throw Error('Invalid fixture era')
   // Original UNREVIEWED source only; optional eras remain disabled pending genuine producer qualification.
   return { bootstrap: request.bootstrap.document!, privacyScope: request.privacyScope, themeId, availableThemeIds: [themeId],
    rooms: rooms(), roomId: rooms().default_room_id, resources: files.resources, movementPolicy: localInput.movementPolicy, zoom: localInput.zoom }
  },
  renderer: captureCamera(createWholeCharacterWorldRenderer({ resolveRuntime(context) {
   if (context.signal.aborted) throw Error('Expired source fixture')
   const runtime = characters?.read(canonical.privacy.capture(), data.avatar)
   if (!runtime) throw Error('Missing root character runtime'); return runtime
  } })),
 }) } : {}),
})
const router = createMemoryRouter(createAppRoutes(ports), { initialEntries: ['/world'] }), root = createRoot(document.getElementById('root')!)
root.render(<RouterProvider router={router} />)
function state() {
 const binding = ports.worldAccess?.read(), presentation = binding?.presentation?.read(), view = binding?.replication.read()
 if (presentation && !firstCamera) firstCamera = presentation.camera
 const runtime = ports.characters?.read(auth.privacy.capture(), data.avatar)
 const overview = presentation?.overview?.read()
 return { version: data.version, derived: data.derived, originalThemeProvenance: data.originalThemeProvenance,
  suppliedThemeProvenance: data.bootstrap.theme_manifests.map(t => t.provenance), sourceOnly, productionAdmitted: false, originalMapProvenance: data.map.provenance, originalPlaces: data.map.places,
  suppliedThemePins: data.bootstrap.theme_manifests.map(t => ({ themeId: t.theme_id, hash: t.content_hash, mapHash: t.map_content_hash, avatarHash: t.avatar_content_hash })),
  framesByType: sockets.flatMap(socket => socket.sent.map(text => JSON.parse(text).type as string)).reduce((counts, type) => ({ ...counts, [type]: (counts[type] ?? 0) + 1 }), {} as Record<string, number>), preferenceWrites, savedPreferences: participant.preferences, path: router.state.location.pathname, published: !!presentation, factoryCalls, requests, camera: presentation?.camera.read(), sameCamera: presentation ? presentation.camera === firstCamera : null,
  map: presentation?.map.expected, overview, theme: presentation?.theme.read(), metadata: overview ? getReferenceOverviewMetadata(overview) : null,
  sourceCaches: caches.map(c => c.diagnostics()), phase: view?.phase, entities: view?.entities.length,
  socketPeak, sockets: sockets.length, activeSockets: sockets.filter(s => s.readyState < 2).length, canvasPeak,
  canvases: document.querySelectorAll('.shell-world-host canvas').length, runtime: runtime?.diagnostics(), urls: structuredClone(urls), disposed }
}
async function overviewPixels() {
 const value = ports.worldAccess?.read()?.presentation?.read()?.overview?.read(); if (!value) return null
 const response = await fetch(value.imageUrl), bytes = new Uint8Array(await response.arrayBuffer())
 const image = new Image(); image.src = value.imageUrl; await image.decode()
 const canvas = document.createElement('canvas'); canvas.width = image.naturalWidth; canvas.height = image.naturalHeight
 const context = canvas.getContext('2d')!; context.drawImage(image, 0, 0)
 const rgba = new Uint8Array(context.getImageData(0, 0, canvas.width, canvas.height).data.buffer)
 const result = { width: canvas.width, height: canvas.height, bytes: bytes.byteLength, pngHash: await sha256(bytes), rgbaHash: await sha256(rgba) }; canvas.width = canvas.height = 0; return result
}
function cameraProjection() {
 const binding = ports.worldAccess?.read(), camera = projectionCamera
 const self = binding?.replication.read()?.entities.find(e => e.self)?.position
 const surface = document.querySelector('.shell-world-host canvas')?.getBoundingClientRect()
 if (!camera || !surface || !self) throw Error('Camera or self unavailable')
 const points = [{ x: 160, y: 13 }, { x: 744, y: 13 }, { x: 160, y: 597 }, { x: 744, y: 597 }, { x: 160.1, y: 13.1 }, { x: 743.9, y: 596.9 }, self]
 return { self, surface: { left: surface.left, top: surface.top, right: surface.right, bottom: surface.bottom },
  points: points.map(world => { const client = camera.unproject(world); return { world, client, roundtrip: client ? camera.project(client) : null } }) }
}
let probe: Awaited<ReturnType<typeof attachProbe>> | null = null
let borrowed: AbortController | null = null
async function attachProbe() {
 const Phaser = (await import('phaser')).default
 const pool = Reflect.get(Phaser.Display.Canvas.CanvasPool, 'pool') as { parent: PhaserType.Game; canvas: HTMLCanvasElement }[]
 const game = pool.find(entry => entry.parent?.canvas === entry.canvas && entry.canvas.closest('.shell-world-host'))?.parent
 if (!game || !(game.renderer instanceof Phaser.Renderer.Canvas.CanvasRenderer)) throw Error('Actual incumbent Canvas Phaser unavailable')
 const renderer = game.renderer, scene = game.scene.getScene('mcr-map'), camera = scene.cameras.main
 const originals = { pre: renderer.preRender, render: renderer.render, post: renderer.postRender, callback: game.loop.callback }
 const counts = { updates: 0, pre: 0, render: 0, post: 0 }
 const update = () => { counts.updates++ }, pre = () => { counts.pre++ }, render = () => { counts.render++ }, post = () => { counts.post++ }
 scene.events.on(Phaser.Scenes.Events.UPDATE, update)
 renderer.on(Phaser.Renderer.Events.PRE_RENDER, pre); renderer.on(Phaser.Renderer.Events.RENDER, render); renderer.on(Phaser.Renderer.Events.POST_RENDER, post)
 // Plain data target only: proves the actual TweenManager without changing any product pixels or characters.
 const shell = document.querySelector<HTMLElement>('[data-mcr-shell]')
 if (!shell) throw Error('Actual unique shell absent')
 const motionProbe = document.createElement('span'); motionProbe.hidden = true; motionProbe.style.transition = 'opacity 120ms'; shell.append(motionProbe)
 const model = { value: 0 }, tween = scene.tweens.add({ targets: model, value: 1000, duration: 100000 })
 return { read: () => ({ counts: { ...counts }, tweenValue: model.value, tweenScale: scene.tweens.timeScale,
   shellMotionAttribute: shell.getAttribute('data-mcr-reduced-motion'), shellConnected: shell.isConnected, cssProbeTransition: getComputedStyle(motionProbe).transitionDuration,
   normalMethods: renderer.preRender === originals.pre && renderer.render === originals.render && renderer.postRender === originals.post,
   sameLoopCallback: game.loop.callback === originals.callback, sameCamera: scene.cameras.main === camera,
   camera: { x: camera.scrollX, y: camera.scrollY, zoom: camera.zoom }, noAudio: game.config.audio.noAudio === true,
   audioElements: document.querySelectorAll('audio').length }),
  dispose() { motionProbe.remove(); try { tween.remove() } catch { /* The incumbent scene may already have destroyed its tween. */ }; scene.events.off(Phaser.Scenes.Events.UPDATE, update); renderer.off(Phaser.Renderer.Events.PRE_RENDER, pre); renderer.off(Phaser.Renderer.Events.RENDER, render); renderer.off(Phaser.Renderer.Events.POST_RENDER, post) } }
}
const api = { state, inspectWorld, overviewPixels, cameraProjection,
 heartbeat() { const socket = sockets.at(-1); if (!socket || socket.readyState !== 1) throw Error('Controlled server socket unavailable'); socket.receive(JSON.stringify({ protocolVersion: '1.0.0', type: 'ping', heartbeatId: crypto.randomUUID() })) },
 async beginProbe() { if (!probe) probe = await attachProbe(); return probe.read() },
 engine() { if (!probe) throw Error('Probe not attached'); return probe.read() },
 async borrowEffects() {
  borrowed?.abort(); borrowed = new AbortController()
  const effects = ports.worldAccess?.read()?.presentation?.read()?.runtimePreferences?.effects
  if (!effects) throw Error('Actual production preference binding absent')
  await effects.acquire({ reducedMotion: true, lowPower: true }, borrowed.signal)
 },
 abortBorrowed() { borrowed?.abort(); borrowed = null; return probe?.read() },
 async selectUnavailableTheme() {
  const theme = ports.worldAccess?.read()?.presentation?.read()?.theme
  if (!theme) throw Error('Actual production theme missing')
  await theme.select('DANGER'); return theme.read()
 },
 prepareCatalogRecovery() { const url = new URL(location.href); url.searchParams.delete('catalog'); history.replaceState(history.state, '', url) }, navigate: (path: string) => router.navigate(path),
 suspend() { auth.privacy.changePolicy(auth.privacy.capture(), 'suspended') },
 // Keep document-local URL probes through asynchronous teardown; closing the browser context releases them.
 async dispose() { if (disposed) return; disposed = true; borrowed?.abort(); probe?.dispose(); root.unmount(); router.dispose(); ports.dispose?.(); auth.dispose(); caches.forEach(c => c.dispose()); mutation.disconnect() },
}
Object.assign(window, { __WORLD_PREFERENCES_ROOT__: api })
window.addEventListener('pagehide', () => { void api.dispose() }, { once: true })
declare global { interface Window { __WORLD_PREFERENCES_ROOT__: typeof api } }
