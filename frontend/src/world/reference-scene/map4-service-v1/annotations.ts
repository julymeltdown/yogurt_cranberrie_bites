import type { ReferenceOverviewIdentity } from '../metadata'
import type { SceneryOverview } from '../../illustrated/overview'
import { map4Parade, map4ThemeHashes } from './identity'
import { presentationHash } from './pins'
export { presentationHash } from './pins'

export const presentationVersion = 'map4-service-v1'
const routes = Object.freeze({ wall: '/wall', feed: '/feed', meet: '/meet', gifts: '/gifts' } as const)
type Intent = keyof typeof routes
export type Map4PresentationIdentity = ReferenceOverviewIdentity & Partial<Pick<SceneryOverview,
  'mapHash' | 'themeHash' | 'themeRevision' | 'presentationVersion' | 'presentationHash'>>

/** Own Map4 source anchors, never v5 rectangles; all annotations have null canonical authority. */
export const map4Labels = Object.freeze([
  { label: 'MAIN STAGE', icon: 'stage', intent: null, x: 335, y: 147, width: 312 },
  { label: 'PLAZA', icon: null, intent: null, x: 370, y: 461, width: 143 },
  { label: 'WALL', icon: 'wall', intent: 'wall', x: 118, y: 750, width: 221 },
  { label: 'FEED', icon: 'feed', intent: 'feed', x: 658, y: 758, width: 192 },
  { label: 'MEET', icon: 'people', intent: 'meet', x: 372, y: 1024.836257, width: 240 },
  { label: 'GIFTS', icon: 'gift', intent: 'gifts', x: 540, y: 1241.157895, width: 240 },
].map(label => Object.freeze({ ...label, placeId: null, interactionId: null,
  rect: Object.freeze({ x: Math.max(5, Math.min(763 - label.width, Math.round(label.x - label.width / 2))),
    y: Math.max(5, Math.min(1213, Math.round(label.y - 39))), width: label.width, height: 78 }) })))

export function isMap4Presentation(identity: Map4PresentationIdentity) {
  const p = map4Parade
  return identity.mapId === p.mapId && identity.mapRevision === p.mapRevision && identity.mapHash === p.mapHash
    && identity.geometryHash === p.geometryHash && identity.width === p.width && identity.height === p.height
    && Object.hasOwn(map4ThemeHashes, identity.themeId) && identity.themeRevision === 1
    && identity.themeHash === map4ThemeHashes[identity.themeId as keyof typeof map4ThemeHashes]
    && identity.presentationVersion === presentationVersion && identity.presentationHash === presentationHash
}
export function getMap4ServiceAnnotations(identity: Map4PresentationIdentity) {
  return isMap4Presentation(identity) ? map4Labels.flatMap(label => label.intent !== null
    ? [{ intent: label.intent as Intent, label: label.label, path: routes[label.intent as Intent], rect: label.rect }] : []) : []
}
export function map4LabelObstacles(identity: Map4PresentationIdentity, width: number, height: number) {
  return isMap4Presentation(identity) ? map4Labels.map(label => ({ interactionId: null,
    left: label.rect.x / 768 * width, top: label.rect.y / 1296 * height,
    width: label.rect.width / 768 * width, height: label.rect.height / 1296 * height })) : []
}
