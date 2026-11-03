import manifest from '../../../../../../contracts/fixtures/assets/selected-static/avatar-manifest.json'
import { createWholeCharacterRuntime } from '../../../../entities/avatar/whole-character'
import { createSessionController } from '../../../../features/auth/model/session'
import { isolatedCookiePolicy } from '../../../../features/auth/__tests__/fixtures/data'
import { illustratedReleases } from '../../../__tests__/illustrated/fixtures'
import { sceneryResources } from '../../../illustrated/model'
import { contentHash, sha256 } from '../../../map/json'
import { createAssetResources, loadMap, type LoadedMap } from '../../../map/resources'
import { createWorldRuntime, type EngineLease } from '../../../runtime'
import { loadPhaserEngine } from '../../../scenes/MapScene'
import { createWholeCharacterWorldRenderer } from '../../whole-character'
import { validate } from '../../../network/transport/contract'
import type { Snapshot } from '../../../network/WorldTransport'
import type { ReplicationView } from '../../../network/replication'
const eventId = '00000000-0000-4000-8000-000000000002', userId = '00000000-0000-4000-8000-000000000001', roomId = '00000000-0000-4000-8000-000000000003'
const auth = createSessionController({ eventId, cookiePolicy: isolatedCookiePolicy(), fetch: async () => { throw Error('No Auth network in isolated rendering fixture') } })
auth.session.replace(auth.session.read().generation, { userId, eventId, accessToken: 'TEST_ONLY_canonical_fixture' })
const requests: string[] = []
const owner = createWholeCharacterRuntime({ privacy: auth.privacy, origin: location.origin, manifest, pin: manifest,
  fetch: async (input, init) => { requests.push(String(input)); return fetch(input, init) } })
const { map, themes } = await illustratedReleases(), avatarPin = { manifest_id: manifest.manifest_id, manifest_version: manifest.manifest_version, content_hash: manifest.content_hash }
for (const theme of themes) { Object.assign(theme, { avatar_manifest_id: manifest.manifest_id, avatar_manifest_version: 1, avatar_content_hash: manifest.content_hash }); theme.content_hash = await contentHash(theme) }
const resources = createAssetResources({ origin: location.origin, allow: [{ path: map.tilemap.path!, sha256: map.tilemap.sha256!, kind: 'json' }, ...themes.flatMap(theme => [{ path: theme.texture.path!, sha256: theme.texture.sha256!, kind: 'png' as const }, ...sceneryResources(theme)])] })
let engine!: EngineLease, current!: LoadedMap, mounts = 0, destroys = 0
const load = (index: number, signal: AbortSignal) => loadMap({ mapText: JSON.stringify(map), themeText: JSON.stringify(themes[index]), pin: { mapHash: map.content_hash, themeHash: themes[index]!.content_hash, avatar: avatarPin }, resources }, signal)
const host = document.querySelector<HTMLElement>('#host')!
const runtime = createWorldRuntime({ host, load: async signal => current = await load(1, signal), importEngine: async () => { const factory = await loadPhaserEngine(); return (host, map) => {
  engine = factory(host, map); mounts++; return { ...engine, destroy() { destroys++; engine.destroy() } }
} } })
await runtime.start(); engine.camera!.center({ x: 220, y: 550 }, 1)
const raw = { protocolVersion: '1.0.0', type: 'snapshot', streamId: '00000000-0000-4000-8000-000000000004', streamSeq: 0, roomEpoch: '00000000-0000-4000-8000-000000000005', serverTick: 1, eventId, roomId, mapId: map.map_id, mapRevision: map.map_revision, avatarManifestId: manifest.manifest_id, avatarManifestVersion: 1,
  cause: 'INITIAL', requestId: null, selfUserId: userId, ackInputSeq: 0, accessExpiresAt: '2026-11-05T00:00:00Z', players: manifest.presets.map((preset, i) => ({ userId: i === 0 ? userId : `00000000-0000-4000-8000-00000000001${i}`, displayName: `Preset ${i + 1}`, position: { x: 140 + i % 3 * 72, y: 520 + Math.floor(i / 3) * 88 }, facing: 'up', moving: true, avatar: { revision: 1, config: preset.config } })) }
