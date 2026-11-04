import type { AvatarState as WireAvatarState } from '../../../world/network/transport/wire-types'
import { immutable } from '../../../world/entities/replicated'
import { canonicalJson, record, strictJson } from '../../../world/map/json'
import {
  AvatarError, ensure, validateConfig as validateLayeredConfig, validateManifest as validateLayeredManifest,
  type AvatarManifest as LayeredAvatarManifest,
} from '../runtime/contract'
import { schemaValid } from '../runtime/schema'
import { validateWholeCharacterManifest, type WholeCharacterManifest } from '../whole-character/contract'
import { decodeReviewedMotionConfig, type ReviewedMotionConfig } from '../runtime/motion-adoption/reviewed-config'
import { decodeReviewedMotionManifest, type ReviewedMotionManifest } from '../runtime/motion-adoption/reviewed-manifest'

/** Canonical wire union. Rendering and source-art admission belong to their separate owners. */
export type AvatarConfig = WireAvatarState['config']
export type LayeredAvatarConfig = Extract<AvatarConfig, { contract_version: '1.0.0' }>
export type WholeCharacterConfig = Extract<AvatarConfig, { contract_version: '1.1.0' }>
export type WholeCharacterPresetId = WholeCharacterConfig['preset_id']
export type AvatarState = Readonly<{ revision: number; config: AvatarConfig }>
export type ManifestPin = Readonly<{ manifest_id: string; manifest_version: number; content_hash: string }>
export type AvatarManifest = LayeredAvatarManifest | WholeCharacterManifest | ReviewedMotionManifest
export type { LayeredAvatarManifest, WholeCharacterManifest }
export { AvatarError }

/** Strict public DTO admission when no resource manifest accompanies a profile read. */
export function parseConfig(value: unknown): AvatarConfig {
  try {
    if (record(value).contract_version === '2.0.0') return decodeReviewedMotionConfig(value)
    ensure(schemaValid(value, 'config'))
    return immutable(structuredClone(value as AvatarConfig))
  } catch (error) { if (error instanceof AvatarError) throw error; throw new AvatarError('invalid') }
}
export function decodeAvatarState(value: unknown): AvatarState {
  try {
    const state = record(value)
    ensure(Object.keys(state).length === 2 && Object.hasOwn(state, 'revision') && Object.hasOwn(state, 'config'))
    ensure(typeof state.revision === 'number' && Number.isSafeInteger(state.revision) && state.revision >= 1)
    return immutable({ revision: state.revision, config: parseConfig(state.config) })
  } catch (error) { if (error instanceof AvatarError) throw error; throw new AvatarError('invalid') }
}

/** Validate against an already hash-verified manifest, preserving the admitted branch and IDs. */
export function validateConfig(value: unknown, manifest: AvatarManifest): AvatarConfig {
  const config = parseConfig(value)
  if (config.contract_version !== manifest.contract_version
    || config.manifest_id !== manifest.manifest_id || config.manifest_version !== manifest.manifest_version) throw new AvatarError('version')
  if (config.contract_version === '1.0.0') {
    ensure(manifest.contract_version === '1.0.0')
    return validateLayeredConfig(config, manifest)
  }
  if (config.contract_version === '2.0.0') {
    ensure(manifest.contract_version === '2.0.0')
    if (config.representation === 'motion_preset') {
      ensure(manifest.presets.some(preset => preset.preset_id === config.preset_id && canonicalJson(preset.config) === canonicalJson(config)))
    } else {
      for (const slot of ['body', 'eyes', 'hair', 'outfit', 'accessory'] as const) {
        const id = config.item_ids[slot]
        if ((slot === 'outfit' || slot === 'accessory') && id === null) continue
        ensure(manifest.items.some(item => item.item_id === id && item.slot === slot))
      }
    }
    return config satisfies ReviewedMotionConfig
  }
  ensure(manifest.contract_version === '1.1.0')
  ensure(manifest.presets.some(preset => preset.preset_id === config.preset_id && canonicalJson(preset.config) === canonicalJson(config)))
  return immutable(structuredClone(config))
}

/** C06 text admission rejects duplicate keys and lexical floats before JSON.parse loses them. */
export function decodeConfig(text: string, manifest: AvatarManifest): AvatarConfig {
  try { return validateConfig(strictJson(text), manifest) }
  catch (error) { if (error instanceof AvatarError) throw error; throw new AvatarError('invalid') }
}

/** Local canonical schemas only. No source/texture fetch, fallback, conversion or mutation. */
export async function validateManifest(text: string, pin: ManifestPin): Promise<AvatarManifest> {
  try {
    const value = record(strictJson(text))
    if (value.contract_version === '2.0.0') {
      if (value.manifest_id !== pin.manifest_id || value.manifest_version !== pin.manifest_version) throw new AvatarError('version')
      return (await decodeReviewedMotionManifest(value, pin)).manifest
    }
    ensure(schemaValid(value, 'manifest'))
    const manifest = value as AvatarManifest
    if (manifest.manifest_id !== pin.manifest_id || manifest.manifest_version !== pin.manifest_version) throw new AvatarError('version')
    if (manifest.contract_version === '1.0.0') return await validateLayeredManifest(text, pin)
    return await validateWholeCharacterManifest(manifest, pin)
  } catch (error) { if (error instanceof AvatarError) throw error; throw new AvatarError('invalid') }
}
