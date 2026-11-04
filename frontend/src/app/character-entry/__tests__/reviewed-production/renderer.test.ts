// @vitest-environment jsdom
import { afterEach, expect, it, vi } from 'vitest'
import { setup } from '../../../../features/avatar/__tests__/reviewed-production/fixtures'
import { createMotionRuntime } from '../../../../entities/avatar/runtime/motion-adoption/runtime'
import { createMotionWorldRenderer } from '../../../../world/avatar/motion-adoption/renderer'
import type { WorldRendererPort } from '../../../../world/composition'
import type { AvatarScenePort } from '../../../../world/avatar/scene'

const stops: (() => void)[] = []
afterEach(() => { stops.splice(0).reverse().forEach(fn => fn()); vi.restoreAllMocks() })
async function fixture() {
  const f = await setup(); stops.push(f.dispose)
  const authority = await f.create()
  const runtime = createMotionRuntime({ authority, scope: f.scope, privacy: f.auth.privacy, origin: 'https://mcr.test' }); stops.push(runtime.dispose)
  const frames = new Set<(time: number) => void>(), controller = new AbortController()
  const scene: AvatarScenePort = { texture: vi.fn(), sprite: vi.fn(), read: () => ({ textures: 0, sprites: 0, listeners: frames.size }),
    onFrame(fn) { frames.add(fn); return () => { frames.delete(fn) } } }
  const context = { avatarScene: scene, signal: controller.signal, release: { avatarManifest: {
    manifest_id: authority.manifestId, manifest_version: authority.manifestVersion, content_hash: authority.manifestHash,
  } }, sample: () => null } as unknown as Parameters<WorldRendererPort['mount']>[0]
  return { ...f, authority, runtime, frames, controller, context }
}
it('lends the genuine production runtime to the incumbent frame clock and never destroys the shared runtime on view disposal', async () => {
  const f = await fixture(), lease = createMotionWorldRenderer({ qualification: 'REVIEWED_PRODUCTION', resolveRuntime: () => f.runtime }).mount(f.context)
  expect(lease.readStatus?.()).toBe('ready'); expect(f.frames.size).toBe(1)
  expect(f.runtime.diagnostics().decodedAtlases).toBe(0)
  lease.dispose(); expect(f.frames.size).toBe(0); expect(f.runtime.isCurrent()).toBe(true)
})
it('rejects a copied authority despite genuine runtime closures and production qualification text', async () => {
  const f = await fixture(), forged = { ...f.runtime, authority: { ...f.authority } }
  const lease = createMotionWorldRenderer({ qualification: 'REVIEWED_PRODUCTION', resolveRuntime: () => forged }).mount(f.context)
  expect(lease.readStatus?.()).toBe('unavailable'); expect(f.frames.size).toBe(0)
})
it('keeps a genuine production grant out of the TEST_ONLY entrypoint', async () => {
  const f = await fixture(), lease = createMotionWorldRenderer({ qualification: 'TEST_ONLY', resolveRuntime: () => f.runtime }).mount(f.context)
  expect(lease.readStatus?.()).toBe('unavailable'); expect(f.frames.size).toBe(0)
})
it('removes the incumbent clock subscription on world abort while keeping the borrowed account runtime current', async () => {
  const f = await fixture(), lease = createMotionWorldRenderer({ qualification: 'REVIEWED_PRODUCTION', resolveRuntime: () => f.runtime }).mount(f.context)
  f.controller.abort()
  expect(f.frames.size).toBe(0); expect(lease.readStatus?.()).toBe('unavailable'); expect(f.runtime.isCurrent()).toBe(true)
})
