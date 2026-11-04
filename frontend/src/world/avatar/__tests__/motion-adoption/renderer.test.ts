// @vitest-environment jsdom
import { readFile } from 'node:fs/promises'
import { URL as FileURL } from 'node:url'
import { afterEach, beforeAll, expect, it, vi } from 'vitest'
import { validateMotionArt, type MotionArt } from '../../../../entities/avatar/runtime/motion-adoption/art'
import { createMotionRuntime } from '../../../../entities/avatar/runtime/motion-adoption/runtime'
import { createMotionWorldRenderer } from '../../motion-adoption/renderer'
import type { ReplicationView } from '../../../network/replication'
import type { WorldRendererPort } from '../../../composition'
import type { AvatarScenePort } from '../../scene'
import { testMotionAuthority, testMotionPin } from './test-authority'
let art: MotionArt
beforeAll(async () => { art = await validateMotionArt(new Uint8Array(await readFile(new FileURL('../../../../../public/world/avatars/derived-v1/motion-candidates-v1/combined-release-candidate.json', import.meta.url)))) })
const stops: (() => void)[] = []
afterEach(() => { stops.splice(0).reverse().forEach(fn => fn()); vi.restoreAllMocks() })
const settle = async () => { for (let i = 0; i < 30; i++) await Promise.resolve() }
function fixture() {
  vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockReturnValue({ drawImage: vi.fn(), clearRect: vi.fn(), fillRect: vi.fn(), save: vi.fn(), restore: vi.fn(), beginPath: vi.fn(), rect: vi.fn(), clip: vi.fn() } as unknown as CanvasRenderingContext2D)
  let alive = true, sample: ReplicationView | null = null
  const cleanups = new Set<() => void>(), frames = new Set<(time: number) => void>(), scope = { userId: 'self', eventId: 'event', generation: 1, policyGeneration: 1, gate: 'open' as const }
  const imageDispose = vi.fn(), load = vi.fn(async () => ({ art, image: new Image(), dispose: imageDispose }))
  const runtime = createMotionRuntime({ authority: testMotionAuthority(art), scope, privacy: { isCurrent: () => alive,
    registerCleanup(fn) { cleanups.add(fn); return () => { cleanups.delete(fn) } } }, origin: 'https://mcr.test', load })
  const sprites: { update: ReturnType<typeof vi.fn>; dispose: ReturnType<typeof vi.fn> }[] = [], textures: { key: string; dispose: ReturnType<typeof vi.fn> }[] = []
  const scene: AvatarScenePort = { texture: vi.fn(canvas => { expect([canvas.width, canvas.height]).toEqual([320, 256]); const t = { key: String(textures.length), dispose: vi.fn() }; textures.push(t); return t }),
    sprite: () => { const sprite = { update: vi.fn(), dispose: vi.fn() }; sprites.push(sprite); return sprite },
    onFrame(fn) { frames.add(fn); return () => { frames.delete(fn) } }, read: () => ({ textures: textures.length, sprites: sprites.length, listeners: frames.size }) }
  const controller = new AbortController(), context = { avatarScene: scene, signal: controller.signal, map: { widthPx: 1000, heightPx: 800 },
    release: { avatarManifest: testMotionPin, resources: { eventId: scope.eventId } }, sample: () => sample } as unknown as Parameters<WorldRendererPort['mount']>[0]
  const lease = createMotionWorldRenderer({ qualification: 'TEST_ONLY', resolveRuntime: () => runtime }).mount(context)
  const presets = testMotionAuthority(art).presets(art)
  function view(indices = [0, 0]) { return { scope, phase: 'live', visible: true,
    baseline: { eventId: scope.eventId, avatarManifestId: testMotionPin.manifest_id, avatarManifestVersion: 1, streamId: 'stream', roomEpoch: 'epoch', roomId: 'room' },
    entities: indices.map((index, i) => ({ position: { x: 100 + i * 64, y: 200 }, self: i === 0, player: { userId: String(i), facing: 'left', moving: true, avatar: { revision: 1, config: presets[index]!.config } } })) } as unknown as ReplicationView }
  stops.push(runtime.dispose, lease.dispose)
  return { runtime, lease, scope, view, frames, sprites, textures, load, controller, imageDispose,
    tick(value: ReplicationView | null, time: number) { sample = value; for (const fn of Array.from(frames)) fn(time) },
    revoke() { alive = false; for (const fn of Array.from(cleanups)) fn() } }
}
it('shares composition/texture, advances genuine four walk cells and returns to directional idle on the same scene clock', async () => {
  const f = fixture(), view = f.view(); f.tick(view, 0); await settle()
  expect(f.load).toHaveBeenCalledOnce(); expect(f.textures).toHaveLength(1); expect(f.sprites).toHaveLength(2)
  expect(f.runtime.diagnostics().leases).toBe(1)
  const clock = vi.fn(), stop = f.runtime.onFrame(clock)
  for (const [time, frame] of [[140, 7], [280, 8], [420, 9], [560, 6]]) { f.tick(view, time!); expect(f.sprites[0]!.update.mock.lastCall?.[2]).toBe(frame) }
  const idle = structuredClone(view); Object.assign(idle.entities[0]!.player, { moving: false, facing: 'up' }); f.tick(idle, 700)
  expect(f.sprites[0]!.update.mock.lastCall?.[2]).toBe(15); expect(clock).toHaveBeenCalledTimes(5); stop()
  expect(f.frames.size).toBe(1); expect(f.textures).toHaveLength(1)
})
it('releases synchronous private leases, GPU resources and frame listeners, without an extra owner or late resurrection', async () => {
  const f = fixture(); f.tick(f.view(), 0); await settle(); f.revoke()
  expect(f.textures[0]!.dispose).toHaveBeenCalledOnce(); expect(f.sprites.every(s => s.dispose.mock.calls.length === 1)).toBe(true)
  expect(f.frames.size).toBe(0); expect(f.runtime.diagnostics()).toMatchObject({ leases: 0, decodedAtlases: 0, disposed: true })
  expect(f.imageDispose).toHaveBeenCalledOnce(); f.tick(f.view(), 140); expect(f.textures).toHaveLength(1)
})
it('removes an old config/revision, room baseline and invisible roster before reacquiring', async () => {
  const f = fixture(); f.tick(f.view([0]), 0); await settle(); const changed = f.view([1]); f.tick(changed, 140)
  expect(f.textures[0]!.dispose).toHaveBeenCalledOnce(); await settle(); expect(f.textures).toHaveLength(2)
  const room = structuredClone(changed); Object.assign(room.baseline!, { roomEpoch: 'new' }); f.tick(room, 280)
  expect(f.textures[1]!.dispose).toHaveBeenCalledOnce(); await settle()
  f.tick({ ...room, visible: false }, 420); expect(f.runtime.diagnostics().leases).toBe(0)
  expect(f.textures.at(-1)!.dispose).toHaveBeenCalledOnce(); expect(f.load).toHaveBeenCalledOnce()
})
it.each(['position', 'ids', 'manifest', 'scope'] as const)('fails closed on invalid %s', async kind => {
  const f = fixture(), view = f.view()
  if (kind === 'position') Object.assign(view.entities[0]!.position, { x: Infinity })
  if (kind === 'ids') Object.assign(view.entities[1]!.player, { userId: view.entities[0]!.player.userId })
  if (kind === 'manifest') Object.assign(view.baseline!, { avatarManifestId: 'old_public_family' })
  if (kind === 'scope') Object.assign(view.scope, { generation: 2 })
  f.lease.replace(view); await settle(); expect(f.textures).toHaveLength(0); expect(f.lease.readStatus()).toBe('unavailable')
})
it('one aborted consumer cannot cancel the shared decode or another consumer lease', async () => {
  const f = fixture(), first = new AbortController(), second = new AbortController(), preset = testMotionAuthority(art).presets(art)[0]!.config
  const a = f.runtime.acquire(preset, { signal: first.signal }), b = f.runtime.acquire(preset, { signal: second.signal })
  first.abort(); await expect(a).rejects.toThrow(); const lease = await b
  expect(lease.read()).not.toBeNull(); expect(f.load).toHaveBeenCalledOnce(); expect(f.runtime.diagnostics().leases).toBe(1)
  second.abort(); expect(lease.read()).toBeNull(); expect(f.runtime.diagnostics().leases).toBe(0); expect(f.runtime.isCurrent()).toBe(true)
})
it('an abort-resistant image decode is disposed after private withdrawal, without a scene allocation', async () => {
  const f = fixture(); let finish!: (value: Awaited<ReturnType<typeof f.load>>) => void
  f.load.mockImplementation(() => new Promise(resolve => { finish = resolve }))
  f.tick(f.view(), 0); await settle(); f.revoke()
  const dispose = vi.fn(); finish({ art, image: new Image(), dispose }); await settle()
  expect(dispose).toHaveBeenCalledOnce(); expect(f.textures).toHaveLength(0); expect(f.frames.size).toBe(0)
})
