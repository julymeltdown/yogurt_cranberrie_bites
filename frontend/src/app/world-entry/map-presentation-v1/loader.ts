import { contentHash, MapError, requireMap, sha256 } from '../../../world/map/json'
import { abortable, createAssetResources, decodeImage, pngDimensions, type LoadedMap } from '../../../world/map/resources'
import { requireReferenceScene } from '../../../world/reference-scene/model'
import type { ReferenceMapPaint, ReferenceMapPaintProvider } from '../../../world/reference-scene/overview'
import { map4Parade, map4ThemeHashes, matchesMap4Release } from './identity'
import { prepareMap4Labels } from '../../../world/reference-scene/map4-service-v1/labels'
import { presentationVersion, presentationHash } from '../../../world/reference-scene/map4-service-v1/annotations'
import { map4PaintSource } from '../../../world/reference-scene/map4-service-v1/paint-source'

export interface MapPaintDescriptor {
  readonly version: 'map4-parade-firstview-v1'
  readonly path: 'world/map-firstview-cozy-v1/parade-ground.png'
  readonly sha256: string
  readonly bytes: number
  readonly plazaFill: string
}

/** Separate closed same-origin resource reader; never widens the WORLD asset reader. */
export function createMapPaintProvider(descriptor: Readonly<MapPaintDescriptor>, options: {
  origin: string; fetch?: typeof fetch; decode?: typeof decodeImage; labels?: typeof prepareMap4Labels
}): ReferenceMapPaintProvider {
  requireMap(descriptor.version === 'map4-parade-firstview-v1'
    && descriptor.path === 'world/map-firstview-cozy-v1/parade-ground.png'
    && /^sha256:[0-9a-f]{64}$/.test(descriptor.sha256)
    && Number.isSafeInteger(descriptor.bytes) && descriptor.bytes > 0 && descriptor.bytes <= 2_097_152
    && /^#[0-9a-f]{6}$/i.test(descriptor.plazaFill))
  requireMap(descriptor.sha256 === map4PaintSource.sha256 && descriptor.bytes === map4PaintSource.bytes
    && descriptor.plazaFill === map4PaintSource.plazaFill)
  const pin = Object.freeze({ ...descriptor })
  const resource = Object.freeze({ path: pin.path, sha256: pin.sha256, kind: 'png' as const })
  const resources = createAssetResources({ origin: options.origin, allow: [resource], ...(options.fetch ? { fetch: options.fetch } : {}) })
  return async (map: LoadedMap, signal: AbortSignal): Promise<ReferenceMapPaint | null> => {
    if (signal.aborted) throw new MapError('cancelled')
    // Unrelated coherent releases keep their own original overview; never try this paint.
    if (!matchesMap4Release(map)) return null
    requireReferenceScene(map)
    requireMap(await contentHash(map.map) === map4Parade.mapHash && await contentHash(map.theme) === map4ThemeHashes[map.theme.theme_id as keyof typeof map4ThemeHashes])
    if (signal.aborted) throw new MapError('cancelled')
    const bytes = await resources.read(resource, signal)
    requireMap(bytes.byteLength === pin.bytes && await sha256(bytes) === pin.sha256)
    pngDimensions(bytes, map4Parade.width, map4Parade.height)
    const controller = new AbortController(), combined = AbortSignal.any([signal, controller.signal])
    const ownership: { paint: ReferenceMapPaint | null } = { paint: null }
    let closed = false
    const pending = (options.decode ?? decodeImage)(bytes, combined).then(decoded => {
      if (closed || combined.aborted) { decoded.dispose(); throw new MapError('cancelled') }
      ownership.paint = { ground: decoded.image, plazaFill: pin.plazaFill, dispose: decoded.dispose }
      return ownership.paint
    })
    try {
      const paint = await abortable(pending, combined)
      requireMap(paint.ground.naturalWidth === map4Parade.width && paint.ground.naturalHeight === map4Parade.height)
      if (combined.aborted) throw new MapError('cancelled')
      const labelsPending = (options.labels ?? prepareMap4Labels)(combined).then(labels => {
        if (closed || combined.aborted) { labels.dispose(); throw new MapError('cancelled') }
        return labels
      })
      const labels = await abortable(labelsPending, combined)
      let released = false
      return { ...paint, presentationVersion, presentationHash, drawLabels: labels.draw,
        dispose() { if (released) return; released = true; closed = true; controller.abort(); labels.dispose(); paint.dispose() } }
    } catch (error) { closed = true; controller.abort(); ownership.paint?.dispose(); throw error }
  }
}
