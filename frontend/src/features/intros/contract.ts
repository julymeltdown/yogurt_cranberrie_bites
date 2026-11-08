import { z } from 'zod'
import { uuid, freeze } from '../people/contract'
import { favoriteItemSchema, type FavoriteItem } from '../favorites/contract'

/**
 * WP-05: the mutuals-intro post contract.
 *
 * This is the client half of the frozen `social.*` schemas: `IntroPostDetail`,
 * `OwnIntroPost`, `SaveIntroPost`, `IntroLikeState`, `LikerPage`,
 * `IntroComment`, and the audience object.
 *
 * Two rules are encoded here rather than left to a component:
 *
 * * the **audience summary is never computed on the client**. `OwnIntroPost`
 *   carries `audience_summary` as a required string, and nothing in this module
 *   derives a summary from the bounds. The review step renders the server's
 *   string, so the two can never disagree.
 * * a **viewer projection never carries the audience**. `IntroPostDetail` has no
 *   `audience`, `minimum_age` or `maximum_age_exclusive` field, and the decoder
 *   is strict, so a server that leaked one would fail to decode rather than
 *   quietly expose it.
 */

/** The closed gender condition. `unknown` is not a wildcard. */
export const GENDERS = ['any', 'female', 'male', 'unknown'] as const
export type GenderCondition = (typeof GENDERS)[number]

/** The frozen age window. */
export const MIN_AGE = 14
export const MAX_AGE = 100

export const audienceSchema = z
  .strictObject({
    allowed_gender: z.enum(GENDERS),
    minimum_age: z.number().int().min(MIN_AGE).max(MAX_AGE).nullable(),
    maximum_age_exclusive: z.number().int().min(MIN_AGE + 1).max(MAX_AGE + 1).nullable(),
  })
  .refine(
    (value) =>
      (value.minimum_age === null) === (value.maximum_age_exclusive === null) &&
      (value.minimum_age === null ||
        (value.maximum_age_exclusive !== null && value.minimum_age < value.maximum_age_exclusive)),
    { message: 'The age range must be complete and non-empty.' },
  )
export type Audience = z.infer<typeof audienceSchema>

export const ANYONE: Audience = freeze({
  allowed_gender: 'any',
  minimum_age: null,
  maximum_age_exclusive: null,
})

/**
 * The inclusive range the interface shows, converted to the half-open wire form.
 *
 * "Ages 18–29" is `minimum_age: 18, maximum_age_exclusive: 30`. The conversion
 * lives here, once, so the composer, the reducer and the test cannot disagree.
 */
export function inclusiveToHalfOpen(minimum: number, maximumInclusive: number): {
  minimum_age: number
  maximum_age_exclusive: number
} {
  return { minimum_age: minimum, maximum_age_exclusive: maximumInclusive + 1 }
}

/** The half-open wire form back to the inclusive display form. */
export function halfOpenToInclusive(minimum: number, maximumExclusive: number): {
  minimum: number
  maximumInclusive: number
} {
  return { minimum, maximumInclusive: maximumExclusive - 1 }
}

/** Whether an inclusive range is valid: integers, in window, and ordered. */
export function validInclusiveRange(minimum: number, maximumInclusive: number): boolean {
  return (
    Number.isInteger(minimum) &&
    Number.isInteger(maximumInclusive) &&
    minimum >= MIN_AGE &&
    maximumInclusive <= MAX_AGE &&
    minimum <= maximumInclusive
  )
}

/** Build an audience from the interface's inclusive inputs. */
export function audienceFromInclusive(
  gender: GenderCondition,
  range: { minimum: number; maximumInclusive: number } | null,
): Audience | null {
  if (range === null) {
    return freeze({ allowed_gender: gender, minimum_age: null, maximum_age_exclusive: null })
  }
  if (!validInclusiveRange(range.minimum, range.maximumInclusive)) return null
  const half = inclusiveToHalfOpen(range.minimum, range.maximumInclusive)
  return freeze({ allowed_gender: gender, ...half })
}

