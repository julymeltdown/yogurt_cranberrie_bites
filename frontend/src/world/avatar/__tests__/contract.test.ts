// @vitest-environment jsdom
import { afterEach, expect, it, vi } from 'vitest'
import fixture from '../../../../../contracts/fixtures/assets/avatar-manifest.json'
import config from '../../../../../contracts/fixtures/assets/avatar-config.json'
import { contentHash } from '../../map/json'
import { validateConfig, validateManifest, frameIndex, sourceRect, type AvatarConfig } from '../../../entities/avatar/runtime/contract'
import { validateBinding, loadReviewedAvatar } from '../../../entities/avatar/runtime/resources'
import { createAvatarCompositor } from '../../../entities/avatar/runtime/compositor'
import { createReviewedAvatarCompositor } from '../../../entities/avatar/runtime'

async function manifest(mutate: (m: typeof fixture) => void = () => {}) {
  const value = structuredClone(fixture); mutate(value); value.content_hash = await contentHash(value)
  return validateManifest(JSON.stringify(value), value)
}
afterEach(() => vi.restoreAllMocks())
it('validates the frozen C06 fixture without turning its missing image into product art', async () => {
  const m = await validateManifest(JSON.stringify(fixture), fixture)
  expect(validateConfig(config, m)).toEqual(config)
  expect(Object.isFrozen(m.atlas.items)).toBe(true)
  expect(() => validateBinding(m, undefined)).toThrow('unavailable')
  const read = vi.fn()
  await expect(loadReviewedAvatar(m, undefined, { read }, new AbortController().signal)).rejects.toThrow('unavailable')
  await expect(createReviewedAvatarCompositor({ document: m, pin: m, resources: { read }, signal: new AbortController().signal })).rejects.toThrow('unavailable')
  expect(read).not.toHaveBeenCalled()
})
it.each(['hash', 'unknown', 'version', 'anchor', 'overlap', 'bounds', 'duplicate-item', 'missing-layer', 'duplicate-tone', 'duplicate-preset', 'unknown-preset', 'wrong-layer', 'texture-limit'])('rejects invalid C06 avatar metadata: %s', async kind => {
  if (kind === 'hash') { await expect(validateManifest(JSON.stringify({ ...fixture, content_hash: `sha256:${'0'.repeat(64)}` }), fixture)).rejects.toThrow(); return }
  await expect(manifest(m => {
    if (kind === 'unknown') Object.assign(m, { injected: true })
    if (kind === 'version') m.contract_version = '2.0.0'
    if (kind === 'anchor') m.atlas.items[0]!.anchor.y = 59
    if (kind === 'overlap') m.atlas.items[1]!.region.y = 0
    if (kind === 'bounds') m.atlas.items[0]!.region.x = 64
    if (kind === 'duplicate-item') m.atlas.items[1]!.item_id = m.atlas.items[0]!.item_id
    if (kind === 'missing-layer') m.atlas.items.pop()
    if (kind === 'duplicate-tone') m.skin_tones.push(m.skin_tones[0]!)
    if (kind === 'duplicate-preset') m.presets.push(m.presets[0]!)
    if (kind === 'unknown-preset') m.fallback_preset_id = 'missing'
    if (kind === 'wrong-layer') m.presets[0]!.config.item_ids.hair = m.presets[0]!.config.item_ids.body
    if (kind === 'texture-limit') m.atlas.texture.height_px = 4096
  })).rejects.toThrow()
})
it.each(['manifest', 'version', 'tone', 'layer', 'url', 'extra'])('rejects invalid config %s without silently substituting layers', async kind => {
  const m = await manifest(), c = structuredClone(config)
  if (kind === 'manifest') c.manifest_id = 'other'
  if (kind === 'version') c.manifest_version++
  if (kind === 'tone') c.skin_tone_id = 'missing'
  if (kind === 'layer') c.item_ids.hair = c.item_ids.body
  if (kind === 'url') c.item_ids.body = 'https://legacy.invalid/body.png'
  if (kind === 'extra') Object.assign(c.item_ids, { back_hair: 'invented' })
  expect(() => validateConfig(c, m)).toThrow()
})
it('maps all twenty fixed direction/animation cells and rejects invalid frame requests', async () => {
  const m = await manifest(), item = m.atlas.items[2]!
  for (const [r, direction] of (['down', 'left', 'right', 'up'] as const).entries()) {
    for (let column = 0; column < 5; column++) {
      const pose = { direction, animation: column ? 'walk' as const : 'idle' as const, frame: column ? column - 1 : 0 }
      expect(frameIndex(pose)).toBe(r * 5 + column)
      expect(sourceRect(item, pose)).toMatchObject({ x: column * 64, y: item.region.y + r * 64, width: 64, height: 64 })
    }
  }
  expect(() => frameIndex({ direction: 'down', animation: 'walk', frame: 4 })).toThrow()
  expect(() => frameIndex({ direction: 'down', animation: 'idle', frame: 1 })).toThrow()
})
it('bounds ref-counted composed textures, evicts only unpinned entries and clears exported thumbnails on disposal', async () => {
  const m = await manifest(), drawImage = vi.fn(), disposeAtlas = vi.fn()
  vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockReturnValue({ drawImage, imageSmoothingEnabled: true } as unknown as CanvasRenderingContext2D)
  const compositor = createAvatarCompositor({ manifest: m, image: document.createElement('canvas'), bakedSkinToneId: config.skin_tone_id, maxEntries: 1, disposeAtlas })
  const a = compositor.acquire(config), repeated = compositor.acquire(config)
  expect(a.canvas).toBe(repeated.canvas); expect(compositor.read()).toMatchObject({ entries: 1, references: 2 })
  const noAccessory: AvatarConfig = { ...config, item_ids: { ...config.item_ids, accessory: null } } as AvatarConfig
  expect(() => compositor.acquire(noAccessory)).toThrow('capacity')
  a.release(); a.release(); repeated.release()
  const b = compositor.acquire(noAccessory)
  expect(a.canvas.width).toBe(0); expect(compositor.read()).toMatchObject({ entries: 1, references: 1, created: 2, removed: 1 })
  b.release(); const thumbnail = compositor.thumbnail(noAccessory, { direction: 'down', animation: 'idle', frame: 0 })
  expect(thumbnail.canvas.width).toBe(64)
  compositor.dispose(); compositor.dispose()
  expect(thumbnail.canvas.width).toBe(0); expect(disposeAtlas).toHaveBeenCalledOnce()
  expect(compositor.read()).toMatchObject({ entries: 0, created: 2, removed: 2 })
})
it('uses only an explicitly approved complete fallback and rejects unsupported baked tones', async () => {
  const m = await manifest(m => m.skin_tones.push({ skin_tone_id: 'other_tone', color: '#ABCDEF' }))
  vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockReturnValue({ drawImage() {} } as unknown as CanvasRenderingContext2D)
  const options = { manifest: m, image: document.createElement('canvas'), bakedSkinToneId: config.skin_tone_id, disposeAtlas() {} }
  const strict = createAvatarCompositor(options), changed = { ...config, skin_tone_id: 'other_tone' }
  expect(() => strict.acquire(changed)).toThrow('unavailable'); strict.dispose()
  const approved = createAvatarCompositor({ ...options, fallbackPresetId: m.fallback_preset_id })
  expect(() => approved.acquire({ ...config, manifest_version: 999 })).toThrow('version')
  expect(() => approved.acquire(changed)).toThrow('unavailable')
  const unknownItem = { ...config, item_ids: { ...config.item_ids, body: 'missing' } }
  const fallback = approved.acquire(unknownItem)
  expect(fallback.fallback).toBe(true); expect(unknownItem.item_ids.body).toBe('missing')
  fallback.release(); approved.dispose()
})
