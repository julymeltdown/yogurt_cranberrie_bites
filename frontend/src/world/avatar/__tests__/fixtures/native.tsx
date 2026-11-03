// SYNTHETIC TEST ART ONLY. Real existing Phaser + canonical Auth + FE16/17 + composed pixels.
import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { createBrowserRouter, RouterProvider } from 'react-router-dom'
import Phaser from 'phaser'
import { createAppRoutes } from '../../../../app/router'
import { createProductionPorts } from '../../../../app/ports'
import { createSessionController } from '../../../../features/auth/model/session'
import { isolatedCookiePolicy } from '../../../../features/auth/__tests__/fixtures/data'
import { validate } from '../../../network/transport/contract'
import type { WorldRooms, AvatarConfig } from '../../../network/WorldTransport'
import { loadPhaserEngine } from '../../../scenes/MapScene'
import { createWorldComposition } from '../../../composition'
import { eventId, resolverFor } from '../../../composition/__tests__/fixtures/data'
import { createAssetResources } from '../../../map/resources'
import { canonicalJson } from '../../../map/json'
import { validateManifest, type Pose } from '../../../../entities/avatar/runtime/contract'
import { loadAtlas } from '../../../../entities/avatar/runtime/resources'
import { createAvatarCompositor, type AvatarCompositor } from '../../../../entities/avatar/runtime/compositor'
import { createAvatarRenderer } from '../../renderer'
import type { AvatarScenePort } from '../../scene'
import { testData } from './data'
import '../../../../shared/styles/base.css'
import '../../../../app/shell.css'

