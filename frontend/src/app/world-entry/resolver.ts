import type { SessionController } from '../../features/auth/model/session'
import { list, object, requireValue, text } from '../../features/auth/model/contract'
import { isThemeId, type ThemeId } from '../../features/theme/model/preferences'
import { validateWorldRooms } from '../../features/world-rooms/model/contract'
import { characterSourcePack } from '../../entities/avatar/source-catalog'
import { immutable } from '../../world/entities/replicated'
import { validatePolicy, type SimulationPolicy } from '../../world/input/simulation'
import { createAssetResources, type AssetResources, type Resource } from '../../world/map/resources'
import { validateManifests } from '../../world/map/manifest'
import { canonicalJson } from '../../features/auth/model/bootstrap'
import type { Geometry } from '../../world/map/manifest'
import type { ResolvedWorldRelease, WorldReleaseResolver } from '../../world/composition'
import { assertEntry, captureEntry, validateEntry, type WorldEntryRequest } from './authority'

export interface LocalInputConfiguration {
  /** Reviewed application configuration, not a claimed C05 authoritative movement model. */
  readonly revision: string
  readonly reviewedBy: string
  readonly movementPolicy: SimulationPolicy
  readonly zoom: number
}
export type WorldEntryRelease = ResolvedWorldRelease
export interface WorldEntryResolverOptions {
  readonly auth: SessionController
  readonly origin: string
  readonly localInput: LocalInputConfiguration
  /** Optional fetch seam for the existing same-origin, hash-checked asset reader only. */
  readonly assetFetch?: typeof fetch
}
function reviewedArt(document: Record<string, unknown>) {
  const provenance = object(document.provenance)
  requireValue(provenance.source_kind === 'ORIGINAL_ART' && provenance.review_state === 'REVIEWED'
    && text(provenance.reviewed_by).trim().length > 0 && text(provenance.reviewed_at).length > 0)
  return provenance
}
function provided(value: unknown, kind: Resource['kind']): Resource {
  const r = object(value)
  requireValue((kind === 'json' ? r.resource_state : r.image_state) === 'PROVIDED')
  // createAssetResources independently enforces the strict world/ path, extension, hash and uniqueness rules.
  return Object.freeze({ path: text(r.path), sha256: text(r.sha256), kind })
}
/** No default release, bootstrap refetch, ticket/admission claim, local fixture or art fallback. */
export function createWorldEntryResolver(options: WorldEntryResolverOptions): WorldReleaseResolver {
  const { auth } = options
  const configuration = immutable(structuredClone(options.localInput))
  requireValue(configuration.revision.trim().length > 0 && configuration.reviewedBy.trim().length > 0)
  const origin = options.origin, assetFetch = options.assetFetch
  return async function resolve(input: WorldEntryRequest): Promise<WorldEntryRelease> {
    const captured = captureEntry(input)
    return auth.privacy.run(captured.privacyScope, async ({ signal }) => {
      const document = await validateEntry(auth, captured)
      const map = object(document.map_manifest), avatar = object(document.avatar_manifest)
      const themeId = object(object(document.participant).preferences).theme
      requireValue(isThemeId(themeId))
      const theme = list(document.theme_manifests).map(object).find(t => t.theme_id === themeId)
      requireValue(theme)
      reviewedArt(map); reviewedArt(theme)
      const provenance = reviewedArt(avatar)
      requireValue(provenance.source_id === characterSourcePack.source_pack_id)
      // Never carry C06 synthetic place metadata into a production map release.
      requireValue(list(map.places).map(object).every(place => object(place.provenance).source_kind !== 'ORIGINAL_FIXTURE'))
      const allow = [provided(map.tilemap, 'json'), provided(theme.texture, 'png'), provided(object(avatar.atlas).texture, 'png')]
      const availableThemeIds: ThemeId[] = [themeId]
      for (const candidate of list(document.theme_manifests).map(object)) {
        if (candidate === theme) continue
        // Optional skins gain only their original C03 descriptor, never a caller URL or art fallback.
        try {
          reviewedArt(candidate)
          const resource = provided(candidate.texture, 'png')
          await validateManifests(canonicalJson(map), canonicalJson(candidate), { mapHash: text(map.content_hash), themeHash: text(candidate.content_hash),
            avatar: { manifest_id: text(avatar.manifest_id), manifest_version: Number(avatar.manifest_version), content_hash: text(avatar.content_hash) } })
          const existing = allow.find(item => item.path === resource.path)
          requireValue(!existing || existing.sha256 === resource.sha256 && existing.kind === resource.kind)
          if (!existing) allow.push(resource)
          requireValue(isThemeId(candidate.theme_id)); availableThemeIds.push(candidate.theme_id)
        } catch { /* An unqualified optional skin remains unavailable; selected release checks above stay mandatory. */ }
      }
      const assets = createAssetResources({ origin, allow, ...(assetFetch ? { fetch: assetFetch } : {}) })
      validatePolicy(object(map.geometry) as unknown as Geometry, configuration.movementPolicy)
      requireValue(Number.isFinite(configuration.zoom) && configuration.zoom >= 1 && configuration.zoom <= 8)
      // No resource pin is guessed; C03 validated the hashes and cross-manifest bindings locally above.
      assertEntry(auth, captured)
      const response = await auth.request({ path: `/api/concert/events/${captured.scope.eventId}/rooms`, method: 'GET',
        access: 'protected', errorFamily: 'core', signal, decode: validateWorldRooms(captured.scope) })
      assertEntry(auth, captured)
      requireValue(auth.session.isCurrent(response.scope))
      const rooms = response.data
      // The default is a catalog choice only. The canonical ticket/snapshot still decides admission, including full rooms.
      const resources: AssetResources = Object.freeze({ read(resource: Resource, readSignal: AbortSignal) {
        const pinned = Object.freeze({ path: resource.path, sha256: resource.sha256, kind: resource.kind })
        const combined = AbortSignal.any([captured.signal, readSignal])
        return auth.privacy.run(captured.privacyScope, ({ signal: leaseSignal }) => {
          assertEntry(auth, captured); return assets.read(pinned, leaseSignal)
        }, combined)
      } })
      return Object.freeze({ availableThemeIds: Object.freeze(availableThemeIds), privacyScope: captured.privacyScope, bootstrap: captured.bootstrap.document!, themeId,
        roomId: rooms.default_room_id, rooms, resources, movementPolicy: configuration.movementPolicy, zoom: configuration.zoom })
    }, captured.signal)
  }
}
