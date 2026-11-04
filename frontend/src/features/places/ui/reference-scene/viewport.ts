import type { SceneryOverview } from '../../../../world/illustrated/overview'

/** Presentation bounds only; never an authoritative world-camera transform. */
export function mapViewport(slot: { width: number; height: number }, image: { width: number; height: number }, zoom: number) {
  const baseWidth = Math.max(slot.width, slot.height * image.width / image.height)
  const baseHeight = baseWidth * image.height / image.width
  const minZoom = Math.min(1, slot.width / baseWidth, slot.height / baseHeight)
  const effectiveZoom = Math.max(minZoom, Math.min(3, zoom))
  const width = baseWidth * effectiveZoom, height = baseHeight * effectiveZoom
  return { minZoom, effectiveZoom, width, height,
    offsetX: Math.max(0, (slot.width - width) / 2), offsetY: Math.max(0, (slot.height - height) / 2) }
}

/** A replaced URL/release gets a new image element and local viewport lifetime. */
export function overviewIdentity(image: SceneryOverview) {
  return JSON.stringify([image.mapId, image.mapRevision, image.mapHash, image.geometryHash,
    image.themeId, image.themeRevision, image.themeHash, image.width, image.height, image.imageUrl])
}
