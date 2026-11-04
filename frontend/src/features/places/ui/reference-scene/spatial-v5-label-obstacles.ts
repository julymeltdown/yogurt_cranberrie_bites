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

/** Position the existing controls inside the image viewport, independently of the caption row. */
export function spatialV5ZoomTop(view: { left: number; top: number; width: number; height: number }, labels: ReturnType<typeof spatialV5LabelObstacles>) {
  if (!labels.length) return undefined
  const max = Math.max(8, view.height - 100)
  const choices = [Math.max(8, max - 100)]
  for (let top = 8; top <= max; top += 16) choices.push(top)
  function covered(top: number) {
    const left = view.left + view.width - 56, y = view.top + top
    return labels.reduce((area, label) => area + Math.max(0, Math.min(left + 48, label.left + label.width) - Math.max(left, label.left))
      * Math.max(0, Math.min(y + 92, label.top + label.height) - Math.max(y, label.top)), 0)
  }
  return choices.sort((a, b) => covered(a) - covered(b))[0]
}
