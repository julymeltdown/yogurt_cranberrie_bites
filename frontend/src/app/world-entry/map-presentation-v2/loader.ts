import { contentHash, MapError, requireMap, sha256 } from '../../../world/map/json'
import { canonicalPresentationJson } from '../../../world/reference-scene/map4-service-v2/presentation-json'
import { abortable, createAssetResources, decodeImage, pngDimensions, type LoadedMap } from '../../../world/map/resources'
import { requireReferenceScene } from '../../../world/reference-scene/model'
import type { ReferenceMapCompositionPaint, ReferenceMapPaintProvider } from '../../../world/reference-scene/overview'
import { map4Parade, map4ThemeHashes, matchesMap4Release } from '../map-presentation-v1/identity'
import { decodeCompositionLayout } from '../../../world/reference-scene/map4-service-v2/contract'
import { compositionDigest, type CompositionModel } from '../../../world/reference-scene/map4-service-v2/model'
import { compositionVersion } from '../../../world/reference-scene/map4-service-v2/semantics'
import { prepareCompositionLabels } from '../../../world/reference-scene/map4-service-v2/labels'

/** A closed Map-only reader; scene resources never enter the WORLD allowlist. */
export function createMapCompositionProvider(model: CompositionModel, options: {
  origin: string; fetch?: typeof fetch; decode?: typeof decodeImage; labels?: typeof prepareCompositionLabels
}): ReferenceMapPaintProvider<ReferenceMapCompositionPaint> {
  const registration = model.registration
  const layoutResource = Object.freeze({ path: registration.layout.path, sha256: registration.layout.sha256, kind: 'json' as const })
  const assets = registration.assets.map(asset => Object.freeze({ path: asset.path, sha256: asset.sha256, kind: 'png' as const }))
  const resources = createAssetResources({ origin: options.origin, allow: [layoutResource, ...assets], ...(options.fetch ? { fetch: options.fetch } : {}) })
  return async (map: LoadedMap, signal: AbortSignal): Promise<ReferenceMapCompositionPaint | null> => {
    if (signal.aborted) throw new MapError('cancelled')
    if (!matchesMap4Release(map)) return null
    requireReferenceScene(map)
    requireMap(await contentHash(map.map) === map4Parade.mapHash
      && await contentHash(map.theme) === map4ThemeHashes[map.theme.theme_id as keyof typeof map4ThemeHashes]
      && await compositionDigest(registration) === registration.presentationHash)
    if (signal.aborted) throw new MapError('cancelled')
    const layoutBytes = await resources.read(layoutResource, signal)
    requireMap(layoutBytes.byteLength === registration.layout.bytes && await sha256(layoutBytes) === registration.layout.sha256)
    const document: unknown = JSON.parse(new TextDecoder('utf-8', { fatal: true }).decode(layoutBytes))
    decodeCompositionLayout(document, registration)
    requireMap(canonicalPresentationJson(document) === model.source)
    const selected = registration.assets.find(asset => asset.themeId === map.theme.theme_id)!
    const resource = assets.find(asset => asset.path === selected.path)!
    const bytes = await resources.read(resource, signal)
    requireMap(bytes.byteLength === selected.bytes && await sha256(bytes) === selected.sha256)
    pngDimensions(bytes, 768, 1296)
    const controller = new AbortController(), combined = AbortSignal.any([signal, controller.signal])
    let decoded: Awaited<ReturnType<typeof decodeImage>> | null = null, closed = false
    const pending = (options.decode ?? decodeImage)(bytes, combined).then(value => {
      if (closed || combined.aborted) { value.dispose(); throw new MapError('cancelled') }
      decoded = value; return value
    })
    try {
      const scene = await abortable(pending, combined)
      requireMap(scene.image.naturalWidth === 768 && scene.image.naturalHeight === 1296)
      if (combined.aborted) throw new MapError('cancelled')
      const labelsPending = (options.labels ?? prepareCompositionLabels)(model, combined, options).then(labels => {
        if (closed || combined.aborted) { labels.dispose(); throw new MapError('cancelled') }; return labels
      })
      const labels = await abortable(labelsPending, combined)
      let released = false
      return { mode: 'full-composition', scene: scene.image, presentationVersion: compositionVersion,
        presentationHash: registration.presentationHash, drawLabels: labels.draw,
        dispose() { if (released) return; released = true; closed = true; controller.abort(); labels.dispose(); scene.dispose() } }
    } catch (error) { closed = true; controller.abort(); (decoded as Awaited<ReturnType<typeof decodeImage>> | null)?.dispose(); throw error }
  }
}
