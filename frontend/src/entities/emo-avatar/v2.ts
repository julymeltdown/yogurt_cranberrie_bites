import { requireMotion, type MotionSelection } from './art'
import { strictJson } from '../../world/map/json'

/** Frozen C06-MOTION-V2 producer shape; decoder qualification does not grant inventory admission. */
export const MOTION_V2_FAMILY = 'mcr_selected_motion_20261103' as const
export const MOTION_V2_PRESETS = Object.freeze(['motion_v2_photo01_front_01', 'motion_v2_photo01_front_02',
  'motion_v2_photo01_front_03', 'motion_v2_photo01_front_04', 'motion_v2_photo01_front_05', 'motion_v2_photo01_front_06'] as const)
type Index4 = 0 | 1 | 2 | 3
type Index6 = Index4 | 4 | 5
type Index8 = Index6 | 6 | 7
interface Header {
  readonly contract_version: '2.0.0'; readonly kind: 'avatar_config'
  readonly manifest_id: typeof MOTION_V2_FAMILY; readonly manifest_version: 1
}
export interface MotionV2PresetConfig extends Header {
  readonly representation: 'motion_preset'; readonly preset_id: typeof MOTION_V2_PRESETS[number]
}
export interface MotionV2LayeredConfig extends Header {
  readonly representation: 'motion_layered'
  readonly item_ids: Readonly<{ body: `motion_v2_body_${Index8}`; eyes: `motion_v2_eyes_${Index4}`; hair: `motion_v2_hair_${Index6}`;
    outfit: `motion_v2_outfit_${Index6}` | null; accessory: `motion_v2_accessory_${Index4}` | null }>
}
export type MotionV2Config = MotionV2PresetConfig | MotionV2LayeredConfig
const record = (value: unknown): value is Record<string, unknown> => !!value && typeof value === 'object' && !Array.isArray(value)
const exact = (value: Record<string, unknown>, keys: string) => Object.keys(value).sort().join(',') === keys
const header: Header = Object.freeze({ contract_version: '2.0.0', kind: 'avatar_config', manifest_id: MOTION_V2_FAMILY, manifest_version: 1 })
export function decodeMotionV2Config(value: unknown): MotionV2Config {
  requireMotion(record(value) && value.contract_version === header.contract_version && value.kind === header.kind
    && value.manifest_id === header.manifest_id && value.manifest_version === header.manifest_version)
  if (value.representation === 'motion_preset') {
    requireMotion(exact(value, 'contract_version,kind,manifest_id,manifest_version,preset_id,representation')
      && MOTION_V2_PRESETS.includes(value.preset_id as typeof MOTION_V2_PRESETS[number]))
    return Object.freeze({ ...header, representation: 'motion_preset', preset_id: value.preset_id as MotionV2PresetConfig['preset_id'] })
  }
  requireMotion(value.representation === 'motion_layered' && exact(value, 'contract_version,item_ids,kind,manifest_id,manifest_version,representation')
    && record(value.item_ids) && exact(value.item_ids, 'accessory,body,eyes,hair,outfit'))
  const items = value.item_ids
  for (const [slot, maximum] of [['body', 8], ['eyes', 4], ['hair', 6], ['outfit', 6], ['accessory', 4]] as const) {
    if ((slot === 'outfit' || slot === 'accessory') && items[slot] === null) continue
    requireMotion(Array.from({ length: maximum }, (_, index) => `motion_v2_${slot}_${index}`).includes(items[slot] as string))
  }
  return Object.freeze({ ...header, representation: 'motion_layered', item_ids: Object.freeze({ ...items }) as MotionV2LayeredConfig['item_ids'] })
}
/** Wire text retains lexical validation before JSON.parse can erase duplicate keys or floats. */
export function decodeMotionV2ConfigText(text: string): MotionV2Config {
  requireMotion(new TextEncoder().encode(text).byteLength <= 4096)
  return decodeMotionV2Config(strictJson(text))
}
/** Canonical identifiers map only through this frozen producer namespace; local keys are never serialized. */
export function motionV2Selection(input: unknown): MotionSelection {
  const config = decodeMotionV2Config(input)
  if (config.representation === 'motion_preset') return Object.freeze({ mode: 'preset', preset: `photo01-front-${config.preset_id.slice(-2)}` })
  const key = <T extends string | null>(id: T): string | null => id === null ? null : id.slice('motion_v2_'.length).replace('_', '-')
  return Object.freeze({ mode: 'wardrobe', body: key(config.item_ids.body)!, eyes: key(config.item_ids.eyes)!, hair: key(config.item_ids.hair)!,
    outfit: key(config.item_ids.outfit), accessory: key(config.item_ids.accessory) })
}
export function motionV2Config(selection: MotionSelection): MotionV2Config {
  if (selection.mode === 'preset') {
    requireMotion(/^photo01-front-0[1-6]$/.test(selection.preset))
    return decodeMotionV2Config({ ...header, representation: 'motion_preset', preset_id: `motion_v2_photo01_front_${selection.preset.slice(-2)}` })
  }
  const key = (local: string | null) => local === null ? null : `motion_v2_${local.replace('-', '_')}`
  return decodeMotionV2Config({ ...header, representation: 'motion_layered', item_ids: {
    body: key(selection.body), eyes: key(selection.eyes), hair: key(selection.hair), outfit: key(selection.outfit), accessory: key(selection.accessory),
  } })
}
