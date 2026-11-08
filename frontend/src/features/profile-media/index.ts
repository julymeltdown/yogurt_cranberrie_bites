/**
 * WP-06: the profile media feature.
 *
 * Everything the profile screen needs to publish an ordinary photograph and to
 * show the profile's posts. Nothing here accepts an event id, a participant key
 * or an intro post id: a profile post is independent of all three.
 */
export {
  createProfileMediaComposer,
  type ComposerAttachment,
  type ComposerDeps,
  type ComposerState,
  type ProfileMediaComposerController,
  type PublishPhase,
} from './controller'
export {
  createGalleryReader,
  createGalleryStore,
  admitGalleryMedia,
  ProfileMediaGallery,
  type GalleryDeps,
  type GalleryReader,
  type GalleryState,
  type GalleryStore,
  type ProfileMediaGalleryProps,
} from './ProfileMediaGallery'
export { ProfileMediaComposer, type ProfileMediaComposerProps } from './ProfileMediaComposer'
export {
  createProfileMediaUpload,
  cryptoIdempotencyKey,
  classifyFailure,
  failureMessage,
  retryable,
  type ProfileMediaUpload,
  type UploadAuth,
  type UploadDeps,
  type UploadFailure,
  type UploadPhase,
  type UploadState,
} from './upload'
export {
  ACCEPTED_CONTENT_TYPES,
  MAX_ALT_TEXT_CODE_POINTS,
  MAX_ATTACHMENTS,
  MAX_DESCRIPTION_CODE_POINTS,
  MAX_DIMENSION,
  MAX_PIXELS,
  MAX_UPLOAD_BYTES,
  countedLength,
  draftIsPublishable,
  remaining,
  validateAltText,
  validateAttachmentCount,
  validateDescription,
  validateDimensions,
  validateDraft,
  validateFile,
  validateVisibility,
  type AcceptedContentType,
  type FieldIssue,
  type PostVisibility,
  type ProfilePostDraft,
} from './validation'
export {
  PATHS,
  PROFILE_MEDIA_PURPOSES,
  MEDIA_CONTENT_TYPES,
  createProfilePostBody,
  decodeDeleted,
  decodeMedia,
  decodeProfilePost,
  decodeProfilePostPage,
  decodeUploadBytesResult,
  decodeUploadIntent,
  type CreateProfilePost,
  type Deleted,
  type FinalizeUpload,
  type InitUpload,
  type MediaContentType,
  type ProfileMedia,
  type ProfileMediaPurpose,
  type ProfilePost,
  type ProfilePostPage,
  type UploadBytesResult,
  type UploadIntent,
} from './contract'
