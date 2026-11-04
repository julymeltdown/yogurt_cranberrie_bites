import { sha256, strictJson } from '../../../../world/map/json'
import { MOTION_ART, assertMotionArt, requireMotion, validateMotionSelection, type MotionArt, type MotionDirection, type MotionLayer } from './art'
import type { MotionAuthority } from './runtime'
import { decodeMotionV2Config, motionV2Config, motionV2Selection, MOTION_V2_FAMILY, type MotionV2Config } from './v2'

/** Producer ae41db9db2493c259c72ef461844a3ff1019a1f6. Immutable candidate, never admission authority. */
export const MOTION_V2_CANDIDATE = Object.freeze({
  commit: 'ae41db9db2493c259c72ef461844a3ff1019a1f6',
  path: 'backend/releases/selected-character-motion-v2/manifest.json', bytes: 95128,
  sha256: 'sha256:9a555f726a9e421f14ed876117be87bb1bb8fa128ac401ab38f8927760776fd6',
  configSchema: Object.freeze({ path: 'contracts/assets/v2/avatar-config.schema.json', bytes: 3543,
    sha256: 'sha256:3cbfd3185113d7f4913de89b0e04d67046dbab64b3f90c0b5f14a261afc6a78b' }),
  manifestSchema: Object.freeze({ path: 'contracts/assets/v2/avatar-manifest.schema.json', bytes: 102672,
    sha256: 'sha256:8fc206f00d263ed9465cad932768075a1a0ca61d927e98ccc2aa8e9e7ef0d54f' }),
})
interface DirectionFrames { readonly direction: MotionDirection; readonly frames: readonly number[]; readonly hand_frames?: readonly number[] }
interface CandidateManifest {
  readonly contract_version: '2.0.0'; readonly kind: 'avatar_manifest'; readonly manifest_id: typeof MOTION_V2_FAMILY
  readonly manifest_version: 1; readonly representation: 'selected_source_motion'; readonly production_eligible: false
  readonly qualification: 'CANDIDATE_ONLY'; readonly fallback: 'NONE'
  readonly source: { readonly pack_id: string; readonly revision: number; readonly descriptor: Resource }
  readonly atlas: Resource & { readonly width: number; readonly height: number }
  readonly presets: readonly { readonly preset_id: string; readonly local_key: string; readonly directions: readonly DirectionFrames[] }[]
  readonly items: readonly { readonly item_id: string; readonly slot: MotionLayer; readonly local_key: string; readonly directions: readonly DirectionFrames[];
    readonly skin_palette?: { readonly colors: readonly (readonly number[])[] } }[]
}
interface Resource { readonly path: string; readonly sha256: string; readonly bytes: number }
const verified = Symbol('verified motion v2 candidate')
export interface MotionV2Candidate { readonly manifest: CandidateManifest; readonly [verified]: true }
function freeze<T>(value: T): T { if (value && typeof value === 'object') { Object.values(value).forEach(freeze); Object.freeze(value) } return value }

/** Exact release bytes are the producer's const schema. Modified/reformatted releases need a new pin. */
export async function decodeMotionV2Candidate(bytes: Uint8Array<ArrayBuffer>): Promise<MotionV2Candidate> {
  requireMotion(bytes.byteLength === MOTION_V2_CANDIDATE.bytes && await sha256(bytes) === MOTION_V2_CANDIDATE.sha256)
  const manifest = strictJson(new TextDecoder('utf-8', { fatal: true }).decode(bytes)) as unknown as CandidateManifest
  requireMotion(manifest.contract_version === '2.0.0' && manifest.manifest_id === MOTION_V2_FAMILY && manifest.manifest_version === 1
    && manifest.production_eligible === false && manifest.qualification === 'CANDIDATE_ONLY' && manifest.fallback === 'NONE'
    && manifest.source.pack_id === 'mcr_user_characters_20261103' && manifest.source.revision === 3
    && manifest.source.descriptor.path === `frontend/public/${MOTION_ART.descriptor.path}`
    && `sha256:${manifest.source.descriptor.sha256}` === MOTION_ART.descriptor.sha256
    && manifest.source.descriptor.bytes === MOTION_ART.descriptor.bytes
    && manifest.atlas.path === `frontend/public/${MOTION_ART.atlas.path}` && `sha256:${manifest.atlas.sha256}` === MOTION_ART.atlas.sha256
    && manifest.atlas.bytes === MOTION_ART.atlas.bytes && manifest.atlas.width === 1280 && manifest.atlas.height === 1856)
  return freeze({ manifest, [verified]: true as const })
}

/** Candidate proof may exercise canonical wire IDs, but cannot enable a production registry. */
export function createMotionV2CandidateAuthority(candidate: MotionV2Candidate): MotionAuthority<MotionV2Config> {
  requireMotion(candidate?.[verified] === true)
  const manifest = candidate.manifest
  return Object.freeze({ qualification: 'TEST_ONLY' as const,
    manifestId: manifest.manifest_id, manifestVersion: manifest.manifest_version, manifestHash: MOTION_V2_CANDIDATE.sha256,
    decodeConfig: decodeMotionV2Config, selection: motionV2Selection,
    presets(art: MotionArt) {
      assertMotionArt(art)
      return Object.freeze(manifest.presets.map((preset, index) => {
        const selected = validateMotionSelection({ mode: 'preset', preset: preset.local_key }, art)
        const config = motionV2Config(selected)
        requireMotion(config.representation === 'motion_preset' && config.preset_id === preset.preset_id)
        return Object.freeze({ id: preset.preset_id, label: `Preset ${index + 1}`, config })
      }))
    },
    replace(config: MotionV2Config, selection: Parameters<typeof motionV2Config>[0]) {
      decodeMotionV2Config(config); return motionV2Config(selection)
    },
  })
}
