import { useState } from 'react'
import { createRoot } from 'react-dom/client'
import { createSessionController } from '../../../../features/auth/model/session'
import { isolatedCookiePolicy } from '../../../../features/auth/__tests__/fixtures/data'
import { loadMotionArtwork } from '../../../../entities/avatar/runtime/motion-adoption/art'
import { createMotionRuntime } from '../../../../entities/avatar/runtime/motion-adoption/runtime'
import { createMotionEditorController, type MotionAvatarState } from '../../../../features/avatar/motion-adoption/controller'
import { MotionEditor } from '../../../../features/avatar/motion-adoption/MotionEditor'
import { MotionView } from '../../../../features/avatar/motion-adoption/MotionView'
import { createMotionV2CandidateAuthority, decodeMotionV2Candidate } from '../../../../entities/avatar/runtime/motion-adoption/v2-candidate'
import { motionV2Selection, type MotionV2Config } from '../../../../entities/avatar/runtime/motion-adoption/v2'
import { createMotionWorldRenderer } from '../../../../world/avatar/motion-adoption/renderer'
import { illustratedReleases } from '../../../../world/__tests__/illustrated/fixtures'
import { createAssetResources, loadMap, type LoadedMap } from '../../../../world/map/resources'
import { sceneryResources } from '../../../../world/illustrated/model'
import { contentHash, sha256 } from '../../../../world/map/json'
import { createWorldRuntime, type EngineLease } from '../../../../world/runtime'
import { loadPhaserEngine } from '../../../../world/scenes/MapScene'
import type { ReplicationView } from '../../../../world/network/replication'
import { TransportError, parseServerError } from '../../../../shared/api/errors'
import '../../../../shared/styles/base.css'
import '../../../../shared/styles/themes/themes.css'
import './fixture.css'

// Component/real-Phaser fixture. This is not the application's production Root or a public C05/HTTP release.
const eventId = '00000000-0000-4000-8000-000000000002', userId = '00000000-0000-4000-8000-000000000001'
const auth = createSessionController({ eventId, cookiePolicy: isolatedCookiePolicy(), fetch: async () => { throw Error('Auth network outside this fixture') } })
auth.session.replace(auth.session.read().generation, { userId, eventId, accessToken: 'TEST_ONLY_motion_fixture' })
const scope = auth.privacy.capture(), abort = new AbortController(), requests: string[] = []
const loadedArt = await loadMotionArtwork({ origin: location.origin, signal: abort.signal,
  fetch: (input, init) => { requests.push(String(input)); return fetch(input, init) } })
const candidateResponse = await fetch('/__motion/candidate', { credentials: 'omit', cache: 'no-store', signal: abort.signal })
if (!candidateResponse.ok) throw Error('Pinned candidate unavailable')
const candidate = await decodeMotionV2Candidate(new Uint8Array(await candidateResponse.arrayBuffer()))
const authority = createMotionV2CandidateAuthority(candidate)
const testMotionPin = Object.freeze({ manifest_id: authority.manifestId, manifest_version: authority.manifestVersion, content_hash: authority.manifestHash })
const owner = createMotionRuntime({ authority, scope, privacy: auth.privacy, origin: location.origin, load: async () => loadedArt })
async function request(method: 'GET' | 'PUT', body?: unknown): Promise<MotionAvatarState<MotionV2Config>> {
  const response = await fetch('/__motion/me', { method, credentials: 'omit', cache: 'no-store', signal: abort.signal,
    ...(body ? { headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(body) } : {}) })
  const value = await response.json()
  if (!response.ok) throw new TransportError('http', { status: response.status, server: parseServerError(value, 'core', response.status), uncertain: response.status >= 500 })
  if (Object.keys(value).sort().join(',') !== 'config,revision' || !Number.isSafeInteger(value.revision) || value.revision < 1) throw Error('Invalid test-only receipt')
  return { revision: value.revision, config: authority.decodeConfig(value.config) }
}
let saved = await request('GET'), profile = saved, renders = 0
const peer = authority.presets(loadedArt.art)[3]!.config
const { map, themes } = await illustratedReleases()
for (const theme of themes) { Object.assign(theme, { avatar_manifest_id: testMotionPin.manifest_id, avatar_manifest_version: 1, avatar_content_hash: testMotionPin.content_hash }); theme.content_hash = await contentHash(theme) }
const resources = createAssetResources({ origin: location.origin, allow: [{ path: map.tilemap.path!, sha256: map.tilemap.sha256!, kind: 'json' },
  ...themes.flatMap(theme => [{ path: theme.texture.path!, sha256: theme.texture.sha256!, kind: 'png' as const }, ...sceneryResources(theme)])] })