const snapshot = validate<Snapshot>('Snapshot', raw)
let view: ReplicationView = { scope: auth.privacy.capture(), phase: 'live', visible: true, baseline: snapshot, entities: snapshot.players.map(player => ({ player, self: player.userId === userId, position: player.position })), pendingInputs: 0, ackInputSeq: 0, spatialPrediction: 'unavailable', problem: null }
const abort = new AbortController(), port = createWholeCharacterWorldRenderer({ resolveRuntime: () => owner })
const lease = port.mount({ host, map: current, camera: engine.camera!, avatarScene: engine.avatarScene!, release: { avatarManifest: manifest, resources: { eventId } } as never, signal: abort.signal, sample: () => view })
lease.replace(view)
const api = {
  state: () => ({ status: lease.readStatus!(), scene: engine.avatarScene!.read(), owner: owner.diagnostics(), requests, mounts, destroys, canvases: host.querySelectorAll('canvas').length, camera: engine.camera!.read(), geometry: current.map.geometry_hash }),
  move() {
    const next = validate<Snapshot>('Snapshot', { ...snapshot, players: view.entities.map(entity => ({ ...entity.player, position: { ...entity.player.position, x: entity.player.position.x + 4 } })) })
    view = { ...view, entities: next.players.map(player => ({ player, self: player.userId === userId, position: player.position })) }; lease.replace(view)
  },
  visible(value: boolean) { view = { ...view, visible: value }; lease.replace(view); runtime.setVisible(value) },
  config() { const entities = structuredClone(view.entities); Object.assign(entities[0]!.player.avatar, { revision: 2, config: manifest.presets[1]!.config }); view = { ...view, entities }; lease.replace(view) },
  async era(index: number) { let next!: LoadedMap; await runtime.replaceSkin(async signal => next = await load(index, signal)); current = next },
  privacy() { auth.privacy.changePolicy(auth.privacy.capture(), 'block') },
  clear() { view = { ...view, phase: 'closed', entities: [] }; lease.replace(view) },
  async artFrames() {
    const images = new Set<HTMLImageElement>(), hashes: string[] = [], sources: number[] = []
    for (const preset of manifest.presets) {
      const acquired = await owner.acquire(preset.config, { signal: abort.signal }), render = acquired.read()!, cell = document.createElement('canvas'); cell.width = cell.height = 64
      images.add(render.image); sources.push(render.source.x); cell.getContext('2d')!.drawImage(render.image, render.source.x, 0, 64, 64, 0, 0, 64, 64)
      hashes.push(await sha256(new Uint8Array(cell.getContext('2d')!.getImageData(0, 0, 64, 64).data.buffer))); acquired.dispose(); cell.width = cell.height = 0
    }
    return { images: images.size, hashes, sources }
  },
  async sceneContract() {
    const acquired = await owner.acquire(manifest.presets[0]!.config, { signal: abort.signal }), render = acquired.read()!, cell = document.createElement('canvas'); cell.width = cell.height = 64
    cell.getContext('2d')!.drawImage(render.image, render.source.x, 0, 64, 64, 0, 0, 64, 64)
    const pixelHash = await sha256(new Uint8Array(cell.getContext('2d')!.getImageData(0, 0, 64, 64).data.buffer))
    let rejected = 0
    try { engine.avatarScene!.texture(cell) } catch { rejected++ }
    const texture = engine.avatarScene!.staticTexture64!(cell), sprite = engine.avatarScene!.sprite(texture); sprite.update(140, 520, 0, 1.5)
    for (const frame of [1, 19, -1, NaN]) { try { sprite.update(140, 520, frame, 1.5) } catch { rejected++ } }
    sprite.dispose(); texture.dispose(); cell.width = 320; cell.height = 256
    try { engine.avatarScene!.staticTexture64!(cell) } catch { rejected++ }
    const layered = engine.avatarScene!.texture(cell), old = engine.avatarScene!.sprite(layered); old.update(140, 520, 19, 1.5)
    try { old.update(140, 520, 20, 1.5) } catch { rejected++ }
    old.dispose(); layered.dispose(); acquired.dispose(); cell.width = cell.height = 0; return { rejected, pixelHash }
  },
  async dispose() { lease.dispose(); abort.abort(); owner.dispose(); runtime.dispose(); await runtime.settled(); auth.dispose(); return { scene: engine.avatarScene!.read(), canvases: host.querySelectorAll('canvas').length, owner: owner.diagnostics(), mounts, destroys } },
}
Object.assign(window, { wholeWorld: api })
