import { z } from 'zod'
import type { AvatarConfig as WireAvatarConfig } from '../../world/network/transport/wire-types'
import type { PrivacyScope } from '../../shared/query/privacy'

export type AvatarConfig = WireAvatarConfig
export type Layer = keyof AvatarConfig['item_ids']
export const layers: readonly Layer[] = ['body', 'eyes', 'hair', 'outfit', 'accessory']
const assetId = z.string().regex(/^[a-z][a-z0-9_]{0,63}$/)
const configSchema = z.strictObject({ contract_version: z.literal('1.0.0'), kind: z.literal('avatar_config'),
  manifest_id: assetId, manifest_version: z.number().int().min(1).max(Number.MAX_SAFE_INTEGER),
  item_ids: z.strictObject({ body: assetId, eyes: assetId, hair: assetId, outfit: assetId, accessory: assetId.nullable() }), skin_tone_id: assetId })
const avatarSchema = z.strictObject({ revision: z.number().int().min(1).max(Number.MAX_SAFE_INTEGER), config: configSchema })
export interface AvatarState { readonly revision: number; readonly config: AvatarConfig }
export function immutable<T>(value: T): T {
  if (value && typeof value === 'object') { Object.values(value).forEach(immutable); Object.freeze(value) }
  return value
}
export const copyConfig = (config: AvatarConfig): AvatarConfig => immutable(configSchema.parse(config))
export function decodeAvatar(value: unknown): AvatarState { return immutable(avatarSchema.parse(value)) }
export function decodeParticipantAvatar(value: unknown, scope: PrivacyScope): AvatarState {
  const participant = z.object({ event_id: z.literal(scope.eventId), user_id: z.literal(scope.userId),
    profile: z.object({ user_id: z.literal(scope.userId), avatar: avatarSchema }) }).parse(value)
  return immutable(participant.profile.avatar)
}
export const sameConfig = (a: AvatarConfig, b: AvatarConfig) => a.manifest_id === b.manifest_id && a.manifest_version === b.manifest_version
  && a.skin_tone_id === b.skin_tone_id && layers.every(layer => a.item_ids[layer] === b.item_ids[layer])
export interface AvatarCatalog {
  readonly presets: readonly { readonly id: string; readonly config: AvatarConfig }[]
  readonly items: readonly { readonly id: string; readonly layer: Layer }[]
  readonly tones: readonly string[]
}
export interface PreviewLease { readonly canvas: HTMLCanvasElement; dispose(): void }
/** Production adapts the shared FE12 compositor only. Test implementations stay under __tests__. */
export interface AvatarPreviewPort {
  readonly catalog: AvatarCatalog
  /** Reject unknown IDs, mismatched manifests and unavailable assets/skin combinations. */
  supports(config: AvatarConfig): boolean
  render(config: AvatarConfig, signal: AbortSignal): Promise<PreviewLease>
  dispose(): void
}