let engine!: EngineLease, current!: LoadedMap, mounts = 0, destroys = 0, themeIndex = 1
const load = async (index: number, signal: AbortSignal) => {
  try { return await loadMap({ mapText: JSON.stringify(map), themeText: JSON.stringify(themes[index]),
    pin: { mapHash: map.content_hash, themeHash: themes[index]!.content_hash, avatar: testMotionPin }, resources }, signal) }
  catch (error) { console.error('Fixture map qualification failed', error); throw error }
}
const worldHost = document.querySelector<HTMLElement>('#world')!
const world = createWorldRuntime({ host: worldHost, load: async signal => current = await load(themeIndex, signal), importEngine: async () => {
  try { const factory = await loadPhaserEngine(); return (host, map) => {
    try { engine = factory(host, map); mounts++; return { ...engine, destroy() { destroys++; engine.destroy() } } }
    catch (error) { console.error('Fixture engine construction failed', error); throw error }
  } } catch (error) { console.error('Fixture engine import failed', error); throw error }
} })
const worldState = await world.start(); if (worldState.phase !== 'ready') throw Error(`Fixture world startup ${JSON.stringify(worldState)}`)
engine.camera!.center({ x: 220, y: 550 }, 1)
const makeView = (): ReplicationView => ({ scope, phase: 'live', visible: true,
  baseline: { eventId, avatarManifestId: testMotionPin.manifest_id, avatarManifestVersion: 1, streamId: 'test-only-stream', roomEpoch: 'test-only-epoch', roomId: 'test-only-room' },
  entities: [saved, { revision: 1, config: peer }].map((avatar, i) => ({ self: i === 0, position: { x: 180 + i * 88, y: 540 },
    player: { userId: i ? 'test-only-peer' : userId, position: { x: 180 + i * 88, y: 540 }, facing: 'down', moving: false, avatar } })),
}) as unknown as ReplicationView
// Intentionally local TEST_ONLY view. The unchanged public C05 decoder must reject these configs.
let view = makeView()
const renderer = createMotionWorldRenderer({ qualification: 'TEST_ONLY', resolveRuntime: () => owner })
const observedFrames: { texture: string; frame: number; x: number; y: number }[] = [], observedFrameSet = new Set<number>(), incumbentScene = engine.avatarScene!
const observedScene = { ...incumbentScene, sprite(texture: Parameters<typeof incumbentScene.sprite>[0]) {
  const sprite = incumbentScene.sprite(texture)
  return { ...sprite, update(x: number, y: number, frame: number, depth: number) {
    sprite.update(x, y, frame, depth); observedFrameSet.add(frame); observedFrames.push({ texture: texture.key, frame, x, y }); if (observedFrames.length > 200) observedFrames.shift()
  } }
} }
const lease = renderer.mount({ host: worldHost, map: current, camera: engine.camera!, avatarScene: observedScene,
  release: { avatarManifest: testMotionPin, resources: { eventId } } as never, signal: abort.signal, sample: () => view })
