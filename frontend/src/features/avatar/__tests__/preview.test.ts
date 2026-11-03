// @vitest-environment jsdom
import { describe, expect, it, vi } from 'vitest'
import manifest from '../../../../../contracts/fixtures/assets/avatar-manifest.json'
import { createReviewedAvatarPreview, compositorPreview } from '../preview'
import { original, alternate, unsupported } from './fixtures/harness'
import { validateManifest } from '../../../entities/avatar/runtime/contract'
import { canonicalJson } from '../../../world/map/json'
import type { AvatarCompositor } from '../../../entities/avatar/runtime'
describe('FE13 shared compositor adapter', () => {
  it('fails closed without production artwork and never fetches unapproved assets', async () => {
    const read = vi.fn(), lifetime = new AbortController()
    await expect(createReviewedAvatarPreview({ document: manifest, pin: manifest, resources: { read }, signal: lifetime.signal })).rejects.toMatchObject({ reason: 'unavailable' })
    expect(read).not.toHaveBeenCalled()
  })
  it('uses shared validation, rejects unknown IDs/versions/unsupported tones and disposes fallback thumbnails', async () => {
    const checked = await validateManifest(canonicalJson(manifest), manifest), lifetime = new AbortController(), canvas = document.createElement('canvas'), disposeThumbnail = vi.fn()
    let fallback = false
    const thumbnail = vi.fn(() => ({ canvas, fallback, dispose: disposeThumbnail })), dispose = vi.fn()
    const compositor: AvatarCompositor = { manifest: checked, thumbnail, dispose, read: () => ({ entries: 0, references: 0, created: 0, removed: 0, disposed: false }), acquire() { throw new Error('Must not bypass shared thumbnail') } }
    const port = compositorPreview(compositor, 'tone_fixture', lifetime.signal)
    expect(port.supports(original)).toBe(true)
    expect(port.supports(alternate)).toBe(false); expect(port.supports(unsupported)).toBe(false)
    expect(port.supports({ ...original, manifest_version: 2 })).toBe(false)
    expect(port.supports({ ...original, item_ids: { ...original.item_ids, hair: 'photo_01' } })).toBe(false)
    const selected = new AbortController(), lease = await port.render(original, selected.signal)
    expect(thumbnail).toHaveBeenCalledWith(original, { direction: 'down', animation: 'idle', frame: 0 }, selected.signal)
    lease.dispose(); expect(disposeThumbnail).toHaveBeenCalledOnce()
    fallback = true
    await expect(port.render(original, selected.signal)).rejects.toMatchObject({ reason: 'unavailable' })
    expect(disposeThumbnail).toHaveBeenCalledTimes(2)
    lifetime.abort(); expect(dispose).toHaveBeenCalledOnce(); expect(port.supports(original)).toBe(false)
    port.dispose(); expect(dispose).toHaveBeenCalledOnce()
  })
})
