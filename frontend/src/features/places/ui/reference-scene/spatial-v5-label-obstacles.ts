import presentation from '../../../../world/reference-scene/spatial-v5/map-labels-source.json'
import { getSpatialV5OverviewMetadata } from '../../../../world/reference-scene/spatial-v5/model'
import type { ReferenceOverviewIdentity } from '../../../../world/reference-scene/metadata'

/** Authored MAP label bounds, projected into the existing image plane; no new destinations. */
export function spatialV5LabelObstacles(identity: ReferenceOverviewIdentity, width: number, height: number) {
  if (!getSpatialV5OverviewMetadata(identity)) return []
  return presentation.labels.map(label => ({
    interactionId: label.interaction_id,
    left: label.rect.x / presentation.dimensions.width * width,
    top: label.rect.y / presentation.dimensions.height * height,
    width: label.rect.width / presentation.dimensions.width * width,
    height: label.rect.height / presentation.dimensions.height * height,
  }))
}

/** Keep the existing 44px controls above the caption, inside a short viewport and off label ink. */
export function spatialV5ZoomBottom(view: { left: number; top: number; width: number; height: number }, labels: ReturnType<typeof spatialV5LabelObstacles>) {
  if (!labels.length) return undefined
  const max = Math.max(105, view.height - 60)
  const choices = [Math.min(200, max)]
  for (let bottom = 105; bottom <= max; bottom += 16) choices.push(bottom)
  function covered(bottom: number) {
    const left = view.left + view.width - 56, top = view.top + view.height + 44 - bottom - 92
    return labels.reduce((area, label) => area + Math.max(0, Math.min(left + 48, label.left + label.width) - Math.max(left, label.left))
      * Math.max(0, Math.min(top + 92, label.top + label.height) - Math.max(top, label.top)), 0)
  }
  return choices.sort((a, b) => covered(a) - covered(b))[0]
}
