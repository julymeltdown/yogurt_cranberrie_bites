/**
 * WP-08 T-704: the profile-owned emo avatar entity.
 *
 * The character artwork, compositor and runtime, re-keyed from concert
 * participation to profile ownership. The source pack
 * (`entities/avatar/character-source-pack.json`) and the atlas path/hash
 * contract are preserved: `MOTION_ART` pins exactly the same descriptor and
 * atlas paths, sha256 values and byte counts the concert editor pinned, and the
 * files are read from the same `frontend/public/world/avatars/derived-v1/`
 * location. There is no fallback artwork anywhere in this module: every
 * verification failure throws, and callers render an explicit unavailable state.
 */
export {
  MOTION_ART,
  MOTION_LAYERS,
  assertMotionArt,
  loadMotionArtwork,
  motionFrame,
  requireMotion,
  validateMotionArt,
  validateMotionSelection,
  type MotionArt,
  type MotionDirection,
  type MotionItem,
  type MotionLayer,
  type MotionSelection,
} from './art'
export { createMotionCompositor, type MotionComposition, type MotionCompositor, type MotionPart } from './compositor'
export {
  createMotionRuntime,
  type MotionAuthority,
  type MotionAuthorityOperations,
  type MotionLease,
  type MotionPartLease,
  type MotionRuntime,
  type ProfileAvatarScope,
} from './runtime'
export {
  REVIEWED_MOTION_FAMILY,
  decodeReviewedMotionConfig,
  decodeReviewedMotionConfigText,
  reviewedMotionConfig,
  reviewedMotionSelection,
  type ReviewedMotionConfig,
  type ReviewedMotionLayeredConfig,
  type ReviewedMotionPresetConfig,
} from './reviewed-config'
export {
  REVIEWED_MOTION,
  createReviewedMotionFixtureAuthority,
  decodeReviewedMotionManifest,
  reviewedMotionAuthorityOperations,
  type ReviewedMotionManifest,
  type ReviewedMotionPin,
  type ReviewedMotionStage,
} from './reviewed-manifest'
export {
  assertReviewedProductionAuthority,
  createReviewedProductionAuthority,
  isReviewedProductionAuthority,
  type PublicReviewedConfig,
  type ReviewedProductionAuth,
  type ReviewedProductionAuthority,
  type ReviewedProductionLifetime,
} from './reviewed-production'
