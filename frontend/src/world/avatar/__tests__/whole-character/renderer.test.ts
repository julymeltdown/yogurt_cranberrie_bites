// @vitest-environment jsdom
import { afterEach, expect, it, vi } from 'vitest'
import manifest from '../../../../../../contracts/fixtures/assets/selected-static/avatar-manifest.json'
import type { WholeCharacterLease, WholeCharacterRender } from '../../../../entities/avatar/whole-character'
import { createWholeCharacterWorldRenderer, type BorrowedWholeCharacterRuntime } from '../../whole-character'
import type { WorldRendererPort } from '../../../composition'
import type { ReplicationView } from '../../../network/replication'
import type { AvatarScenePort } from '../../scene'
const scope = { userId: '00000000-0000-4000-8000-000000000001', eventId: '00000000-0000-4000-8000-000000000002', generation: 1, policyGeneration: 1, gate: 'open' as const }
const stops: (() => void)[] = []
afterEach(() => { stops.splice(0).forEach(stop => stop()); vi.restoreAllMocks() })
async function settle() { for (let i = 0; i < 12; i++) await Promise.resolve() }
function fixture() {
  vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockReturnValue({ imageSmoothingEnabled: false, drawImage: vi.fn() } as unknown as CanvasRenderingContext2D)
  const frames = new Set<() => void>(), controller = new AbortController(), calls: { config: unknown; signal: AbortSignal; finish(lease: WholeCharacterLease): void; reject(): void }[] = []
  const spriteRows: { update: ReturnType<typeof vi.fn>; dispose: ReturnType<typeof vi.fn> }[] = [], textureRows: { key: string; dispose: ReturnType<typeof vi.fn> }[] = []
  let alive = true, sample: ReplicationView | null = null
  const borrowed = { scope, isCurrent: () => alive, validateConfig(value: unknown) {
    if (!value || typeof value !== 'object' || !manifest.presets.some(p => JSON.stringify(p.config) === JSON.stringify(value))) throw Error('invalid')
    return value
  }, acquire(config: unknown, options: { signal: AbortSignal }) { return new Promise<WholeCharacterLease>((finish, reject) => calls.push({ config, signal: options.signal, finish, reject: () => reject(Error('failed')) })) }, dispose: vi.fn() } as unknown as BorrowedWholeCharacterRuntime & { dispose: ReturnType<typeof vi.fn> }
  const scene: AvatarScenePort = { texture: vi.fn(), staticTexture64: vi.fn(canvas => {
    expect([canvas.width, canvas.height]).toEqual([64, 64]); const texture = { key: String(textureRows.length), dispose: vi.fn() }; textureRows.push(texture); return texture
  }), sprite: vi.fn(() => { const row = { update: vi.fn(), dispose: vi.fn() }; spriteRows.push(row); return row }),
    onFrame(fn) { const next = () => fn(0); frames.add(next); return () => { frames.delete(next) } }, read: () => ({ textures: textureRows.length, sprites: spriteRows.length, listeners: frames.size }) }
  const context = { host: document.createElement('div'), map: { widthPx: 1000, heightPx: 800 }, camera: {}, avatarScene: scene, signal: controller.signal,
    release: { avatarManifest: manifest, resources: { eventId: scope.eventId } }, sample: () => sample } as unknown as Parameters<WorldRendererPort['mount']>[0]
  const port = createWholeCharacterWorldRenderer({ resolveRuntime: () => borrowed }), lease = port.mount(context)
  stops.push(lease.dispose)
  function complete(index: number) {
    let active = true
    const listeners = new Set<() => void>(), dispose = vi.fn(() => { if (!active) return; active = false; for (const fn of Array.from(listeners)) fn() })
    const config = calls[index]!.config as typeof manifest.presets[number]['config']
    const image = { naturalWidth: 384, naturalHeight: 64 } as HTMLImageElement
    const read = vi.fn(() => active ? { config, pose: 'front_idle', image, source: { x: (Number(config.preset_id.slice(-2)) - 1) * 64, y: 0, width: 64, height: 64 }, anchor: { x: 32, y: 60 }, logicalCell: { width: 64, height: 64 } } as WholeCharacterRender : null)
    const owned = { read, dispose, subscribe(fn: () => void) { listeners.add(fn); return () => { listeners.delete(fn) } } }
    calls[index]!.finish(owned); return { ...owned, revoke: dispose }
  }
  return { lease, calls, complete, frames, spriteRows, textureRows, borrowed, scene, controller, context,
    privacy() { alive = false }, tick(view: ReplicationView | null) { sample = view; for (const fn of Array.from(frames)) fn() } }
}
function view(presets = [0], patch: Partial<ReplicationView> = {}): ReplicationView {
  return { scope, phase: 'live', visible: true, baseline: { eventId: scope.eventId, avatarManifestId: manifest.manifest_id, avatarManifestVersion: 1, streamId: 'one', roomEpoch: 'one', roomId: 'one' },
    entities: presets.map((index, i) => ({ self: i === 0, position: { x: 100 + i * 64, y: 200 }, player: { userId: String(i), position: { x: 100 + i * 64, y: 200 }, facing: 'up', moving: true, avatar: { revision: 1, config: manifest.presets[index]!.config } } })), ...patch } as unknown as ReplicationView
}
it('shares one async asset/texture across same-config rows and always renders frame0 at canonical foot/depth', async () => {
  const f = fixture(); f.lease.replace(view([0, 0])); await settle(); expect(f.calls).toHaveLength(1); f.complete(0); await settle()
  expect(f.textureRows).toHaveLength(1); expect(f.spriteRows).toHaveLength(2)
  expect(f.spriteRows[0]!.update).toHaveBeenLastCalledWith(100, 200, 0, 1 + 200 / 801 * .9)
  f.lease.replace(view([0])); expect(f.spriteRows[1]!.dispose).toHaveBeenCalledOnce(); expect(f.textureRows[0]!.dispose).not.toHaveBeenCalled()
  f.lease.replace(view([])); expect(f.textureRows[0]!.dispose).toHaveBeenCalledOnce(); expect(f.borrowed.dispose).not.toHaveBeenCalled()
})
it('consumes six genuine config identities separately with no facing/walk substitution', async () => {
  const f = fixture(); f.lease.replace(view([0, 1, 2, 3, 4, 5])); await settle(); expect(f.calls).toHaveLength(6)
  for (let i = 0; i < 6; i++) f.complete(i); await settle(); expect(f.textureRows).toHaveLength(6)
  expect(f.spriteRows.every(row => row.update.mock.calls.every(call => call[2] === 0))).toBe(true)
})
it.each(['closed', 'resync', 'suspended', 'unavailable'] as const)('clears synchronously on %s and ignores late acquire', async phase => {
  const f = fixture(); f.lease.replace(view()); await settle(); f.lease.replace(view([0], { phase })); expect(f.calls[0]!.signal.aborted).toBe(true)
  const late = f.complete(0); await settle(); expect(late.dispose).toHaveBeenCalled(); expect(f.textureRows).toHaveLength(0); expect(f.lease.readStatus!()).toBe('unavailable')
})
it('evicts a changed config/revision immediately and fences abort-resistant completion', async () => {
  const f = fixture(); f.lease.replace(view()); await settle(); f.lease.replace(view([1])); await settle(); const late = f.complete(0); f.complete(1); await settle()
  expect(late.dispose).toHaveBeenCalled(); expect(f.textureRows).toHaveLength(1)
  const changed = view([1]); (changed.entities[0]!.player.avatar as { revision: number }).revision = 2; f.lease.replace(changed); await settle(); expect(f.textureRows[0]!.dispose).toHaveBeenCalled(); expect(f.calls).toHaveLength(3)
})
it('lease revocation clears GPU immediately and scope loss releases scene listeners without disposing borrowed owner', async () => {
  const f = fixture(); f.lease.replace(view()); await settle(); const owned = f.complete(0); await settle(); f.privacy(); owned.revoke()
  expect(f.spriteRows[0]!.dispose).toHaveBeenCalledOnce(); expect(f.textureRows[0]!.dispose).toHaveBeenCalledOnce(); expect(f.frames.size).toBe(0); expect(f.borrowed.dispose).not.toHaveBeenCalled()
})
it.each([NaN, Infinity, -1, 1001])('rejects finite/out-of-map coordinate %s before resource acquisition', async x => {
  const f = fixture(), v = view(); (v.entities[0]!.position as { x: number }).x = x; f.lease.replace(v); await settle(); expect(f.calls).toHaveLength(0); expect(f.lease.readStatus!()).toBe('unavailable')
})
it('rejects duplicate IDs and oversized rooms atomically', async () => {
  const f = fixture(), duplicate = view([0, 1]); (duplicate.entities[1]!.player as { userId: string }).userId = '0'; f.lease.replace(duplicate); await settle(); expect(f.calls).toHaveLength(0)
  f.lease.replace(view(Array.from({ length: 51 }, () => 0))); await settle(); expect(f.calls).toHaveLength(0)
})
it('handles missing sample, invisible roster and generation replacement without resurrecting art', async () => {
  const f = fixture(); f.lease.replace(view()); await settle(); f.complete(0); await settle(); f.tick(null); expect(f.textureRows[0]!.dispose).toHaveBeenCalled()
  f.tick(view([0], { visible: false })); expect(f.lease.readStatus!()).toBe('unavailable')
  f.lease.replace(view([0], { scope: { ...scope, generation: 2 } })); expect(f.frames.size).toBe(0); expect(f.borrowed.dispose).not.toHaveBeenCalled()
})
it('does not retry a failed asset every frame, and abort/dispose releases pending work', async () => {
  const f = fixture(); f.lease.replace(view()); await settle(); f.calls[0]!.reject(); await settle(); f.tick(view()); f.tick(view()); await settle(); expect(f.calls).toHaveLength(1)
  f.controller.abort(); expect(f.frames.size).toBe(0); f.lease.dispose(); expect(f.borrowed.dispose).not.toHaveBeenCalled()
})

