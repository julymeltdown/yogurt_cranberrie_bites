import type { ActiveScope, ShellBootstrap } from '../../app/ports'
import { canonicalJson, decodeBootstrap } from '../../features/auth/model/bootstrap'
import { decodeContract, list, number, object, requireValue, text } from '../../features/auth/model/contract'
import type { PrivacyScope } from '../../shared/query/privacy'
import { validateManifests } from '../map/manifest'
import type { AssetResources } from '../map/resources'
import type { SimulationPolicy } from '../input/simulation'
import { validatePolicy } from '../input/simulation'
import type { WorldResources } from '../network/WorldTransport'
import type { WorldRooms } from '../network/WorldTransport'
import { validate } from '../network/transport/contract'
import { immutable } from '../entities/replicated'

/** The integrator supplies an authorized C03 bundle, never caller-selected asset URLs.
 * resources must be the reviewed release's hash/path allowlist. No default release exists.
 */
export interface ResolvedWorldRelease {
  readonly privacyScope: PrivacyScope
  readonly bootstrap: unknown
  readonly themeId: 'REVENGE' | 'PARADE' | 'DANGER'
  readonly roomId: string
  /** C05 room read authorized by the same privacy lease; tickets still decide admission. */
  readonly rooms: WorldRooms
  readonly resources: AssetResources
  /** FE15's local intent/pathfinding policy; never an authoritative C05 physics model. */
  readonly movementPolicy: SimulationPolicy
  readonly zoom: number
}
export interface WorldReleaseRequest {
  readonly scope: ActiveScope
  readonly privacyScope: PrivacyScope
  readonly bootstrap: ShellBootstrap
  readonly signal: AbortSignal
}
export type WorldReleaseResolver = (request: WorldReleaseRequest) => Promise<ResolvedWorldRelease>
export function samePrivacy(a: PrivacyScope, b: PrivacyScope) {
  return a.userId === b.userId && a.eventId === b.eventId && a.generation === b.generation
    && a.policyGeneration === b.policyGeneration && a.gate === b.gate
}
export async function prepareRelease(input: ResolvedWorldRelease, expected: Omit<WorldReleaseRequest, 'signal'>) {
  requireValue(samePrivacy(input.privacyScope, expected.privacyScope))
  const { roomId, themeId, resources: assets, zoom } = input
  const movementPolicy = Object.freeze({ ...input.movementPolicy })
  const rooms = immutable(structuredClone(validate<WorldRooms>('WorldRooms', input.rooms)))
  requireValue(rooms.event_id === expected.scope.eventId && new Set(rooms.items.map(room => room.room_id)).size === rooms.items.length
    && rooms.items.some(room => room.room_id === rooms.default_room_id) && rooms.items.some(room => room.room_id === roomId))
  // Clone before async hashing so a resolver cannot mutate a half-validated bundle.
  const bundle = structuredClone(decodeContract('Bootstrap', input.bootstrap))
  requireValue(expected.bootstrap.document && canonicalJson(bundle) === canonicalJson(expected.bootstrap.document))
  const projection = await decodeBootstrap(bundle, expected.scope)
  requireValue(canonicalJson(projection) === canonicalJson(expected.bootstrap))
  const map = object(bundle.map_manifest), avatar = object(bundle.avatar_manifest)
  const theme = list(bundle.theme_manifests).map(object).find(theme => theme.theme_id === themeId)
  requireValue(theme)
  const pin = { mapHash: text(map.content_hash), themeHash: text(theme.content_hash), avatar: {
    manifest_id: text(avatar.manifest_id), manifest_version: number(avatar.manifest_version), content_hash: text(avatar.content_hash),
  } }
  const mapText = canonicalJson(map), themeText = canonicalJson(theme)
  const metadata = await validateManifests(mapText, themeText, pin)
  validatePolicy(metadata.map.geometry, movementPolicy)
  requireValue(Number.isFinite(zoom) && zoom >= 1 && zoom <= 8)
  const resources: WorldResources = Object.freeze({ eventId: expected.scope.eventId, roomId,
    mapId: metadata.map.map_id, mapRevision: metadata.map.map_revision,
    avatarManifestId: pin.avatar.manifest_id, avatarManifestVersion: pin.avatar.manifest_version })
  return { resources, rooms, mapInput: { mapText, themeText, pin, resources: assets },
    avatarManifest: immutable(avatar), movementPolicy, zoom }
}
export type PreparedWorldRelease = Awaited<ReturnType<typeof prepareRelease>>
