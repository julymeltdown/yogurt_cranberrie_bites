// @vitest-environment jsdom
import { expect, it, vi } from 'vitest'
import manifest from '../../../../../contracts/fixtures/assets/avatar-manifest.json'
import { createAvatarRenderer } from '../renderer'
import type { AvatarCompositor } from '../../../entities/avatar/runtime/compositor'
import type { WorldRendererPort } from '../../composition'
import type { AvatarScenePort } from '../scene'

function fixture() {
  let finish!: (compositor: AvatarCompositor) => void
  const pending = new Promise<AvatarCompositor>(resolve => { finish = resolve })
  const controller = new AbortController(), listeners = new Set<(time: number) => void>()
  const texture = vi.fn(), sprite = vi.fn(), dispose = vi.fn()
  const scene: AvatarScenePort = { texture, sprite, onFrame(listener) { listeners.add(listener); return () => { listeners.delete(listener) } },
    read: () => ({ sprites: 0, textures: 0, listeners: listeners.size }) }
  const context = { host: document.createElement('div'), map: { widthPx: 100, heightPx: 100 }, camera: {},
    release: { avatarManifest: manifest, resources: { eventId: 'fixture-event' } }, signal: controller.signal, avatarScene: scene,
    sample: () => null } as unknown as Parameters<WorldRendererPort['mount']>[0]
  const renderer = createAvatarRenderer({ load: () => pending }), lease = renderer.mount(context)
  return { renderer, lease, finish, controller, listeners, texture, sprite, dispose }
}
it('disposes an abort-resistant compositor completing after privacy teardown without restoring sprites or status', async () => {
  const f = fixture(), status = vi.fn()
  f.lease.subscribeStatus!(status)
  await Promise.resolve() // The in-flight loader is now actually dispatched.
  expect(f.listeners.size).toBe(1); f.controller.abort()
  expect(f.listeners.size).toBe(0); expect(f.renderer.read()).toMatchObject({ sprites: 0, textures: 0, disposals: 1 })
  f.finish({ dispose: f.dispose } as unknown as AvatarCompositor)
  await new Promise(resolve => setTimeout(resolve, 0))
  expect(f.dispose).toHaveBeenCalledOnce(); expect(status).not.toHaveBeenCalled()
  expect(f.texture).not.toHaveBeenCalled(); expect(f.sprite).not.toHaveBeenCalled()
  expect(f.lease.readStatus!()).toBe('unavailable')
})
it('does not dispatch a loader after a synchronous renderer disposal', async () => {
  const f = fixture(); f.lease.dispose(); await Promise.resolve()
  expect(f.listeners.size).toBe(0); expect(f.renderer.read().disposals).toBe(1)
})