const nativeFetch = globalThis.fetch.bind(globalThis)
const data = await testData(new Uint8Array(await (await nativeFetch('/world/fe12-test/avatar.png')).arrayBuffer()))
const auth = createSessionController({ eventId, cookiePolicy: isolatedCookiePolicy(), fetch: nativeFetch })
auth.start()
await new Promise<void>(resolve => { const stop = auth.subscribe(() => { if (auth.read().capabilities !== 'loading') { stop(); resolve() } }) })
if (!await auth.login('fixture@example.test', 'fixture-password')) throw new Error('Canonical test login failed')
const assets = createAssetResources({ origin: location.origin, allow: data.allow })
const checkedManifest = await validateManifest(canonicalJson(data.bootstrap.avatar_manifest), data.bootstrap.avatar_manifest)
const corrupt = data.avatarPng.slice(); corrupt[corrupt.length - 1]! ^= 1
let hashRejected = false
try { await loadAtlas(checkedManifest, { read: async () => corrupt }, new AbortController().signal) }
catch (error) { hashRejected = error instanceof Error && error.message === 'Avatar invalid' }
let compositor: AvatarCompositor | null = null, scene: AvatarScenePort | null = null, game: Phaser.Game | null = null, atlasDisposals = 0
const stats = { games: 0, peakGames: 0, mounts: 0, disposals: 0 }
const renderer = createAvatarRenderer({ async load(document, signal) {
  const manifest = await validateManifest(canonicalJson(document), data.bootstrap.avatar_manifest)
  const loaded = await loadAtlas(manifest, assets, signal)
  // Explicit fixture-only policy. Production createReviewedAvatarRenderer rejects these unreviewed test assets.
  compositor = createAvatarCompositor({ manifest, image: loaded.image, bakedSkinToneId: 'tone_fixture',
    disposeAtlas() { atlasDisposals++; loaded.dispose() } })
  return compositor
} })
const composition = createWorldComposition({ auth, renderer, resolveRelease: async request => {
  const rooms = await auth.request({ path: `/api/concert/events/${eventId}/rooms`, method: 'GET', access: 'protected', errorFamily: 'core', signal: request.signal,
    decode: value => validate<WorldRooms>('WorldRooms', value) })
  return { ...await resolverFor(auth, data, nativeFetch)(), privacyScope: request.privacyScope, bootstrap: request.bootstrap.document, rooms: rooms.data }
}, importEngine: async () => {
  const factory = await loadPhaserEngine()
  return (host, map) => {
    const prototype = Phaser.Game.prototype as unknown as { start(this: Phaser.Game): void }, start = prototype.start
    const captureGame = (value: Phaser.Game) => { game = value }
    // Fixture-only read instrumentation captures the actual engine, without creating one.
    prototype.start = function() { captureGame(this); return start.call(this) }
    const engine = factory(host, map); scene = engine.avatarScene ?? null
    stats.games++; stats.mounts++; stats.peakGames = Math.max(stats.games, stats.peakGames)
    void engine.ready.finally(() => { prototype.start = start })
    let disposed = false
    return { ...engine, destroy() { if (disposed) return; disposed = true; stats.games--; stats.disposals++; engine.destroy() } }
  }
} })
history.replaceState(null, '', '/world')
const router = createBrowserRouter(createAppRoutes({ ...createProductionPorts(auth), world: composition })), root = createRoot(document.getElementById('root')!)
const sprites = () => game?.scene?.getScene('mcr-map')?.children?.list.filter(object => object.type === 'Sprite') as Phaser.GameObjects.Sprite[] | undefined ?? []
const pixels = (canvas: HTMLCanvasElement, x = 0, y = 0) => canvas.getContext('2d')!.getImageData(x, y, 64, 64).data
const fixture = {
  state: () => ({ ...stats, renderer: renderer.read(), composition: composition.read(), projection: composition.replication.read(),
    scene: scene?.read(), compositor: compositor?.read(), atlasDisposals, hashRejected, actualSprites: sprites().length,
    spriteFrames: sprites().map(sprite => ({ x: sprite.x, y: sprite.y, frame: Number(sprite.frame.name) })),
    actualAvatarTextures: Object.keys(game?.textures?.list ?? {}).filter(key => key.startsWith('mcr-avatar-')).length }),
  pixelProof() {
    const entities = composition.replication.read()!.entities, sprite = sprites().find(sprite => sprite.x === entities[0]!.position.x && sprite.y === entities[0]!.position.y)!
    const config = entities[0]!.player.avatar.config, source = sprite.texture.getSourceImage() as HTMLCanvasElement
    let equal = true, golden = true
    for (const [row, direction] of (['down', 'left', 'right', 'up'] as const).entries()) for (let col = 0; col < 5; col++) {
      const pose: Pose = { direction, animation: col ? 'walk' : 'idle', frame: col ? col - 1 : 0 }
      const thumbnail = compositor!.thumbnail(config, pose), a = pixels(thumbnail.canvas), b = pixels(source, col * 64, row * 64)
      equal &&= a.every((v, i) => v === b[i])
      const color = (x: number, y: number) => a.slice((y * 64 + x) * 4, (y * 64 + x) * 4 + 4).join(',')
      golden &&= color(0, 0) === [20 + row * 40, 20 + col * 40, 150, 255].join(',')
        && color(29, 23) === '50,180,70,255' && color(25, 40) === '230,190,40,255'
        && color(40, 32) === '180,70,210,255' && color(1, 10) === '0,0,0,0'
      if (row === 0 && col === 0) { document.getElementById('thumbnail')!.replaceChildren(thumbnail.canvas) }
      else thumbnail.dispose()
    }
    const foot = composition.replication.read()!.entities[0]!.position
    const frame = Number(sprite.frame.name)
    return { equalAll20Frames: equal, goldenOrderAndMarkers: golden, frame,
      anchor: { x: sprite.originX, y: sprite.originY }, authoritativePosition: sprite.x === foot.x && sprite.y === foot.y,
      sourceWidth: source.width, sourceHeight: source.height, config: config as AvatarConfig }
  },
  revoke() { auth.privacy.changePolicy(auth.privacy.capture(), 'suspended'); return fixture.state() },
  async dispose() { root.unmount(); router.dispose(); composition.dispose(); auth.dispose(); await composition.settled(); return fixture.state() },
}
Object.assign(window, { __FE12_TEST_ONLY__: fixture })
root.render(<StrictMode><RouterProvider router={router} /></StrictMode>)
declare global { interface Window { __FE12_TEST_ONLY__: typeof fixture } }
