import { z } from 'zod'
import { uuid, freeze } from '../people/contract'
import { favoriteItemSchema } from '../favorites/contract'

/**
 * WP-05 T-415: the activity feed contract.
 *
 * The four kinds are likes received, comments, replies and follows. There is
 * deliberately **no** `pending`, `unanswered`, `accepted` or `deadline` field
 * anywhere in this schema, and the decoder is strict, so a server that sent one
 * would fail to decode rather than quietly render an approval affordance.
 *
 * A like is not a request. The product says so explicitly, and this schema is
 * where that is enforced on the client.
 */
export const ACTIVITY_KINDS = ['INTRO_LIKED', 'INTRO_COMMENT', 'INTRO_REPLY', 'FOLLOW'] as const
export type ActivityKind = (typeof ACTIVITY_KINDS)[number]

export const activityActorSchema = z.strictObject({
  profile_id: uuid,
  nickname: z.string().min(1).max(40),
  profile_image_media_id: uuid.nullable(),
  shared_with_author: z.array(favoriteItemSchema).max(15),
  interaction_style: z.string().max(120).nullable(),
})
export type ActivityActor = z.infer<typeof activityActorSchema>

export const activityItemSchema = z.strictObject({
  activity_id: uuid,
  kind: z.enum(ACTIVITY_KINDS),
  actor: activityActorSchema,
  intro_id: uuid.nullable(),
  comment_id: uuid.nullable(),
  created_at: z.string(),
  read: z.boolean(),
})
export type ActivityItem = z.infer<typeof activityItemSchema>

export const activityPageSchema = z.strictObject({
  items: z.array(activityItemSchema).max(100),
  next_cursor: z.string().min(1).max(1024).regex(/^[A-Za-z0-9_-]+$/).nullable(),
})
export type ActivityPage = z.infer<typeof activityPageSchema>

export function decodeActivityPage(value: unknown): ActivityPage {
  return freeze(activityPageSchema.parse(value))
}

/** The sentence an entry shows, in the product's words. */
export function activityLabel(item: ActivityItem): string {
  switch (item.kind) {
    case 'INTRO_LIKED':
      return `${item.actor.nickname} left a heart on your mutuals-intro post`
    case 'INTRO_COMMENT':
      return `${item.actor.nickname} commented on your mutuals-intro post`
    case 'INTRO_REPLY':
      return `${item.actor.nickname} replied to your comment`
    case 'FOLLOW':
      return `${item.actor.nickname} followed you`
  }
}

/**
 * The one-tap destination of an entry.
 *
 * A like opens the liker's profile — never an acceptance screen and never the
 * room. A comment or reply opens the post. A follow opens the follower.
 */
export function activityTarget(item: ActivityItem): string {
  if (item.kind === 'FOLLOW' || item.kind === 'INTRO_LIKED') {
    return `/profiles/${item.actor.profile_id}`
  }
  return item.intro_id === null ? `/profiles/${item.actor.profile_id}` : `/intro-posts/${item.intro_id}`
}