// ---------------------------------------------------------------------------
// Posts
// ---------------------------------------------------------------------------

export const INTRO_STATES = ['DRAFT', 'PUBLISHED', 'DELETED'] as const
export type IntroState = (typeof INTRO_STATES)[number]

export const MAX_BODY = 1000
export const MAX_SINGLE_LINE = 120
export const MAX_COMMENT = 500

/** `social.OwnIntroPost` — the author-only projection. */
export const ownIntroPostSchema = z.strictObject({
  intro_id: uuid,
  state: z.enum(INTRO_STATES),
  body: z.string().max(MAX_BODY),
  favorites_snapshot: z.array(favoriteItemSchema).max(15),
  currently_seeking: z.string().max(MAX_SINGLE_LINE).nullable(),
  interaction_style: z.string().max(MAX_SINGLE_LINE).nullable(),
  attachment_media_ids: z.array(uuid).max(4),
  audience: audienceSchema,
  // Server-derived. The client renders this string and never recomputes it.
  audience_summary: z.string().min(1).max(80),
  like_count: z.number().int().min(0),
  comment_count: z.number().int().min(0),
  published_at: z.string().nullable(),
  expires_at: z.string().nullable(),
  revision: z.number().int().min(1),
  updated_at: z.string(),
})
export type OwnIntroPost = z.infer<typeof ownIntroPostSchema>

/** `social.IntroLikeSummary`, as it appears inside a detail. */
export const introLikeSummarySchema = z.strictObject({
  count: z.number().int().min(0),
  liked_by_viewer: z.boolean(),
})

/** `feed.Media`, as an attachment appears. */
export const mediaSchema = z.strictObject({
  media_id: uuid,
  content_type: z.enum(['image/jpeg', 'image/png', 'image/webp']),
  width: z.number().int().min(1),
  height: z.number().int().min(1),
  revision: z.number().int().min(1),
  read_path: z.string().min(1),
})
export type Media = z.infer<typeof mediaSchema>

/** A minimal public author projection, as a card needs it. */
export const introAuthorSchema = z.strictObject({
  profile_id: uuid,
  nickname: z.string().min(1).max(40),
  profile_image_media_id: uuid.nullable(),
})

/**
 * `social.IntroPostDetail` — the viewer-facing projection.
 *
 * Deliberately absent: `audience`, `minimum_age`, `maximum_age_exclusive`,
 * `allowed_gender` and `room_id`. The object is strict, so a server that sent
 * one of them would fail to decode.
 */
export const introPostDetailSchema = z.strictObject({
  intro_id: uuid,
  author: introAuthorSchema,
  body: z.string().min(1).max(MAX_BODY),
  favorites_snapshot: z.array(favoriteItemSchema).max(15),
  currently_seeking: z.string().max(MAX_SINGLE_LINE).nullable(),
  interaction_style: z.string().max(MAX_SINGLE_LINE).nullable(),
  attachments: z.array(mediaSchema).max(4),
  like: introLikeSummarySchema,
  comment_count: z.number().int().min(0),
  published_at: z.string(),
  expires_at: z.string().nullable(),
})
export type IntroPostDetail = z.infer<typeof introPostDetailSchema>

export const introPostPageSchema = z.strictObject({
  items: z.array(introPostDetailSchema).max(100),
  next_cursor: z.string().min(1).max(1024).regex(/^[A-Za-z0-9_-]+$/).nullable(),
})
export type IntroPostPage = z.infer<typeof introPostPageSchema>

/** `social.IntroLikeState`. `like_count` is null only after access is lost. */
export const introLikeStateSchema = z.strictObject({
  liked: z.boolean(),
  like_count: z.number().int().min(0).nullable(),
})
export type IntroLikeState = z.infer<typeof introLikeStateSchema>