lease.replace(view)
let updateProfile = () => {}
const controller = createMotionEditorController({ runtime: owner, privacy: auth.privacy, persistence: {
  read: signal => request('GET').then(value => { if (signal.aborted) throw Error('Aborted read'); return value }),
  save: (expectedRevision, config, signal) => request('PUT', { expected_revision: expectedRevision, config }).then(value => { if (signal.aborted) throw Error('Aborted save'); return value }),
}, onCommitted(value) {
  saved = value; profile = value; view = makeView(); lease.replace(view); updateProfile()
} })
function Fixture() {
  const [tab, setTab] = useState('world'), [revision, setRevision] = useState(profile.revision)
  updateProfile = () => setRevision(profile.revision); renders++
  return <><nav aria-label="Fixture navigation">{['world', 'customize', 'profile'].map(value => <button type="button" key={value} aria-pressed={tab === value} onClick={() => setTab(value)}>{value.toUpperCase()}</button>)}</nav>
    <main>{tab === 'customize' ? <MotionEditor controller={controller} /> : tab === 'profile' ? <section aria-label="Profile"><h1>PROFILE</h1>
      <MotionView runtime={owner} config={profile.config} size={72} label="Saved profile avatar" /><p>Saved revision {revision}</p></section> : <p>Two local test actors · one genuine Phaser scene · no live socket</p>}</main></>
}
const reactRoot = createRoot(document.querySelector('#controls')!); reactRoot.render(<Fixture />)
Object.assign(window, { motionJourney: {
  state: () => ({ requests, mounts, destroys, sockets: 0, status: lease.readStatus(), scene: engine.avatarScene!.read(), owner: owner.diagnostics(),
    saved, profile, controller: controller.read(), renders, gameCanvases: worldHost.querySelectorAll('canvas').length, geometry: current.map.geometry_hash,
    actualSpriteFrames: observedFrames.slice(-100), distinctSpriteFrames: [...observedFrameSet] }),
  move(direction: 'down' | 'left' | 'right' | 'up', moving: boolean) { observedFrameSet.clear(); view = { ...view, entities: view.entities.map(entity => ({ ...entity, player: { ...entity.player, facing: direction, moving } })) }; lease.replace(view) },
  clearFrameObservations() { observedFrames.length = 0; observedFrameSet.clear() },
  async theme(index: number) { themeIndex = index; let next!: LoadedMap; await world.replaceSkin(async signal => next = await load(index, signal)); current = next; document.documentElement.dataset.mcrTheme = ['REVENGE', 'PARADE', 'DANGER'][index] },
  async pixels() {
    const current = await owner.acquire(saved.config, { signal: abort.signal }), canvas = document.createElement('canvas'); canvas.width = canvas.height = 64
    const ctx = canvas.getContext('2d')!; ctx.drawImage(current.read()!.canvas, 0, 0, 64, 64, 0, 0, 64, 64)
    const hash = await sha256(new Uint8Array(ctx.getImageData(0, 0, 64, 64).data.buffer)); current.dispose()
    const portrait = document.querySelector<HTMLCanvasElement>('canvas[aria-label="Saved profile avatar"]')
    return { sharedComposition: hash, profile: portrait ? await sha256(new Uint8Array(portrait.getContext('2d')!.getImageData(0, 0, 64, 64).data.buffer)) : null }
  },
  async artProof() {
    const reference = document.createElement('canvas'), target = document.createElement('canvas'); reference.width = target.width = reference.height = target.height = 64
    const a = reference.getContext('2d', { willReadFrequently: true })!, b = target.getContext('2d', { willReadFrequently: true })!
    let compared = 0
    const distinct: number[] = []
    for (const preset of authority.presets(loadedArt.art)) {
      const lease = await owner.acquire(preset.config, { signal: abort.signal }), render = lease.read()!
      for (let direction = 0; direction < 4; direction++) {
        const phases = new Set<string>()
        for (let phase = 0; phase < 5; phase++) {
          const selected = motionV2Selection(preset.config)
          if (selected.mode !== 'preset') throw Error('Wrong preset branch')
          const index = loadedArt.art.descriptor.presets.frames.find(frame => frame.preset === selected.preset && frame.direction === ['front', 'left', 'right', 'rear'][direction] && frame.phase === phase)!.atlasIndex
          a.clearRect(0, 0, 64, 64); b.clearRect(0, 0, 64, 64)
          a.drawImage(loadedArt.image, index % 20 * 64, Math.floor(index / 20) * 64, 64, 64, 0, 0, 64, 64)
          b.drawImage(render.canvas, phase * 64, direction * 64, 64, 64, 0, 0, 64, 64)
          const expected = await sha256(new Uint8Array(a.getImageData(0, 0, 64, 64).data.buffer)), actual = await sha256(new Uint8Array(b.getImageData(0, 0, 64, 64).data.buffer))
          if (expected !== actual) throw Error(`Preset UV mismatch ${preset.id}/${direction}/${phase}`)
          phases.add(actual); compared++
        }
        distinct.push(phases.size)
      }
      lease.dispose()
    }
    reference.width = target.width = reference.height = target.height = 0
    return { compared, distinct }
  },
  async wardrobeProof(cases: { body: number; hair: number; eyes: number; outfit: number; accessory: number | null; direction: number; phase: number }[]) {
    const canvas = document.createElement('canvas'); canvas.width = canvas.height = 64
    const ctx = canvas.getContext('2d', { willReadFrequently: true })!, hashes: string[] = []
    for (const entry of cases) {
      const config = authority.replace(saved.config, { mode: 'wardrobe', body: `body-${entry.body}`, hair: `hair-${entry.hair}`, eyes: `eyes-${entry.eyes}`,
        outfit: `outfit-${entry.outfit}`, accessory: entry.accessory === null ? null : `accessory-${entry.accessory}` })
      const lease = await owner.acquire(config, { signal: abort.signal }), render = lease.read()!
      ctx.clearRect(0, 0, 64, 64); ctx.drawImage(render.canvas, entry.phase * 64, entry.direction * 64, 64, 64, 0, 0, 64, 64)
      hashes.push(await sha256(new Uint8Array(ctx.getImageData(0, 0, 64, 64).data.buffer))); lease.dispose()
    }
    canvas.width = canvas.height = 0
    return { cases: hashes.length, aggregate: await sha256(new TextEncoder().encode(hashes.join('\n'))) }
  },
  privacy() { auth.privacy.changePolicy(scope, 'block') },
  async dispose() { reactRoot.unmount(); controller.dispose(); lease.dispose(); abort.abort(); owner.dispose(); world.dispose(); await world.settled(); auth.dispose(); return { mounts, destroys, canvases: worldHost.querySelectorAll('canvas').length, scene: engine.avatarScene!.read(), owner: owner.diagnostics() } },
} })
