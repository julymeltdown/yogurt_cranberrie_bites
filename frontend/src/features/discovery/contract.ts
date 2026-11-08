import { z } from 'zod'
import { uuid, freeze } from '../people/contract'
import { favoriteItemSchema, type Category, type FavoriteItem } from '../favorites/contract'

/**
 * WP-03 T-805: the discovery contract.
 *
 * The product shows two lists that are **never merged**:
 *
 * * **People** — `core.PeoplePage`, profiles whose catalog picks overlap with
 *   the viewer's. This file decodes that page.
 * * **Mutuals-intro posts** — `social.IntroPostPage`, post cards with their own
 *   heart button and their own audience filter. That is a different operation
 *   with a different schema, and it is decoded elsewhere; nothing in this module
 *   can turn an intro post into a people card or the reverse.
 *
 * The people card has no score, no rank and no match level. `shared_items` is a
 * required array: when there is no overlap it is empty, and the card then shows
 * no shared-interest line at all rather than an empty claim.
 */

export const peopleCardSchema = z.strictObject({
  profile_id: uuid,
  nickname: z.string().min(1).max(40),
  profile_image_media_id: uuid.nullable(),
  shared_items: z.array(favoriteItemSchema).min(1).max(15),
  interaction_style: z.string().max(120).nullable(),
})
export type PeopleCard = z.infer<typeof peopleCardSchema>

export const peoplePageSchema = z.strictObject({
  items: z.array(peopleCardSchema).max(100),
  next_cursor: z.string().min(1).max(1024).regex(/^[A-Za-z0-9_-]+$/).nullable(),
})
export type PeoplePage = z.infer<typeof peoplePageSchema>

/** The tabs of the discovery screen, in the product's order. */
export const DISCOVERY_TABS = ['PEOPLE', 'MUTUALS_INTROS'] as const
export type DiscoveryTab = (typeof DISCOVERY_TABS)[number]

/** The people sub-tabs, in the product's order: music, anime, games. */
export const PEOPLE_CATEGORIES = ['MUSIC', 'ANIME', 'GAME'] as const

/** The default people tab is music. */
export const DEFAULT_PEOPLE_CATEGORY: Category = 'MUSIC'

/** The label each people tab shows. */
export const PEOPLE_CATEGORY_LABEL: Readonly<Record<Category, string>> = Object.freeze({
  MUSIC: 'Bands and artists',
  ANIME: 'Anime',
  GAME: 'Games',
})

/**
 * The shared-interest line: "You both love MCR and NANA".
 *
 * Returns `null` when there is no overlap. That is the whole rule — the line is
 * omitted, never rendered empty and never replaced with a generic "you might
 * get along" claim.
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
 * Decode a people page. A card that claims a shared interest must name at least
 * one item, so a card with an empty `shared_items` is refused here rather than
 * rendered as a claim with nothing behind it.
 */
export function decodePeoplePage(value: unknown): PeoplePage {
  const page = peoplePageSchema.parse(value)
  if (new Set(page.items.map((card) => card.profile_id)).size !== page.items.length) {
    throw new Error('Duplicate discovery card')
  }
  return freeze(page)
}
