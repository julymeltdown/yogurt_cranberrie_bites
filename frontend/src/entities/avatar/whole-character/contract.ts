import configSchema from '../../../../../contracts/assets/v1/avatar-config.schema.json'
import manifestSchema from '../../../../../contracts/assets/v1/avatar-manifest.schema.json'
import commonSchema from '../../../../../contracts/assets/v1/common.schema.json'
import { canonicalJson, contentHash, strictJson } from '../../../world/map/json'

export class WholeCharacterError extends Error {
  constructor(readonly reason: 'invalid' | 'version' | 'unavailable' | 'cancelled' | 'capacity') { super(`Whole character ${reason}.`); this.name = 'WholeCharacterError' }
}
export function ensure(value: unknown): asserts value { if (!value) throw new WholeCharacterError('invalid') }
export const PRESET_IDS = Object.freeze(['photo01_front_01', 'photo01_front_02', 'photo01_front_03', 'photo01_front_04', 'photo01_front_05', 'photo01_front_06'] as const)
export type WholeCharacterPresetId = typeof PRESET_IDS[number]
export interface WholeCharacterConfig {
  readonly contract_version: '1.1.0'; readonly kind: 'avatar_config'; readonly manifest_id: 'mcr_selected_static_20261103'
  readonly manifest_version: 1; readonly representation: 'whole_character'; readonly preset_id: WholeCharacterPresetId
}
export interface WholeCharacterFrame { readonly x: number; readonly y: number; readonly width_px: 64; readonly height_px: 64 }
export interface WholeCharacterManifest {
  readonly contract_version: '1.1.0'; readonly kind: 'avatar_manifest'; readonly manifest_id: WholeCharacterConfig['manifest_id']
  readonly manifest_version: 1; readonly representation: 'whole_character'; readonly content_hash: string
  readonly source: { readonly source_pack_id: string; readonly source_pack_revision: number; readonly source_id: string; readonly source_sha256: string; readonly release_path: string; readonly release_sha256: string; readonly source_commit: string }
  readonly atlas: { readonly atlas_id: string; readonly cell: { readonly width_px: 64; readonly height_px: 64 }; readonly anchor: { readonly x: 32; readonly y: 60 }; readonly texture: { readonly image_state: 'PROVIDED'; readonly path: string; readonly sha256: string; readonly width_px: 384; readonly height_px: 64 } }
  readonly capabilities: { readonly poses: readonly ['front_idle']; readonly layer_customization: false; readonly skin_tone_customization: false }
  readonly presets: readonly { readonly preset_id: WholeCharacterPresetId; readonly config: WholeCharacterConfig; readonly art_id: string; readonly frame: WholeCharacterFrame }[]
  readonly default_preset_id: WholeCharacterPresetId
}
export interface WholeCharacterPin { readonly manifest_id: string; readonly manifest_version: number; readonly content_hash: string }
export const REVIEWED = Object.freeze({
  manifestId: 'mcr_selected_static_20261103', manifestHash: 'sha256:0627c7d178b22535a561e1270db6d8bf9e2383ba84d03a63962e48e565c59b2b',
  descriptor: Object.freeze({ path: 'world/avatars/derived-v1/static-release.json', sha256: 'sha256:cc53c6852742e9cbb9858a2d0961411e1a9e131d1e30c0f2b73b265e03f91677', bytes: 1898 }),
  texture: Object.freeze({ path: 'world/avatars/derived-v1/photo01-front-idle-64-e124c88815b22220.png', sha256: 'sha256:e124c88815b22220df2f2dc59e2f26b3ceb845e57b77674fd8e55383116d7efe', bytes: 36030, width: 384, height: 64 }),
  detail: Object.freeze({ path: 'world/avatars/derived-v1/photo01-front-idle-256-6258b9f9ee51aed7.png', sha256: 'sha256:6258b9f9ee51aed7a04b0690dfb6fc20592ed8347d72fcaaf8037b4c19e7b099', bytes: 368812, width: 1536, height: 256 }),
  sourceHash: '67448360482a0dd68172d6089378a002190142bb8bd804818c89a86370154311', sourceCommit: '7e07f7328f079991a9aedb7ad83a4b682decb543',
})
const record = (value: unknown): Record<string, unknown> => { ensure(value !== null && typeof value === 'object' && !Array.isArray(value)); return value as Record<string, unknown> }
const docs: Record<string, unknown> = { [configSchema.$id]: configSchema, [manifestSchema.$id]: manifestSchema, [commonSchema.$id]: commonSchema }
const keywords = new Set(['$id', '$schema', '$defs', 'title', 'description', '$ref', 'type', 'const', 'enum', 'properties', 'required', 'additionalProperties', 'items', 'minItems', 'maxItems', 'uniqueItems', 'minLength', 'maxLength', 'minimum', 'maximum', 'pattern', 'format', 'anyOf', 'oneOf', 'allOf', 'if', 'then', 'else'])
/** Evaluate the actual owner-published whole-character definition, not a parallel wire schema. */
export function wholeSchemaValid(value: unknown, kind: 'config' | 'manifest'): boolean {
  let visits = 0
  function valid(v: unknown, rule: unknown, root: unknown, depth = 0): boolean {
    ensure(++visits <= 20000 && depth <= 24)
    const s = record(rule); ensure(Object.keys(s).every(key => keywords.has(key)))
    if (s.$ref) {
      ensure(typeof s.$ref === 'string'); const [uri, fragment] = s.$ref.split('#'), doc = uri ? docs[uri] : root
      ensure(doc && fragment && /^\/\$defs\/[A-Za-z0-9_]+$/.test(fragment))
      if (!valid(v, record(record(doc).$defs)[fragment.split('/')[2]!], doc, depth + 1)) return false
    }
    if (s.type && !(s.type === 'object' ? !!v && typeof v === 'object' && !Array.isArray(v) : s.type === 'array' ? Array.isArray(v)
      : s.type === 'integer' ? typeof v === 'number' && Number.isSafeInteger(v) : s.type === 'null' ? v === null : typeof v === s.type)) return false
    if ('const' in s && canonicalJson(v) !== canonicalJson(s.const)) return false
    if (Array.isArray(s.enum) && !s.enum.some(x => canonicalJson(v) === canonicalJson(x))) return false
    if (typeof v === 'number' && (typeof s.minimum === 'number' && v < s.minimum || typeof s.maximum === 'number' && v > s.maximum)) return false
    if (typeof v === 'string') {
      if (Array.from(v).length < Number(s.minLength ?? 0) || Array.from(v).length > Number(s.maxLength ?? 32768)) return false
      if (s.pattern && !new RegExp(String(s.pattern), 'u').test(v)) return false
      if (s.format) { ensure(s.format === 'date-time'); if (!/^\d{4}-\d\d-\d\dT\d\d:\d\d:\d\d(?:\.\d+)?Z$/.test(v) || !Number.isFinite(Date.parse(v))) return false }
    }
    if (v && typeof v === 'object' && !Array.isArray(v)) {
      const object = record(v), props = s.properties ? record(s.properties) : {}; ensure(Object.keys(object).length <= 64)
      if (Array.isArray(s.required) && !s.required.every(k => typeof k === 'string' && Object.hasOwn(object, k))) return false
      if (s.additionalProperties === false && Object.keys(object).some(k => !Object.hasOwn(props, k))) return false
      if (!Object.entries(props).every(([key, child]) => !Object.hasOwn(object, key) || valid(object[key], child, root, depth + 1))) return false
    }
    if (Array.isArray(v)) {
      if (v.length < Number(s.minItems ?? 0) || v.length > Number(s.maxItems ?? 256)) return false
      if (s.uniqueItems && new Set(v.map(x => canonicalJson(x))).size !== v.length) return false
      if (s.items && !v.every(x => valid(x, s.items, root, depth + 1))) return false
    }
    if (Array.isArray(s.anyOf) && !s.anyOf.some(child => valid(v, child, root, depth + 1))) return false
    if (Array.isArray(s.oneOf) && s.oneOf.filter(child => valid(v, child, root, depth + 1)).length !== 1) return false
    if (Array.isArray(s.allOf) && !s.allOf.every(child => valid(v, child, root, depth + 1))) return false
    if (s.if) { const branch = valid(v, s.if, root, depth + 1) ? s.then : s.else; if (branch && !valid(v, branch, root, depth + 1)) return false }
    return true
  }
  try { const doc = kind === 'config' ? configSchema : manifestSchema; return valid(value, record(record(doc).$defs)[kind === 'config' ? 'WholeCharacterAvatarConfig' : 'WholeCharacterAvatarManifest'], doc) } catch { return false }
}
export function freeze<T>(value: T): T { if (value && typeof value === 'object') { Object.values(value).forEach(freeze); Object.freeze(value) } return value }
export function validateWholeCharacterConfig(input: unknown): WholeCharacterConfig {
  ensure(wholeSchemaValid(input, 'config'))
  const config = input as WholeCharacterConfig
  ensure(config.manifest_id === REVIEWED.manifestId && config.manifest_version === 1 && PRESET_IDS.includes(config.preset_id))
  return freeze({ ...config })
}
export async function validateWholeCharacterManifest(input: unknown, pin: WholeCharacterPin): Promise<WholeCharacterManifest> {
  if (typeof input === 'string') { ensure(new TextEncoder().encode(input).length <= 32768); input = strictJson(input) }
  ensure(wholeSchemaValid(input, 'manifest'))
  const text = canonicalJson(input); ensure(new TextEncoder().encode(text).length <= 32768)
  const m = JSON.parse(text) as WholeCharacterManifest
  if (pin.manifest_id !== REVIEWED.manifestId || pin.manifest_version !== 1 || pin.content_hash !== REVIEWED.manifestHash) throw new WholeCharacterError('version')
  ensure(m.manifest_id === pin.manifest_id && m.manifest_version === pin.manifest_version && m.content_hash === pin.content_hash && await contentHash(m as unknown as Record<string, unknown>) === pin.content_hash)
  ensure(m.source.source_id === 'photo_01' && m.source.source_pack_id === 'mcr_user_characters_20261103' && m.source.source_pack_revision === 3
    && m.source.source_sha256.replace(/^sha256:/, '') === REVIEWED.sourceHash && m.source.source_commit === REVIEWED.sourceCommit
    && m.source.release_path === REVIEWED.descriptor.path && m.source.release_sha256 === REVIEWED.descriptor.sha256
    && m.atlas.texture.path === REVIEWED.texture.path && m.atlas.texture.sha256 === REVIEWED.texture.sha256)
  ensure(m.presets.length === 6 && new Set(m.presets.map(p => p.preset_id)).size === 6)
  m.presets.forEach((p, i) => {
    ensure(p.preset_id === PRESET_IDS[i] && p.art_id === `photo01-front-0${i + 1}` && p.frame.x === i * 64 && p.frame.y === 0 && p.frame.width_px === 64 && p.frame.height_px === 64)
    const config = validateWholeCharacterConfig(p.config); ensure(config.preset_id === p.preset_id)
  })
  return freeze(m)
}

export function sameWholeCharacterConfig(left: unknown, right: unknown): boolean {
  try { return validateWholeCharacterConfig(left).preset_id === validateWholeCharacterConfig(right).preset_id } catch { return false }
}
export interface WholeCharacterChoice { readonly id: WholeCharacterPresetId; readonly label: string; readonly config: WholeCharacterConfig }
