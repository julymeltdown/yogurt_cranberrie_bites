import { getSpatialOverviewMetadata } from './spatial-v3/model'
import { getSpatialV5OverviewMetadata } from './spatial-v5/model'
import { getSpatialV4OverviewMetadata } from './spatial-v4/model'
/** Frozen projection of artist milestone 8f89d190; these are virtual illustration coordinates. */
export const referenceSceneIdentity = Object.freeze({
  sourceRevision: '8f89d1905a86143d34ff77d2951fc60651ab2497',
  mapId: 'mcr_reference_scene_r3', mapRevision: 2, themeId: 'PARADE',
  geometryHash: 'sha256:20168a1df64cd3c72ea3b8cb576ca895dcff8c7edfb0e77886cc647fb1df37e5',
  width: 768, height: 1296,
})
export interface ReferenceOverviewIdentity {
  readonly geometryHash: string; readonly width: number; readonly height: number; readonly themeId: string
  readonly mapId?: string; readonly mapRevision?: number
}
export interface ReferenceOverviewLabel { readonly label: string; readonly interactionId: string | null; readonly x: number; readonly y: number }
const mapLabels: readonly ReferenceOverviewLabel[] = Object.freeze([
  Object.freeze({ label: 'MAIN STAGE', x: 434, y: 370, interactionId: 'stage_info' }),
  Object.freeze({ label: 'PLAZA', x: 370, y: 640, interactionId: null }),
  Object.freeze({ label: 'MERCH', x: 118, y: 929, interactionId: 'gifts_info' }),
  Object.freeze({ label: 'FOOD', x: 658, y: 937, interactionId: null }),
  Object.freeze({ label: 'MEETUP ZONE', x: 372, y: 1118, interactionId: null }),
  Object.freeze({ label: 'GIVEAWAY BOOTH', x: 540, y: 1260, interactionId: null }),
])
const metadata = Object.freeze({ ...referenceSceneIdentity, mapLabels })
/** Illustration offsets only. Callers still resolve current catalog authorization and actual place IDs. */
export function getReferenceOverviewMetadata(identity: ReferenceOverviewIdentity) {
  return identity.geometryHash === metadata.geometryHash && identity.width === metadata.width
    && identity.height === metadata.height && identity.themeId === metadata.themeId ? metadata
    : getSpatialV5OverviewMetadata(identity) ?? getSpatialV4OverviewMetadata(identity) ?? getSpatialOverviewMetadata(identity)
}
