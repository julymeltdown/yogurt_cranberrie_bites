import type { ReviewedAvatarBinding, ReviewedAtlasLeases } from '../../entities/avatar/runtime'
import type { AssetResources } from '../../world/map/resources'
import type { PrivacyScope } from '../../shared/query/privacy'
import type { ShellBootstrap } from '../ports'
import type { ReviewedThumbnailSource } from '../../features/me'
/** Explicit local reviewed release inputs. No defaults, manifest replacement, or independent identity. */
export interface AccountAssetsPort {
  readonly resources: AssetResources
  readonly binding: ReviewedAvatarBinding
}
/** Use the exact validated bootstrap document and original authorizing lease. */
export function thumbnailSource(bootstrap: ShellBootstrap, scope: PrivacyScope, assets?: AccountAssetsPort, leases?: ReviewedAtlasLeases): ReviewedThumbnailSource | undefined {
  if (!assets) return undefined
  const manifest = bootstrap.document?.avatar_manifest
  if (!manifest || typeof manifest !== 'object' || Array.isArray(manifest)) return undefined
  const value = manifest as Record<string, unknown>
  if (typeof value.manifest_id !== 'string' || !Number.isSafeInteger(value.manifest_version) || Number(value.manifest_version) < 1
    || typeof value.content_hash !== 'string' || !/^sha256:[a-f0-9]{64}$/.test(value.content_hash)) return undefined
  return Object.freeze({ scope, options: Object.freeze({ document: manifest,
    pin: Object.freeze({ manifest_id: value.manifest_id, manifest_version: Number(value.manifest_version), content_hash: value.content_hash }),
    binding: assets.binding, resources: assets.resources, ...(leases ? { leases } : {}) }) })
}
