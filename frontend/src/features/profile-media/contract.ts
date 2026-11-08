/**
 * WP-06: the profile post and profile media wire contract.
 *
 * These are the eight frozen §2.3 operations, decoded strictly. The decoder is
 * the only place a response shape is trusted, and it refuses anything the
 * contract does not declare: an extra field, a missing field, a wrong enum, a
 * non-canonical UUID, a media purpose from the concert scope.
 *
 * Two rules are structural:
 *
 * * **No event anywhere.** A profile post and a profile photograph have no
 *   event scope. The paths are constants here, and a decoder that sees an
 *   `event_id` refuses the payload rather than carrying it into the UI.
 * * **A media purpose is one of the four profile purposes.** The frozen
 *   `feed.Media.purpose` enum still lists the concert purposes, so the contract
 *   cannot express the restriction; this decoder can, and does. A profile
 *   gallery that received a `POST`-purpose asset would be a contract violation,
 *   not something to render.
 */
import { z } from 'zod'
import { codePointLength } from '../../shared/validation/text'

const uuid = z.string().regex(/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/)
  .refine(value => value !== '00000000-0000-0000-0000-000000000000', 'A nil UUID is never an id.')
const revision = z.number().int().min(1).max(Number.MAX_SAFE_INTEGER)
const timestamp = z.string().refine(value => {
  const parsed = Date.parse(value)
  return Number.isFinite(parsed) && /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?Z$/.test(value)
}, 'A UTC timestamp ends in Z.')
// The frozen `common.UtcTimestamp` allows a fractional second, which the server
// emits; `expires_at` is a timestamp and not a date.
const boundedText = (maximum: number) => z.string().refine(value => codePointLength(value) <= maximum, `At most ${maximum} code points.`)

/** The four profile media purposes. The concert purposes are not profile media. */
export const PROFILE_MEDIA_PURPOSES = ['PROFILE_IMAGE', 'PROFILE_POST', 'INTRO_POST', 'ROOM_POSTER'] as const
export type ProfileMediaPurpose = typeof PROFILE_MEDIA_PURPOSES[number]

/** The three content types the foundation serves. */
export const MEDIA_CONTENT_TYPES = ['image/jpeg', 'image/png', 'image/webp'] as const
export type MediaContentType = typeof MEDIA_CONTENT_TYPES[number]

/** The frozen paths. There is no event segment in any of them. */
export const PATHS = Object.freeze({
  listPosts: (profileId: string) => `/api/profiles/${profileId}/posts`,
  createPost: '/api/me/posts',
  deletePost: (postId: string) => `/api/me/posts/${postId}`,
  initUpload: '/api/me/media/uploads',
  putBytes: (mediaId: string) => `/api/me/media/${mediaId}/bytes`,
  finalize: (mediaId: string) => `/api/me/media/${mediaId}/finalize`,
  media: (mediaId: string) => `/api/media/${mediaId}`,
  mediaContent: (mediaId: string) => `/api/media/${mediaId}/content`,
})

/** `feed.Media`, restricted to the profile purposes. */
export const mediaSchema = z.strictObject({
  media_id: uuid,
  state: z.literal('READY'),
  revision,
  width: z.number().int().min(1).max(4096),
  height: z.number().int().min(1).max(4096),
  content_type: z.enum(MEDIA_CONTENT_TYPES),
  purpose: z.enum(PROFILE_MEDIA_PURPOSES),
  read_path: z.string().regex(/^\/api\/media\/[0-9a-f-]{36}\/content$/),
})
export type ProfileMedia = z.infer<typeof mediaSchema>

/** The author subset the concert service projects. */
const authorSchema = z.strictObject({ profile_id: uuid })

/** `social.ProfilePost`. */
export const profilePostSchema = z.strictObject({
  post_id: uuid,
  author: authorSchema,
  description: boundedText(500),
  media: z.array(mediaSchema).max(4),
  visibility: z.enum(['PUBLIC', 'PRIVATE']),
  created_at: timestamp,
  expires_at: timestamp.nullable(),
  revision,
})
export type ProfilePost = z.infer<typeof profilePostSchema>

