// TEST ONLY QA05 application fault journey, adapted from published FE15 fixture at50d950d7/a84885f2.
// Actual app routes/WorldShell/composition/controller/default Phaser; explicit Auth/HTTP/C05 source fixture.
import { createRoot } from 'react-dom/client'
import { createMemoryRouter, RouterProvider } from 'react-router-dom'
import type PhaserType from 'phaser'
import { createAppRoutes } from '../../../../src/app/router'
import { createProductionPorts } from '../../../../src/app/ports'
import { createSessionController } from '../../../../src/features/auth/model/session'
import { capabilities, authResponse, isolatedCookiePolicy } from '../../../../src/features/auth/__tests__/fixtures/data'
import { createWorldComposition } from '../../../../src/world/composition'
import { createWholeCharacterWorldRenderer } from '../../../../src/world/avatar/whole-character'
import { isThemeId } from '../../../../src/features/theme/model/preferences'
import { referenceFixtureResources } from '../../../../src/world/reference-scene/__tests__/fixture'
import { spatialFixtureResources } from '../../../../src/world/reference-scene/__tests__/spatial-v3/fixture'
import { getReferenceOverviewMetadata, spatialThemeIds } from '../../../../src/world/reference-scene'
import { validateManifests } from '../../../../src/world/map/manifest'
import { sha256 } from '../../../../src/world/map/json'
import { eventId } from '../../../../src/world/composition/__tests__/fixtures/data'
import { rooms } from '../../../../src/app/world-entry/__tests__/data'
import { createRootSceneOverview, preflightReferenceScene } from '../../../../src/app/world-entry/reference-scene-overview'
import { sceneData, snapshot, localInput } from '../../../../src/app/world-entry/__tests__/reference-scene-root/data'
import '../../../../src/shared/styles/base.css'
import '../../../../src/app/shell.css'

import { validate } from '../../../../src/world/network/transport/contract'
import type { Snapshot } from '../../../../src/world/network/WorldTransport'

if (!import.meta.env.DEV) throw Error('QA05 fault journey is development-only')

