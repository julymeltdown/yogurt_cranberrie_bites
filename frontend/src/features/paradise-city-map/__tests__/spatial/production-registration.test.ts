import { execFileSync } from 'node:child_process'
import { existsSync, readFileSync } from 'node:fs'
import { describe, expect, it, vi } from 'vitest'
import { imagePoint } from '../../spatial/contract'
import { createSiteplanLoader, siteplanSHA256 } from '../../spatial/image-resource'
import { productionSiteplanRegistration } from '../../spatial/registration'
import { fitViewport, imageToViewport, viewportToImage } from '../../spatial/viewport'

const producer = '46b4d8670d31365a88b9006250befdcf2832135e'
/** Canonical published files work in clean clones; immutable Git is this older-base
 * checkout's read-only qualification fallback. Both modes face the same pins. */
function asset(path: string) {
  const canonical = new URL('../../../../../public/' + path, import.meta.url)
  if (existsSync(canonical)) return new Uint8Array(readFileSync(canonical))
  return new Uint8Array(execFileSync('git', ['show', producer + ':frontend/public/' + path], { maxBuffer: 1024 * 1024 }))
}
function resources(manifest: Uint8Array, image: Uint8Array) {
  return vi.fn<typeof fetch>(async input => new Response(new Uint8Array(String(input).endsWith('asset-interface.json') ? manifest : image).buffer))
}
const origin = 'https://mcr.example.test'

describe('qualified ART immutable registration consumption', () => {
  it('binds the exact full-board pack and all 22 source landmarks through the actual loader/model', async () => {
    const registration = productionSiteplanRegistration, manifest = asset(registration.manifest.path), image = asset(registration.image.path)
    expect(manifest.byteLength).toBe(registration.manifest.bytes); expect(await siteplanSHA256(manifest)).toBe(registration.manifest.sha256)
    expect(image.byteLength).toBe(registration.image.bytes); expect(await siteplanSHA256(image)).toBe(registration.image.sha256)
    const dispose = vi.fn(), fetch = resources(manifest, image)
    const loader = createSiteplanLoader(registration, { origin, fetch, decode: async () => ({ source: {} as CanvasImageSource, naturalWidth: 1024, naturalHeight: 1280, dispose }) })
    try {
      const loaded = await loader.load(), plan = loaded.snapshot.siteplan
      expect(loaded.producer).toEqual({ kind: 'QUALIFIED_PRODUCER', sourceRevision: producer })
      expect(plan.landmarks.map(point => point.id)).toEqual(registration.acceptedLandmarkIds)
      expect(plan.landmarks).toHaveLength(22); expect(fetch).toHaveBeenCalledTimes(2)
      expect(loaded.snapshot.geographicProjection.state).toBe('unavailable')
      for (const landmark of plan.landmarks) {
        expect(loaded.model.landmark(landmark.id)).toBe(landmark)
        for (const width of [360, 390, 430]) {
          const transform = fitViewport(plan.image, { width, height: 600 })
          const inverse = viewportToImage(imageToViewport(landmark.anchor, transform), transform, plan.image)
          expect(inverse?.x).toBeCloseTo(landmark.anchor.x, 8); expect(inverse?.y).toBeCloseTo(landmark.anchor.y, 8)
        }
        expect(loaded.model.hitTest(imagePoint(landmark.anchor.x, landmark.anchor.y))?.id).toBe(landmark.id)
      }
      expect(loaded.model.landmark('photo-zone').sourceRefs).toEqual(['photo-5', 'photo-10'])
    } finally { loader.dispose() }
    expect(dispose).toHaveBeenCalledTimes(1)
  })
  it('rejects a valid-looking anchor change against the qualified manifest pin before image decoding', async () => {
    const registration = productionSiteplanRegistration, manifest = asset(registration.manifest.path)
    const document = JSON.parse(new TextDecoder().decode(manifest)); document.landmarks[0].anchor.x++
    const changed = new TextEncoder().encode(JSON.stringify(document, null, 2) + '\n'), decode = vi.fn()
    const loader = createSiteplanLoader(registration, { origin, fetch: resources(changed, asset(registration.image.path)), decode })
    try { await expect(loader.load()).rejects.toMatchObject({ code: 'HASH_MISMATCH' }); expect(decode).not.toHaveBeenCalled() } finally { loader.dispose() }
  })
  it('rejects changed JPEG bytes before calling the injected decoder', async () => {
    const registration = productionSiteplanRegistration, image = asset(registration.image.path); image[100] = image[100]! ^ 1
    const decode = vi.fn(), loader = createSiteplanLoader(registration, { origin, fetch: resources(asset(registration.manifest.path), image), decode })
    try { await expect(loader.load()).rejects.toMatchObject({ code: 'HASH_MISMATCH' }); expect(decode).not.toHaveBeenCalled() } finally { loader.dispose() }
  })
})
