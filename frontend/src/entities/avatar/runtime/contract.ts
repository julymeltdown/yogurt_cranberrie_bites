import type fixture from '../../../../../contracts/fixtures/assets/avatar-manifest.json'
import type { AvatarConfig as WireAvatarConfig } from '../../../world/network/WorldTransport'
import { immutable } from '../../../world/entities/replicated'
import { contentHash, strictJson } from '../../../world/map/json'
import { schemaValid } from './schema'

/** The layered compositor must never admit a whole-character configuration. */
export type AvatarConfig = Extract<WireAvatarConfig, { contract_version: '1.0.0' }>
export type Layer = 'body' | 'eyes' | 'hair' | 'outfit' | 'accessory'
export type Direction = 'down' | 'left' | 'right' | 'up'
export type Pose = Readonly<{ direction: Direction; animation: 'idle' | 'walk'; frame: number }>
type Texture = { image_state: 'PROVIDED' | 'NOT_PROVIDED'; path: string | null; sha256: string | null; width_px: number; height_px: number }
export type AvatarManifest = Omit<typeof fixture, 'atlas' | 'provenance' | 'presets' | 'contract_version' | 'kind'> & {
  contract_version: '1.0.0'; kind: 'avatar_manifest'
  provenance: Omit<typeof fixture.provenance, 'source_uri' | 'reviewed_by' | 'reviewed_at'> & { source_uri: string | null; reviewed_by: string | null; reviewed_at: string | null }
  atlas: Omit<typeof fixture.atlas, 'texture'> & { texture: Texture }
  presets: { preset_id: string; config: AvatarConfig }[]
}
export class AvatarError extends Error {
  constructor(readonly reason: 'invalid' | 'version' | 'unavailable' | 'cancelled' | 'capacity') { super(`Avatar ${reason}`); this.name = 'AvatarError' }
}
export function ensure(value: unknown): asserts value { if (!value) throw new AvatarError('invalid') }
export function validateConfig(value: unknown, manifest: AvatarManifest): AvatarConfig {
  ensure(schemaValid(value, 'config'))
  if ((value as WireAvatarConfig).contract_version !== '1.0.0') throw new AvatarError('version')
  const config = value as AvatarConfig
  if (config.manifest_id !== manifest.manifest_id || config.manifest_version !== manifest.manifest_version) throw new AvatarError('version')
  ensure(manifest.skin_tones.some(t => t.skin_tone_id === config.skin_tone_id))
  for (const layer of manifest.layer_order as Layer[]) {
    const id = config.item_ids[layer]
    if (layer === 'accessory' && id === null) continue
    ensure(manifest.atlas.items.some(item => item.item_id === id && item.layer_id === layer))
  }
  return immutable(structuredClone(config))
}
export async function validateManifest(text: string, pin: { manifest_id: string; manifest_version: number; content_hash: string }): Promise<AvatarManifest> {
  const value = strictJson(text)
  ensure(schemaValid(value, 'manifest'))
  if ((value as { contract_version: string }).contract_version !== '1.0.0') throw new AvatarError('version')
  const manifest = value as AvatarManifest, { atlas } = manifest, texture = atlas.texture
  if (manifest.manifest_id !== pin.manifest_id || manifest.manifest_version !== pin.manifest_version) throw new AvatarError('version')
  ensure(manifest.content_hash === pin.content_hash && await contentHash(manifest) === pin.content_hash)
  for (const entries of [atlas.items.map(i => i.item_id), manifest.skin_tones.map(t => t.skin_tone_id), manifest.presets.map(p => p.preset_id)]) {
    ensure(entries.length <= 256 && new Set(entries).size === entries.length)
  }
  ensure(manifest.layer_order.every(layer => atlas.items.some(item => item.layer_id === layer)))
  ensure(texture.width_px <= 2048 && texture.height_px <= 2048 && texture.width_px % 64 === 0 && texture.height_px % 64 === 0)
  const occupied = new Set<string>()
  for (const item of atlas.items) {
    const r = item.region
    ensure(r.x % 64 === 0 && r.y % 64 === 0 && r.x + r.width_px <= texture.width_px && r.y + r.height_px <= texture.height_px)
    for (let y = r.y; y < r.y + r.height_px; y += 64) for (let x = r.x; x < r.x + r.width_px; x += 64) {
      const key = `${x},${y}`; ensure(!occupied.has(key)); occupied.add(key)
    }
  }
  ensure(manifest.presets.some(p => p.preset_id === manifest.fallback_preset_id))
  for (const preset of manifest.presets) validateConfig(preset.config, manifest)
  return immutable(manifest)
}
export function frameIndex(pose: Pose): number {
  const row = ['down', 'left', 'right', 'up'].indexOf(pose.direction)
  ensure(row >= 0 && ['idle', 'walk'].includes(pose.animation) && Number.isInteger(pose.frame)
    && pose.frame >= 0 && pose.frame < (pose.animation === 'idle' ? 1 : 4))
  return row * 5 + (pose.animation === 'idle' ? 0 : pose.frame + 1)
}
export function sourceRect(item: AvatarManifest['atlas']['items'][number], pose: Pose) {
  const frame = frameIndex(pose)
  return { x: item.region.x + frame % 5 * 64, y: item.region.y + Math.floor(frame / 5) * 64, width: 64, height: 64,
    key: `${item.item_id}/${pose.direction}/${pose.animation}/${pose.frame}` }
}
