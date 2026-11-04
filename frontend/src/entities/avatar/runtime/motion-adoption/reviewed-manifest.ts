import { canonicalJson, contentHash, record, strictJson } from '../../../../world/map/json'
import { MOTION_ART, assertMotionArt, requireMotion, validateMotionSelection, type MotionArt, type MotionDirection, type MotionLayer } from './art'
import { MOTION_V2_CANDIDATE } from './v2-candidate'
import type { MotionAuthority, MotionAuthorityOperations } from './runtime'
import { decodeReviewedMotionConfig, reviewedMotionConfig, reviewedMotionSelection, REVIEWED_MOTION_FAMILY, type ReviewedMotionConfig } from './reviewed-config'

/** Structural and visual-review pins. Backend admission gates remain independently required. */
export const REVIEWED_MOTION = Object.freeze({
  commit: '0dd9f443945e6d6232728280d03e83933ac0eef2', manifestId: REVIEWED_MOTION_FAMILY, manifestVersion: 2,
  contentHash: 'sha256:7f2891c81ffa1d5850107118898a7ef9377b222ab8639b3f2c9daf7908cf9018',
  manifest: Object.freeze({ path: 'backend/releases/selected-character-motion-v2/reviewed/manifest.json', bytes: 97407,
    sha256: 'sha256:1342f2b640a8135c2f128713fe1f59adc0b6da8b925a0ac84808b2d8d1ec884d' }),
  review: Object.freeze({ path: 'docs/mcr/contracts/avatar-motion-v2/review/visual-review.json', bytes: 8109,
    sha256: 'sha256:27dd1f7f1d31b792f4da415cf6daf6eafc97e91023443a27ac70480f75ce5956' }),
})
interface Resource { readonly path: string; readonly sha256: string; readonly bytes: number }
interface DirectionFrames { readonly direction: MotionDirection; readonly frames: readonly number[]; readonly hand_frames?: readonly number[] }
export interface ReviewedMotionManifest {
  readonly contract_version: '2.0.0'; readonly kind: 'avatar_manifest'; readonly manifest_id: typeof REVIEWED_MOTION_FAMILY; readonly manifest_version: 2
  readonly representation: 'reviewed_selected_source_motion'; readonly production_eligible: true; readonly qualification: 'VISUALLY_REVIEWED'; readonly fallback: 'NONE'
  readonly content_hash: string; readonly default_preset_id: string; readonly review: Resource; readonly candidate_manifest: Resource
  readonly source: { readonly pack_id: string; readonly revision: number; readonly descriptor: Resource }
  readonly atlas: Resource & { readonly width: number; readonly height: number }
  readonly presets: readonly { readonly preset_id: string; readonly local_key: string; readonly config: ReviewedMotionConfig; readonly directions: readonly DirectionFrames[] }[]
  readonly items: readonly { readonly item_id: string; readonly slot: MotionLayer; readonly local_key: string; readonly directions: readonly DirectionFrames[];
    readonly skin_palette?: { readonly colors: readonly (readonly number[])[] } }[]
}
export interface ReviewedMotionPin { readonly manifest_id: string; readonly manifest_version: number; readonly content_hash: string }
const verified = Symbol('verified staged reviewed motion manifest')
export interface ReviewedMotionStage { readonly manifest: ReviewedMotionManifest; readonly qualification: 'STAGED_NOT_ADMITTED'; readonly [verified]: true }
function freeze<T>(value: T): T { if (value && typeof value === 'object') { Object.values(value).forEach(freeze); Object.freeze(value) } return value }

/** Verify the complete canonical document, not its untrusted production_eligible flag. */
export async function decodeReviewedMotionManifest(input: unknown, pin: ReviewedMotionPin): Promise<ReviewedMotionStage> {
  requireMotion(pin.manifest_id === REVIEWED_MOTION_FAMILY && pin.manifest_version === 2 && pin.content_hash === REVIEWED_MOTION.contentHash)
  if (typeof input === 'string') {
    requireMotion(new TextEncoder().encode(input).byteLength <= 131072); input = strictJson(input)
  }
  const text = canonicalJson(input); requireMotion(new TextEncoder().encode(text).byteLength <= 131072)
  const value = record(strictJson(text))
  requireMotion(value.content_hash === pin.content_hash && await contentHash(value) === pin.content_hash)
  // Full-document hash above binds the closed const schema, including every original, UV, palette and review pin.
  const manifest = value as unknown as ReviewedMotionManifest
  requireMotion(manifest.contract_version === '2.0.0' && manifest.kind === 'avatar_manifest'
    && manifest.manifest_id === pin.manifest_id && manifest.manifest_version === pin.manifest_version
    && manifest.representation === 'reviewed_selected_source_motion' && manifest.production_eligible === true
    && manifest.qualification === 'VISUALLY_REVIEWED' && manifest.fallback === 'NONE'
    && manifest.review.path === REVIEWED_MOTION.review.path && `sha256:${manifest.review.sha256}` === REVIEWED_MOTION.review.sha256
    && manifest.review.bytes === REVIEWED_MOTION.review.bytes
    && manifest.candidate_manifest.path === MOTION_V2_CANDIDATE.path && `sha256:${manifest.candidate_manifest.sha256}` === MOTION_V2_CANDIDATE.sha256
    && manifest.candidate_manifest.bytes === MOTION_V2_CANDIDATE.bytes
    && manifest.source.descriptor.path === `frontend/public/${MOTION_ART.descriptor.path}` && `sha256:${manifest.source.descriptor.sha256}` === MOTION_ART.descriptor.sha256
    && manifest.atlas.path === `frontend/public/${MOTION_ART.atlas.path}` && `sha256:${manifest.atlas.sha256}` === MOTION_ART.atlas.sha256)
  return freeze({ manifest, qualification: 'STAGED_NOT_ADMITTED' as const, [verified]: true as const })
}

/** Canonical staged configs may be exercised by fixtures; this is not a production registry authority. */
export function reviewedMotionAuthorityOperations(stage: ReviewedMotionStage): MotionAuthorityOperations<ReviewedMotionConfig> {
  requireMotion(stage?.[verified] === true && stage.qualification === 'STAGED_NOT_ADMITTED')
  return Object.freeze({ manifestId: REVIEWED_MOTION.manifestId, manifestVersion: 2, manifestHash: REVIEWED_MOTION.contentHash,
    decodeConfig: decodeReviewedMotionConfig, selection: reviewedMotionSelection,
    presets(art: MotionArt) {
      assertMotionArt(art)
      return Object.freeze(stage.manifest.presets.map((preset, index) => {
        const selection = validateMotionSelection({ mode: 'preset', preset: preset.local_key }, art), config = decodeReviewedMotionConfig(preset.config)
        requireMotion(config.representation === 'motion_preset' && config.preset_id === preset.preset_id
          && canonicalJson(config) === canonicalJson(reviewedMotionConfig(selection)))
        return Object.freeze({ id: preset.preset_id, label: `Preset ${index + 1}`, config })
      }))
    },
    replace(config: ReviewedMotionConfig, selection: Parameters<typeof reviewedMotionConfig>[0]) {
      decodeReviewedMotionConfig(config); return reviewedMotionConfig(selection)
    },
  })
}

/** Operations alone do not grant production authority. The fixture remains TEST_ONLY. */
export function createReviewedMotionFixtureAuthority(stage: ReviewedMotionStage): MotionAuthority<ReviewedMotionConfig> {
  return Object.freeze({ ...reviewedMotionAuthorityOperations(stage), qualification: 'TEST_ONLY' as const })
}
