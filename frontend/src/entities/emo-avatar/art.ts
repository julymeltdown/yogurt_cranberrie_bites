import { sha256 } from '../../world/map/json'
import { createAssetResources, decodeImage, pngDimensions, type AssetResources } from '../../world/map/resources'

/**
 * WP-08 T-704: the pinned motion artwork for the profile avatar.
 *
 * The path and hash contract is preserved byte for byte from the concert
 * editor: the same descriptor path, the same atlas path, the same sha256 and
 * byte counts. The files are not renamed or copied; both live under
 * `frontend/public/world/avatars/derived-v1/motion-candidates-v1/` and are read
 * through the same `createAssetResources` allowlist.
 *
 * Artwork pins are not a public inventory manifest or a production admission.
 */
export const MOTION_ART = Object.freeze({
  descriptor: Object.freeze({ path: 'world/avatars/derived-v1/motion-candidates-v1/combined-release-candidate.json',
    sha256: 'sha256:6cf3aad25ceba681f4c0306d1139109ee6c07508de617ecf1c0b08b1d43fb950', bytes: 42925, kind: 'json' as const }),
  atlas: Object.freeze({ path: 'world/avatars/derived-v1/motion-candidates-v1/avatar-candidates-64-1cd8f05af8d76d58.png',
    sha256: 'sha256:1cd8f05af8d76d58243e2f6e143961953b4d4291fdd553df8c8d33aabd700dbd', bytes: 470010, kind: 'png' as const }),
})

export type MotionLayer = 'body' | 'eyes' | 'hair' | 'outfit' | 'accessory'
export const MOTION_LAYERS: readonly MotionLayer[] = Object.freeze(['body', 'eyes', 'hair', 'outfit', 'accessory'])
export type MotionDirection = 'down' | 'left' | 'right' | 'up'
export type MotionSelection = Readonly<{ mode: 'preset'; preset: string }> | Readonly<{
  mode: 'wardrobe'; body: string; eyes: string; hair: string; outfit: string | null; accessory: string | null
}>
export interface MotionItem {
  readonly key: string; readonly kind: MotionLayer
  readonly skinPalette?: { readonly colors: readonly (readonly number[])[] }
  readonly directions: readonly { readonly direction: number; readonly frames: readonly number[]; readonly handFrames?: readonly number[] }[]
}
interface Descriptor {
  readonly kind: 'SELECTED_CHARACTER_COMBINED_ART_CANDIDATE'; readonly productionEligible: false
  readonly sourcePack: { readonly id: string; readonly revision: number }
  readonly atlas: { readonly file: string; readonly width: number; readonly height: number; readonly sha256: string; readonly bytes: number }
  readonly physicalFrames: number
  readonly presets: { readonly frames: readonly { readonly preset: string; readonly direction: string; readonly phase: number; readonly atlasIndex: number }[] }
  readonly wardrobe: { readonly items: readonly MotionItem[]; readonly composition: { readonly outfitBodyClipY: number } }
}
const verified = Symbol('verified motion artwork')
export interface MotionArt { readonly descriptor: Descriptor; readonly [verified]: true }

/** Fail closed. There is no fallback artwork path: a missing or unverified art
 * owner throws, and the caller renders an explicit unavailable state. */
export function requireMotion(value: unknown): asserts value { if (!value) throw new Error('Motion artwork unavailable') }
function freeze<T>(value: T): T { if (value && typeof value === 'object') { Object.values(value).forEach(freeze); Object.freeze(value) } return value }

export async function validateMotionArt(bytes: Uint8Array<ArrayBuffer>): Promise<MotionArt> {
  requireMotion(bytes.byteLength === MOTION_ART.descriptor.bytes && await sha256(bytes) === MOTION_ART.descriptor.sha256)
  const descriptor = JSON.parse(new TextDecoder('utf-8', { fatal: true }).decode(bytes)) as Descriptor
  requireMotion(descriptor.kind === 'SELECTED_CHARACTER_COMBINED_ART_CANDIDATE' && descriptor.productionEligible === false
    && descriptor.sourcePack.id === 'mcr_user_characters_20261103' && descriptor.sourcePack.revision === 3
    && descriptor.atlas.width === 1280 && descriptor.atlas.height === 1856 && descriptor.physicalFrames === 576
    && `sha256:${descriptor.atlas.sha256}` === MOTION_ART.atlas.sha256 && descriptor.atlas.bytes === MOTION_ART.atlas.bytes
    && descriptor.presets.frames.length === 120 && descriptor.wardrobe.composition.outfitBodyClipY === 42)
  return freeze({ descriptor, [verified]: true as const })
}
export function assertMotionArt(art: MotionArt) { requireMotion(art?.[verified] === true) }

/** Selected-source local keys only. A public decoder must map authorized inventory to these keys. */
export function validateMotionSelection(value: unknown, art: MotionArt): MotionSelection {
  assertMotionArt(art)
  requireMotion(value && typeof value === 'object' && !Array.isArray(value))
  const input = value as Record<string, unknown>, keys = Object.keys(input).sort().join(',')
  if (input.mode === 'preset') {
    requireMotion(keys === 'mode,preset' && typeof input.preset === 'string' && art.descriptor.presets.frames.some(frame => frame.preset === input.preset))
    return Object.freeze({ mode: 'preset', preset: input.preset })
  }
  requireMotion(input.mode === 'wardrobe' && keys === 'accessory,body,eyes,hair,mode,outfit')
  for (const layer of MOTION_LAYERS) {
    const key = input[layer]
    if ((layer === 'outfit' || layer === 'accessory') && key === null) continue
    requireMotion(typeof key === 'string' && art.descriptor.wardrobe.items.some(item => item.kind === layer && item.key === key))
  }
  return Object.freeze({ mode: 'wardrobe', body: input.body as string, eyes: input.eyes as string, hair: input.hair as string,
    outfit: input.outfit as string | null, accessory: input.accessory as string | null })
}

export function motionFrame(direction: MotionDirection, moving: boolean, time: number, cadence = 140): number {
  const row = ['down', 'left', 'right', 'up'].indexOf(direction)
  requireMotion(row >= 0 && typeof moving === 'boolean' && Number.isFinite(time) && time >= 0 && Number.isFinite(cadence) && cadence >= 50 && cadence <= 1000)
  return row * 5 + (moving ? 1 + Math.floor(time / cadence) % 4 : 0)
}

export async function loadMotionArtwork(options: { origin: string; signal: AbortSignal; resources?: AssetResources; fetch?: typeof fetch }) {
  const resources = options.resources ?? createAssetResources({ origin: options.origin,
    allow: [MOTION_ART.descriptor, MOTION_ART.atlas], ...(options.fetch ? { fetch: options.fetch } : {}) })
  const art = await validateMotionArt(await resources.read(MOTION_ART.descriptor, options.signal))
  const bytes = await resources.read(MOTION_ART.atlas, options.signal)
  requireMotion(bytes.byteLength === MOTION_ART.atlas.bytes && await sha256(bytes) === MOTION_ART.atlas.sha256)
  pngDimensions(bytes, 1280, 1856)
  const decoded = await decodeImage(bytes, options.signal)
  return { art, ...decoded }
}
