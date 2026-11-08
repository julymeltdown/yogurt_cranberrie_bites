/**
 * WP-08 T-704/T-705: the profile avatar feature.
 *
 * Everything the profile screen needs to show and edit the account's emo
 * character. The character is profile-owned: no export here accepts an event id
 * or a participant key, and the only HTTP path the feature uses is
 * `/api/me/avatar`.
 */
export { PROFILE_AVATAR_PATH, createProfileAvatarPersistence } from './api/persistence'
export {
  createProfileAvatarEditorController,
  type MotionAvatarState,
  type MotionEditorState,
  type MotionNotice,
  type MotionPersistence,
  type ProfileAvatarEditorController,
} from './model/controller'
export { createProfileAvatarEditorOwner, type ProfileAvatarEditorOwner } from './model/owner'
export { admitProfileAvatar, isReviewedMotionRelease, sameProfileScope } from './model/admission'
export {
  ACCOUNT_CONTEXT_STALE,
  accountVersion,
  bindAvatarCacheToPrivacy,
  createAccountScopedAvatarCache,
  isAccountContextStale,
  sameAccountVersion,
  type AccountScopedAvatarCache,
  type AccountVersion,
} from './model/cache'
export { ProfileAvatarEditor } from './ui/ProfileAvatarEditor'
export { ProfileAvatarPanel } from './ui/ProfileAvatarPanel'