const parameters = new URLSearchParams(location.search), initial = parameters.get('era')
const data = await sceneData(parameters.get('version') === 'legacy' ? 'legacy' : 'spatial', initial === 'REVENGE' || initial === 'DANGER' ? initial : 'PARADE', parameters.get('source') === 'derived'), requests: { path: string; method: string }[] = []
const caches: { diagnostics(): unknown; dispose(): void }[] = []
const sourceOnly = new URLSearchParams(location.search).get('mode') !== 'production-denied'
let disposed = false, factoryCalls = 0, canvasPeak = 0, firstCamera: unknown = null
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
  rendered: objects.filter(o => o.type === 'Sprite' && String(Reflect.get(o, 'texture')?.key).startsWith('mcr-avatar-')).map(o=>({ x: Reflect.get(o,'x'), y: Reflect.get(o,'y'), texture: Reflect.get(o,'texture')?.key })),
  referenceObjects: objects.filter(o => String(Reflect.get(o, 'texture')?.key).startsWith('mcr-reviewed-map-')).length }
}
const mutation = new MutationObserver(() => { canvasPeak = Math.max(canvasPeak, document.querySelectorAll('.shell-world-host canvas').length); void inspectWorld() })
mutation.observe(document.getElementById('root')!, { childList: true, subtree: true })
const peerId = parameters.get('peer') ?? 'qa05'
const initialFrame: Snapshot = validate<Snapshot>('Snapshot', snapshot(data, rooms().default_room_id))
await fetch('/__qa05/register?peer='+peerId, { method: 'POST', body: JSON.stringify({ snapshot: initialFrame }) })
const auth = createSessionController({ eventId, qualifiedRefresh: true, cookiePolicy: isolatedCookiePolicy(), fetch: async input => {
 const request = input as Request, path = new URL(request.url).pathname; requests.push({ path, method: request.method })
 if (path === '/api/auth/capabilities') return Response.json(capabilities())
 if (path === '/api/auth/login') return Response.json(authResponse())
 if (path === '/api/auth/logout') return new Response(null, { status: 204 })
 if (path.endsWith('/bootstrap')) return Response.json(data.bootstrap)
 if (path.endsWith('/rooms')) return Response.json(rooms())
 if (path.endsWith('/world-tickets')) return fetch('/__qa05/ticket?peer='+peerId, { method: 'POST', body: JSON.stringify(await request.json()), signal: request.signal })
 if (path.endsWith('/places')) return Response.json({ event_id: eventId, revision: 1, items: data.map.places })
 if (path.includes('/places/')) return Response.json({ event_id: eventId, revision: 1, place: data.map.places.find(p => p.place_id === path.split('/').at(-1)) })
 if (path.endsWith('/me')) return Response.json(data.bootstrap.participant)
 return Response.json({ code: 'RESOURCE_UNAVAILABLE', message: 'No source-fixture resource', details: { reason: 'RESOURCE_UNAVAILABLE' } }, { status: 404 })
} })
auth.start(); while (auth.read().capabilities === 'loading') await new Promise(resolve => setTimeout(resolve, 5))
if (!await auth.login('fixture@example.test', 'fixture-password')) throw Error('Fixture login failed')
const ports = createProductionPorts(auth, { origin: location.origin, localInput,
 ...(sourceOnly ? { load: async ({ auth: canonical, characters }: Parameters<NonNullable<import('../../../../src/app/world-entry/production').ProductionWorldOptions['load']>>[0]) => createWorldComposition({
  auth: canonical, createOverview: () => { factoryCalls++; return createRootSceneOverview() },
  resolveRelease: async request => {
   for (const theme of data.bootstrap.theme_manifests.filter(t => t.texture.image_state === 'PROVIDED')) {
    const metadata = await validateManifests(JSON.stringify(data.map), JSON.stringify(theme), { ...data.pin, themeHash: theme.content_hash })
    preflightReferenceScene(metadata)
   }
   const files = data.version === 'spatial' ? spatialFixtureResources(location.origin, { signal: request.signal })
    : referenceFixtureResources(data, location.origin, { signal: request.signal })
   caches.push(files); request.signal.addEventListener('abort', files.dispose, { once: true })
   const themeId = data.theme.theme_id
   if (!isThemeId(themeId)) throw Error('Invalid fixture era')
   // Only separately derived and validated descriptors carry the real published source qualification.
   return { bootstrap: request.bootstrap.document!, privacyScope: request.privacyScope, themeId, availableThemeIds: data.derived ? spatialThemeIds : [themeId],
    rooms: rooms(), roomId: rooms().default_room_id, resources: files.resources, movementPolicy: localInput.movementPolicy, zoom: localInput.zoom }
  },
  renderer: createWholeCharacterWorldRenderer({ resolveRuntime(context) {
   if (context.signal.aborted) throw Error('Expired source fixture')
   const runtime = characters?.read(canonical.privacy.capture(), data.avatar)
   if (!runtime) throw Error('Missing root character runtime'); return runtime
  } }),
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
  path: router.state.location.pathname, published: !!presentation, factoryCalls, requests, camera: presentation?.camera.read(), sameCamera: presentation ? presentation.camera === firstCamera : null,
  map: presentation?.map.expected, overview, theme: presentation?.theme.read(), metadata: overview ? getReferenceOverviewMetadata(overview) : null,
  sourceCaches: caches.map(c => c.diagnostics()), phase: view?.phase, entities: view?.entities.length,
  canvasPeak,
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
function inputState() {
 const presentation = ports.worldAccess?.read()?.presentation?.read(), view = ports.worldAccess?.read()?.replication.read()
 return { root: state(), self: view?.entities.find(e => e.self)?.player.position,
  projection: view ? { ...view, entities: view.entities.map(e=>({ ...e, player: { ...e.player, avatar: { revision: e.player.avatar.revision } } })) } : null, camera: presentation?.camera.read(),
  selected: (document.querySelector('.world-controls select') as HTMLSelectElement | null)?.value,
  controls: document.querySelectorAll('.world-controls').length,
  disabled: (document.querySelector('.world-joystick') as HTMLButtonElement | null)?.disabled,
  focused: document.activeElement?.getAttribute('aria-label'), hidden: document.hidden,
  viewport: { width: innerWidth, height: innerHeight, visualHeight: visualViewport?.height,
    shellHeight: (document.querySelector('.world-shell') as HTMLElement | null)?.style.getPropertyValue('--shell-viewport-height') } }
}
function point(world: { x: number; y: number }) {
 const game = firstGame!, camera = game.scene.getScene('mcr-map').cameras.main
 const origin = camera.getWorldPoint(0, 0), end = camera.getWorldPoint(game.canvas.width, game.canvas.height), rect = game.canvas.getBoundingClientRect()
 return { x: rect.left + (world.x - origin.x) / (end.x - origin.x) * rect.width, y: rect.top + (world.y - origin.y) / (end.y - origin.y) * rect.height }
}
const api = { state, inputState, point, lookAt: (point: { x: number; y: number }) => ports.worldAccess?.read()?.presentation?.read()?.camera.lookAt(point), inspectWorld, overviewPixels, navigate: (path: string) => router.navigate(path),
 suspend() { auth.privacy.changePolicy(auth.privacy.capture(), 'suspended') },
 async dispose() { if (disposed) return; disposed = true; root.unmount(); router.dispose(); ports.dispose?.(); auth.dispose(); caches.forEach(c => c.dispose()); mutation.disconnect(); URL.createObjectURL = createURL; URL.revokeObjectURL = revokeURL },
}
Object.assign(window, { __QA05_CLIENT__: api })
declare global { interface Window { __QA05_CLIENT__: typeof api } }