/** `social.LikerProfile`. */
export const likerProfileSchema = z.strictObject({
  profile_id: uuid,
  nickname: z.string().min(1).max(40),
  profile_image_media_id: uuid.nullable(),
  shared_with_author: z.array(favoriteItemSchema).max(15),
  interaction_style: z.string().max(MAX_SINGLE_LINE).nullable(),
  liked_at: z.string(),
})
export type LikerProfile = z.infer<typeof likerProfileSchema>

export const likerPageSchema = z.strictObject({
  items: z.array(likerProfileSchema).max(100),
  next_cursor: z.string().min(1).max(1024).regex(/^[A-Za-z0-9_-]+$/).nullable(),
})
export type LikerPage = z.infer<typeof likerPageSchema>

/** `social.IntroComment`. */
export const introCommentSchema = z.strictObject({
  comment_id: uuid,
  intro_id: uuid,
  author: introAuthorSchema,
  parent_comment_id: uuid.nullable(),
  body: z.string().max(MAX_COMMENT),
  created_at: z.string(),
  deleted: z.boolean(),
})
export type IntroComment = z.infer<typeof introCommentSchema>

export const introCommentPageSchema = z.strictObject({
  items: z.array(introCommentSchema).max(100),
  next_cursor: z.string().min(1).max(1024).regex(/^[A-Za-z0-9_-]+$/).nullable(),
})
export type IntroCommentPage = z.infer<typeof introCommentPageSchema>

/** `social.SaveIntroPost`. */
export const saveIntroPostSchema = z.strictObject({
  expected_revision: z.number().int().min(0),
  body: z.string().max(MAX_BODY),
  favorites_snapshot: z.array(favoriteItemSchema).max(15),
  currently_seeking: z.string().max(MAX_SINGLE_LINE).nullable(),
  interaction_style: z.string().max(MAX_SINGLE_LINE).nullable(),
  attachment_media_ids: z.array(uuid).max(4),
  audience: audienceSchema,
  expires_at: z.string().nullable(),
})
export type SaveIntroPost = z.infer<typeof saveIntroPostSchema>

// ---------------------------------------------------------------------------
// Decoders
// ---------------------------------------------------------------------------

export function decodeOwnIntroPost(value: unknown): OwnIntroPost {
  return freeze(ownIntroPostSchema.parse(value))
}

export function decodeIntroPostDetail(value: unknown): IntroPostDetail {
  return freeze(introPostDetailSchema.parse(value))
}

export function decodeIntroPostPage(value: unknown): IntroPostPage {
  return freeze(introPostPageSchema.parse(value))
}

export function decodeLikeState(value: unknown): IntroLikeState {
  return freeze(introLikeStateSchema.parse(value))
}

export function decodeLikerPage(value: unknown): LikerPage {
  return freeze(likerPageSchema.parse(value))
}

export function decodeCommentPage(value: unknown): IntroCommentPage {
  return freeze(introCommentPageSchema.parse(value))
}

export function decodeIntroComment(value: unknown): IntroComment {
  return freeze(introCommentSchema.parse(value))
}

/**
 * The shared-interest line for a liker row: "You both love MCR and NANA".
 *
 * `null` when there is no overlap, so the row omits the line entirely rather
 * than showing an empty claim.
 */
export function sharedInterestLine(items: readonly FavoriteItem[]): string | null {
  const names = items.map((item) => item.label).filter((label) => label.trim().length > 0)
  if (names.length === 0) return null
  const shown = names.slice(0, 3)
  const rest = names.length - shown.length
  const list =
    shown.length === 1
      ? shown[0]
      : shown.length === 2
        ? `${shown[0]} and ${shown[1]}`
        : `${shown.slice(0, -1).join(', ')} and ${shown[shown.length - 1]}`
  return `You both love ${list}${rest > 0 ? ` and ${rest} more` : ''}.`
}

/**
 * The like count as a display string.
 *
 * A `null` count means "the viewer may no longer see the count", and it renders
 * as an em dash. It must never render as `0`, because zero is a claim and null
 * is the absence of one.
 */
export function likeCountLabel(count: number | null): string {
  return count === null ? '\u2014' : String(count)
}
