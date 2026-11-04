import presentation from '../../../../world/reference-scene/spatial-v4/map-labels-source.json'
import { getSpatialV4OverviewMetadata } from '../../../../world/reference-scene/spatial-v4/model'
import type { ReferenceOverviewIdentity } from '../../../../world/reference-scene/metadata'

/** Authored MAP label bounds, projected into the existing image plane; no new destinations. */
export function spatialV4LabelObstacles(identity: ReferenceOverviewIdentity, width: number, height: number) {
  if (!getSpatialV4OverviewMetadata(identity)) return []
  return presentation.labels.map(label => ({
    interactionId: label.interaction_id,
    left: label.rect.x / presentation.dimensions.width * width,
    top: label.rect.y / presentation.dimensions.height * height,
    width: label.rect.width / presentation.dimensions.width * width,
    height: label.rect.height / presentation.dimensions.height * height,
  }))
}