it('a room/epoch change evicts the prior skin before the replacement acquisition', async () => {
  const f = fixture(); f.lease.replace(view()); await settle(); f.complete(0); await settle()
  const next = view(); (next.baseline as { roomEpoch: string }).roomEpoch = 'two'; f.lease.replace(next)
  expect(f.textureRows[0]!.dispose).toHaveBeenCalledOnce(); await settle(); expect(f.calls).toHaveLength(2)
})
it('fails closed on an incompatible manifest before borrowing the runtime', () => {
  const f = fixture(); f.lease.dispose()
  const resolveRuntime = vi.fn(() => f.borrowed)
  const context = { ...f.context, release: { ...f.context.release, avatarManifest: { ...manifest, content_hash: 'wrong' } } }
  const lease = createWholeCharacterWorldRenderer({ resolveRuntime }).mount(context)
  expect(resolveRuntime).not.toHaveBeenCalled(); expect(lease.readStatus!()).toBe('unavailable'); lease.dispose()
})
it('refuses a malformed cell/atlas lease without allocating a GPU texture', async () => {
  const f = fixture(); f.lease.replace(view()); await settle(); const owned = f.complete(0)
  const render = owned.read()!; owned.read.mockReturnValue({ ...render, source: { ...render.source, width: 320 } })
  await settle(); expect(f.textureRows).toHaveLength(0); expect(owned.dispose).toHaveBeenCalled(); expect(f.lease.readStatus!()).toBe('unavailable')
})
it('releases a texture returned after synchronous teardown inside scene allocation', async () => {
  const f = fixture(), dispose = vi.fn()
  vi.mocked(f.scene.staticTexture64!).mockImplementationOnce(() => { f.controller.abort(); return { key: 'late', dispose } })
  f.lease.replace(view()); await settle(); const owned = f.complete(0); await settle()
  expect(dispose).toHaveBeenCalledOnce(); expect(owned.dispose).toHaveBeenCalled(); expect(f.spriteRows).toHaveLength(0); expect(f.frames.size).toBe(0)
})
it('unsubscribes a scene callback returned after synchronous abort during registration', () => {
  const f = fixture(); f.lease.dispose(); const controller = new AbortController(), unsubscribe = vi.fn()
  const scene = { ...f.scene, onFrame() { controller.abort(); return unsubscribe } }
  const lease = createWholeCharacterWorldRenderer({ resolveRuntime: () => f.borrowed }).mount({ ...f.context, avatarScene: scene, signal: controller.signal })
  expect(unsubscribe).toHaveBeenCalledOnce(); expect(lease.readStatus!()).toBe('unavailable'); lease.dispose()
})