/** `social.ProfilePostPage`. */
export const profilePostPageSchema = z.strictObject({
  items: z.array(profilePostSchema).max(100),
  next_cursor: z.string().min(1).max(1024).regex(/^[A-Za-z0-9_-]+$/).nullable(),
})
export type ProfilePostPage = z.infer<typeof profilePostPageSchema>

/** `common.Deleted`. */
export const deletedSchema = z.strictObject({
  resource_id: uuid,
  revision,
  state: z.literal('DELETED'),
})
export type Deleted = z.infer<typeof deletedSchema>

/** `feed.UploadIntent`. */
export const uploadIntentSchema = z.strictObject({
  media_id: uuid,
  state: z.literal('PENDING'),
  upload_path: z.string().regex(/^\/api\/me\/media\/[0-9a-f-]{36}\/bytes$/),
  max_bytes: z.literal(10 * 1024 * 1024),
  expires_at: timestamp,
  upload_method: z.literal('PUT'),
  revision,
})
export type UploadIntent = z.infer<typeof uploadIntentSchema>

/** `feed.UploadBytesResult`. */
export const uploadBytesResultSchema = z.strictObject({
  media_id: uuid,
  state: z.literal('UPLOADED'),
  revision,
})
export type UploadBytesResult = z.infer<typeof uploadBytesResultSchema>

/** `social.CreateProfilePost`. */
export type CreateProfilePost = Readonly<{
  description: string
  media_ids: readonly string[]
  visibility: 'PUBLIC' | 'PRIVATE'
  expires_at: string | null
}>

/** `feed.InitUpload`, restricted to the profile purposes. */
export type InitUpload = Readonly<{
  purpose: ProfileMediaPurpose
  content_type: MediaContentType
  byte_length: number
  sha256: string
}>

/** `feed.FinalizeUpload`. */
export type FinalizeUpload = Readonly<{ expected_revision: number }>

/**
 * Decode a value and refuse a payload that names an event.
 *
 * A profile response has no event scope. If a payload carries an `event_id` or
 * an `eventId` anywhere, it came from the wrong producer and must not reach the
 * UI.
 */
export function decodeWithoutEvent<T>(schema: z.ZodType<T>, value: unknown): T {
  const text = JSON.stringify(value)
  if (text.includes('event_id') || text.includes('eventId') || text.includes('"events"')) {
    throw new Error('A profile representation must not carry an event.')
  }
  return schema.parse(value)
}

/** Decode one profile post. */
export const decodeProfilePost = (value: unknown): ProfilePost => decodeWithoutEvent(profilePostSchema, value)

/** Decode one page. */
export const decodeProfilePostPage = (value: unknown): ProfilePostPage => decodeWithoutEvent(profilePostPageSchema, value)

/** Decode a delete acknowledgement. */
export const decodeDeleted = (value: unknown): Deleted => decodeWithoutEvent(deletedSchema, value)

/** Decode an upload intent. */
export const decodeUploadIntent = (value: unknown): UploadIntent => decodeWithoutEvent(uploadIntentSchema, value)

/** Decode a byte-upload acknowledgement. */
export const decodeUploadBytesResult = (value: unknown): UploadBytesResult =>
  decodeWithoutEvent(uploadBytesResultSchema, value)

/** Decode a `feed.Media` projection. */
export const decodeMedia = (value: unknown): ProfileMedia => decodeWithoutEvent(mediaSchema, value)

/**
 * The canonical JSON of a `CreateProfilePost`.
 *
 * Field order is fixed so the body is byte-identical for the same draft, which
 * makes the idempotency binding meaningful.
 */
export function createProfilePostBody(command: CreateProfilePost): {
  readonly description: string
  readonly media_ids: readonly string[]
  readonly visibility: 'PUBLIC' | 'PRIVATE'
  readonly expires_at: string | null
} {
  return Object.freeze({
    description: command.description,
    media_ids: Object.freeze([...command.media_ids]),
    visibility: command.visibility,
    expires_at: command.expires_at,
  })
}
