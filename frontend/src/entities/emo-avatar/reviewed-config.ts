import { strictJson } from '../../world/map/json'
import { requireMotion, type MotionSelection } from './art'
import { MOTION_V2_PRESETS, type MotionV2LayeredConfig } from './v2'

/** Producer 0dd9f443: a distinct identity/revision. Candidate configs never become reviewed configs. */
export const REVIEWED_MOTION_FAMILY = 'mcr_selected_motion_reviewed_20261104' as const
interface Header {
  readonly contract_version: '2.0.0'; readonly kind: 'avatar_config'
  readonly manifest_id: typeof REVIEWED_MOTION_FAMILY; readonly manifest_version: 2
}
export interface ReviewedMotionPresetConfig extends Header {
  readonly representation: 'motion_preset'; readonly preset_id: typeof MOTION_V2_PRESETS[number]
}
export interface ReviewedMotionLayeredConfig extends Header {
  readonly representation: 'motion_layered'
  /** The successor retains the producer's closed item vocabulary, not the candidate identity. */
  readonly item_ids: MotionV2LayeredConfig['item_ids']
}
export type ReviewedMotionConfig = ReviewedMotionPresetConfig | ReviewedMotionLayeredConfig
const header: Header = Object.freeze({ contract_version: '2.0.0', kind: 'avatar_config', manifest_id: REVIEWED_MOTION_FAMILY, manifest_version: 2 })
const record = (value: unknown): value is Record<string, unknown> => !!value && typeof value === 'object' && !Array.isArray(value)
const exact = (value: Record<string, unknown>, keys: string) => Object.keys(value).sort().join(',') === keys
export function decodeReviewedMotionConfig(value: unknown): ReviewedMotionConfig {
  requireMotion(record(value) && value.contract_version === header.contract_version && value.kind === header.kind
    && value.manifest_id === header.manifest_id && value.manifest_version === header.manifest_version)
  if (value.representation === 'motion_preset') {
    requireMotion(exact(value, 'contract_version,kind,manifest_id,manifest_version,preset_id,representation')
      && MOTION_V2_PRESETS.includes(value.preset_id as typeof MOTION_V2_PRESETS[number]))
    return Object.freeze({ ...header, representation: 'motion_preset', preset_id: value.preset_id as ReviewedMotionPresetConfig['preset_id'] })
  }
  requireMotion(value.representation === 'motion_layered' && exact(value, 'contract_version,item_ids,kind,manifest_id,manifest_version,representation')
    && record(value.item_ids) && exact(value.item_ids, 'accessory,body,eyes,hair,outfit'))
  const items = value.item_ids
  for (const [slot, maximum] of [['body', 8], ['eyes', 4], ['hair', 6], ['outfit', 6], ['accessory', 4]] as const) {
    if ((slot === 'outfit' || slot === 'accessory') && items[slot] === null) continue
    requireMotion(Array.from({ length: maximum }, (_, i) => `motion_v2_${slot}_${i}`).includes(items[slot] as string))
  }
  return Object.freeze({ ...header, representation: 'motion_layered', item_ids: Object.freeze({ ...items }) as ReviewedMotionLayeredConfig['item_ids'] })
}
export function decodeReviewedMotionConfigText(text: string): ReviewedMotionConfig {
  requireMotion(new TextEncoder().encode(text).byteLength <= 4096)
  return decodeReviewedMotionConfig(strictJson(text))
}
export function reviewedMotionSelection(value: unknown): MotionSelection {
  const config = decodeReviewedMotionConfig(value)
  if (config.representation === 'motion_preset') return Object.freeze({ mode: 'preset', preset: `photo01-front-${config.preset_id.slice(-2)}` })
  const key = (id: string | null) => id === null ? null : id.slice('motion_v2_'.length).replace('_', '-')
  return Object.freeze({ mode: 'wardrobe', body: key(config.item_ids.body)!, eyes: key(config.item_ids.eyes)!, hair: key(config.item_ids.hair)!,
    outfit: key(config.item_ids.outfit), accessory: key(config.item_ids.accessory) })
}
export function reviewedMotionConfig(selection: MotionSelection): ReviewedMotionConfig {
  if (selection.mode === 'preset') {
    requireMotion(/^photo01-front-0[1-6]$/.test(selection.preset))
    return decodeReviewedMotionConfig({ ...header, representation: 'motion_preset', preset_id: `motion_v2_photo01_front_${selection.preset.slice(-2)}` })
  }
  const key = (value: string | null) => value === null ? null : `motion_v2_${value.replace('-', '_')}`
  return decodeReviewedMotionConfig({ ...header, representation: 'motion_layered', item_ids: {
    body: key(selection.body), eyes: key(selection.eyes), hair: key(selection.hair), outfit: key(selection.outfit), accessory: key(selection.accessory),
  } })
}
