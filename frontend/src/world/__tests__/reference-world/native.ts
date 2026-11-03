import { illustratedReleases } from '../illustrated/fixtures'
import { createAssetResources, loadMap, type LoadedMap } from '../../map/resources'
import { sceneryResources } from '../../illustrated/model'
import { createWorldRuntime, type EngineLease } from '../../runtime'
import { loadPhaserEngine } from '../../scenes/MapScene'
import { attachMovement, type MovementAttachment } from '../../input/controller'
import type { MovementIntent } from '../../input/model'
const synthetic = new URLSearchParams(location.search).has('input')
const input = synthetic ? await import('../fixtures/movement-data').then(module => module.movementData()) : null
const { map, themes, avatar } = input ? { map: input.map, themes: [input.theme], avatar: input.pin.avatar } : await illustratedReleases()
const host = document.querySelector<HTMLElement>('#host')!, controls = document.querySelector<HTMLElement>('#controls')!
const resources = input ? { read: async (resource: { kind: string }) => resource.kind === 'png' ? input.png : input.tiledBytes } : createAssetResources({ origin: location.origin, allow: [{ path: map.tilemap.path!, sha256: map.tilemap.sha256!, kind: 'json' },
  ...themes.flatMap(theme => [{ path: theme.texture.path!, sha256: theme.texture.sha256!, kind: 'png' as const }, ...sceneryResources(theme)])] })
let engine!: EngineLease, movement!: MovementAttachment, current!: LoadedMap, mounts = 0, destroys = 0, era = input ? 0 : 1
let authoritativeFoot: { x: number; y: number } | null = null, failure = false
const intents: MovementIntent[] = []
const load = (index: number, signal: AbortSignal) => loadMap({ mapText: JSON.stringify(map), themeText: JSON.stringify(themes[index]),
  pin: { mapHash: map.content_hash, themeHash: themes[index]!.content_hash, avatar }, resources }, signal)
const runtime = createWorldRuntime({ host, load: async signal => current = await load(era, signal),
  importEngine: async () => { const factory = await loadPhaserEngine(); return (host, map) => {
    mounts++; engine = factory(host, map); const destroy = engine.destroy
    return { ...engine, destroy() { destroys++; destroy() } }
  } },
  attachControls(context) {
    // Same authority fence as composition: local movement alone cannot steer the scene.
    const camera = { ...context.camera, center(predicted: { x: number; y: number }, zoom: number) {
      const target = movement?.camera.read().following === false ? predicted : authoritativeFoot
      if (target) context.camera.center(target, zoom)
    } }
    movement = attachMovement({ ...context, camera }, { controls, zoom: 1,
      policy: { speedPxPerSecond: 96, footWidthPx: 12, footHeightPx: 8, interactionRadiusPx: 96 },
      consume(intent) { intents.push(intent); if (failure && (intent.axisX || intent.axisY)) throw Error('fixture input outage') } })
    return movement
  },
})
function pressed() { for (const button of document.querySelectorAll<HTMLElement>('[data-era]')) button.setAttribute('aria-pressed', String(Number(button.dataset.era) === era)) }
async function select(index: number) {
  let candidate!: LoadedMap
  await runtime.replaceSkin(async signal => candidate = await load(index, signal))
  current = candidate; era = index; pressed()
}
for (const button of document.querySelectorAll<HTMLElement>('[data-era]')) button.addEventListener('click', () => { void select(Number(button.dataset.era)) })
const panel = document.querySelector<HTMLElement>('#panel')!
document.querySelector('#pause')!.addEventListener('click', () => { runtime.setInputBlocked(true); panel.hidden = false; document.querySelector<HTMLButtonElement>('#resume')!.focus() })
document.querySelector('#resume')!.addEventListener('click', () => { panel.hidden = true; runtime.setInputBlocked(false); engine.camera!.surface()!.focus() })
const api = {
  state: () => ({ runtime: runtime.read(), movement: movement?.read(), intents, era, mounts, destroys, camera: engine.camera?.read(), scenery: engine.readScenery?.(),
    mapHash: current.map.content_hash, geometryHash: current.map.geometry_hash, themeHash: current.theme.content_hash, canvas: host.querySelectorAll('canvas').length }),
  select, blocked: (value: boolean) => runtime.setInputBlocked(value), fail: () => { failure = true },
  self(foot: { x: number; y: number }) { authoritativeFoot = foot; engine.camera!.center(foot, movement.camera.read().camera?.zoom ?? 1) },
  look(foot: { x: number; y: number }) { movement.camera.lookAt(foot) },
  project: (point: { x: number; y: number }) => engine.camera!.unproject(point),
  roundtrip: (point: { x: number; y: number }) => engine.camera!.project(point),
  async dispose() { runtime.dispose(); await runtime.settled(); return { canvas: host.querySelectorAll('canvas').length, controls: controls.children.length, scenery: engine.readScenery?.(), destroys } },
}
await runtime.start(); pressed(); Object.assign(window, { referenceWorld: api })
