/**
 * WP-06 T-508: alt text and description validation.
 *
 * The limits are the server's, not a client convenience:
 *
 * | Field | Limit | Where the server enforces it |
 * | --- | --- | --- |
 * | description | 500 code points | `profile_posts.description` CHECK `char_length(description) <= 500` |
 * | alt text | 160 code points | the frozen `social.ProfilePostMedia` alt bound |
 * | attachments | 4 | `profile_post_attachments.position between 0 and 3` |
 *
 * The count is **Unicode code points**, not UTF-16 code units and not grapheme
 * clusters. That is what PostgreSQL's `char_length` counts and what the server
 * module counts with `str::chars`, so all three agree on an emoji (one code
 * point, two UTF-16 units), a combining mark (two code points) and a flag (two
 * code points). A client that counted `.length` would let a 250-emoji
 * description through and then be refused by the server.
 *
 * This module is pure: it never sends a request and never decides visibility.
 */
import { codePointLength } from '../../shared/validation/text'

/** The frozen description bound, in Unicode code points. */
export const MAX_DESCRIPTION_CODE_POINTS = 500
/** The frozen alt-text bound, in Unicode code points. */
export const MAX_ALT_TEXT_CODE_POINTS = 160
/** The frozen attachment count bound. */
export const MAX_ATTACHMENTS = 4
/** The foundation's byte bound for one upload (10 MiB). */
export const MAX_UPLOAD_BYTES = 10 * 1024 * 1024
/** The foundation's per-side pixel bound. */
export const MAX_DIMENSION = 4096
/** The foundation's pixel-product bound. */
export const MAX_PIXELS = 16_000_000
/** The only content types the foundation accepts. GIF and video are not. */
export const ACCEPTED_CONTENT_TYPES = ['image/jpeg', 'image/png', 'image/webp'] as const
export type AcceptedContentType = typeof ACCEPTED_CONTENT_TYPES[number]

/** The closed visibility set of a profile post. */
export type PostVisibility = 'PUBLIC' | 'PRIVATE'

export type FieldIssue = Readonly<{
  field: 'description' | 'alt_text' | 'media' | 'visibility'
  /** The index of the attachment the issue is about, when it is about one. */
  attachment?: number
  code: 'TOO_LONG' | 'TOO_MANY' | 'UNSUPPORTED_TYPE' | 'TOO_LARGE' | 'TOO_MANY_PIXELS' | 'EMPTY'
  message: string
}>

/** The code-point count of a string, the same number the server counts. */
export const countedLength = (value: string): number => codePointLength(value)

/**
 * Validate a description.
 *
 * An empty description is valid: a profile post may be a photograph alone, and
 * the frozen `CreateProfilePost.description` has `minLength: 0`.
 */
export function validateDescription(value: string): FieldIssue | null {
  if (value.includes('\u0000')) {
    return { field: 'description', code: 'EMPTY', message: 'Remove the unsupported character.' }
  }
  const length = countedLength(value)
  if (length > MAX_DESCRIPTION_CODE_POINTS) {
    return {
      field: 'description',
      code: 'TOO_LONG',
      message: `Shorten the description to ${MAX_DESCRIPTION_CODE_POINTS} characters or fewer (${length} now).`,
    }
  }
  return null
}

/** Validate one alt text. An empty alt text is valid; the field is optional. */
export function validateAltText(value: string, attachment = 0): FieldIssue | null {
  if (value.includes('\u0000')) {
    return {
      field: 'alt_text',
      attachment,
      code: 'EMPTY',
      message: 'Remove the unsupported character from the alt text.',
    }
  }
  const length = countedLength(value)
  if (length > MAX_ALT_TEXT_CODE_POINTS) {
    return {
      field: 'alt_text',
      attachment,
      code: 'TOO_LONG',
      message: `Shorten the alt text to ${MAX_ALT_TEXT_CODE_POINTS} characters or fewer (${length} now).`,
    }
  }
  return null
}

/** Validate a picked file against the foundation's byte and type limits. */
export function validateFile(file: { readonly type: string; readonly size: number }, attachment = 0): FieldIssue | null {
  if (!(ACCEPTED_CONTENT_TYPES as readonly string[]).includes(file.type)) {
    return {
      field: 'media',
      attachment,
      code: 'UNSUPPORTED_TYPE',
      message: 'Choose a JPEG, PNG or WebP image. GIF and video are not supported.',
    }
  }
  if (!Number.isSafeInteger(file.size) || file.size < 1 || file.size > MAX_UPLOAD_BYTES) {
    return {
      field: 'media',
      attachment,
      code: 'TOO_LARGE',
      message: 'Choose an image of 10 MiB or smaller.',
    }
  }
  return null
}

/** Validate a decoded image's dimensions against the foundation's limits. */
export function validateDimensions(width: number, height: number, attachment = 0): FieldIssue | null {
  if (
    !Number.isSafeInteger(width) ||
    !Number.isSafeInteger(height) ||
    width < 1 ||
    height < 1 ||
    width > MAX_DIMENSION ||
    height > MAX_DIMENSION ||
    width * height > MAX_PIXELS
  ) {
    return {
      field: 'media',
      attachment,
      code: 'TOO_MANY_PIXELS',
      message: 'Choose an image of at most 4096 px per side and 16 megapixels.',
    }
  }
  return null
}

/** Validate the attachment count. */
export function validateAttachmentCount(count: number): FieldIssue | null {
  if (!Number.isSafeInteger(count) || count < 0 || count > MAX_ATTACHMENTS) {
    return {
      field: 'media',
      code: 'TOO_MANY',
      message: `A post can carry at most ${MAX_ATTACHMENTS} images.`,
    }
  }
  return null
}

/** Validate a visibility value. */
export function validateVisibility(value: string): FieldIssue | null {
  if (value !== 'PUBLIC' && value !== 'PRIVATE') {
    return { field: 'visibility', code: 'EMPTY', message: 'Choose who can see this post.' }
  }
  return null
}

/** The whole composer draft, as the client validates it before upload. */
export type ProfilePostDraft = Readonly<{
  description: string
  visibility: PostVisibility
  attachments: readonly Readonly<{
    /** A client-local id; the server media id arrives after finalize. */
    localId: string
    altText: string
    contentType: AcceptedContentType
    size: number
    width: number
    height: number
  }>[]
}>

/** Validate a whole draft. Returns every issue, in field order. */
export function validateDraft(draft: ProfilePostDraft): readonly FieldIssue[] {
  const issues: FieldIssue[] = []
  const description = validateDescription(draft.description)
  if (description) issues.push(description)
  const visibility = validateVisibility(draft.visibility)
  if (visibility) issues.push(visibility)
  const count = validateAttachmentCount(draft.attachments.length)
  if (count) issues.push(count)
  draft.attachments.forEach((attachment, index) => {
    const file = validateFile({ type: attachment.contentType, size: attachment.size }, index)
    if (file) issues.push(file)
    const dimensions = validateDimensions(attachment.width, attachment.height, index)
    if (dimensions) issues.push(dimensions)
    const alt = validateAltText(attachment.altText, index)
    if (alt) issues.push(alt)
  })
  return Object.freeze(issues)
}

/** Whether a draft may be published. */
export const draftIsPublishable = (draft: ProfilePostDraft): boolean => validateDraft(draft).length === 0

/**
 * The remaining character budget for a field, never negative.
 *
 * The composer shows this next to the counter, so a user sees how much room is
 * left rather than a bare number they must compare against a limit.
 */
export const remaining = (value: string, maximum: number): number =>
  Math.max(0, maximum - countedLength(value))
