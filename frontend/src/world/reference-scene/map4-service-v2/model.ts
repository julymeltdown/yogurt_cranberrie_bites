import type { SceneryOverview } from '../../illustrated/overview'
import { sha256 } from '../../map/json'
import { canonicalPresentationJson } from './presentation-json'
import { compositionStyle as style } from './style'
import { map4Parade, map4ThemeHashes } from '../map4-service-v1/identity'
import { decodeCompositionLayout, type CompositionRegistration, type CompositionLayout } from './contract'
import { compositionVersion, serviceSemantics } from './semantics'

export type CompositionIdentity = Partial<Pick<SceneryOverview, 'width' | 'height' | 'mapId' | 'mapRevision' | 'mapHash'
  | 'geometryHash' | 'themeId' | 'themeRevision' | 'themeHash' | 'presentationVersion' | 'presentationHash'>>
export async function compositionDigest(registration: CompositionRegistration) {
  return sha256(new TextEncoder().encode(canonicalPresentationJson({ version: compositionVersion, layout: registration.layout,
    assets: registration.assets, semantics: serviceSemantics, style })))
}
function immutable<T>(value: T): T {
  if (value && typeof value === 'object') { Object.values(value).forEach(immutable); Object.freeze(value) }
  return value
}
export function createCompositionModel(document: unknown, registration: CompositionRegistration) {
  const parsed = decodeCompositionLayout(document, registration)
  // Own metadata bytes before any asynchronous resource/label preparation.
  const source = canonicalPresentationJson(document), pin = immutable(structuredClone(registration))
  const labels = Object.freeze(serviceSemantics.map(semantic => {
    const anchor = parsed.serviceLabelAnchors.find(a => a.key === semantic.key)!
    return Object.freeze({ ...semantic, ...anchor,
      // Stable transparent hit rectangle, independent of compact measured paint.
      rect: Object.freeze({ x: anchor.x - anchor.width / 2, y: anchor.y - 39, width: anchor.width, height: 78 }) })
  }))
  function matches(identity: CompositionIdentity) {
    return identity.mapId === map4Parade.mapId && identity.mapRevision === 4 && identity.mapHash === map4Parade.mapHash
      && identity.geometryHash === map4Parade.geometryHash && identity.width === 768 && identity.height === 1296
      && typeof identity.themeId === 'string' && Object.hasOwn(map4ThemeHashes, identity.themeId)
      && identity.themeRevision === 1 && identity.themeHash === map4ThemeHashes[identity.themeId as keyof typeof map4ThemeHashes]
      && identity.presentationVersion === compositionVersion && identity.presentationHash === pin.presentationHash
  }
  function annotations(identity: CompositionIdentity) {
    return matches(identity) ? labels.flatMap(label => label.intent !== null && label.path !== null
      ? [{ intent: label.intent, path: label.path, label: label.label, rect: label.rect }] : []) : []
  }
  function targets(identity: CompositionIdentity, width: number, height: number) {
    return annotations(identity).map(label => {
      const w = Math.max(44, label.rect.width / 768 * width), h = Math.max(44, label.rect.height / 1296 * height)
      return { intent: label.intent, label: label.label,
        left: Math.max(0, Math.min(width - w, (label.rect.x + label.rect.width / 2) / 768 * width - w / 2)),
        top: Math.max(0, Math.min(height - h, (label.rect.y + label.rect.height / 2) / 1296 * height - h / 2)), width: w, height: h }
    })
  }
  function obstacles(identity: CompositionIdentity, width: number, height: number) {
    return matches(identity) ? labels.map(label => ({ interactionId: null, left: label.exclusionRect.x / 768 * width,
      top: label.exclusionRect.y / 1296 * height, width: label.exclusionRect.width / 768 * width,
      height: label.exclusionRect.height / 1296 * height })) : []
  }
  function displayPoint(identity: CompositionIdentity, interactionId: string, width: number, height: number) {
    const anchor = matches(identity) ? parsed.canonicalDisplayAnchors.find(a => a.interactionId === interactionId) : null
    return anchor ? { x: anchor.x / 768 * width, y: anchor.y / 1296 * height } : null
  }
  return Object.freeze({ registration: pin, source, parsed, labels, matches, annotations, targets, obstacles, displayPoint })
}
export type CompositionModel = ReturnType<typeof createCompositionModel>
export type { CompositionLayout }
