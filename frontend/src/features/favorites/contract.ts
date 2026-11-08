import { z } from 'zod'
import { uuid, freeze } from '../people/contract'

/**
 * WP-03 T-804: the favorites contract.
 *
 * This is the client half of the frozen `core.CatalogQuery`, `core.CatalogPage`,
 * `core.FavoriteItem` and `core.FavoriteSelection` schemas. It is deliberately
 * narrow:
 *
 * * exactly three categories, in the product's order: music, anime, games;
 * * at most five per category;
 * * no intensity, importance, weight or preference field anywhere. The donor's
 *   taste profile carried those; the destination does not, and the decoder
 *   would reject them because every object here is strict.
 */

/** The contract's category enum, in presentation order. */
export const CATEGORIES = ['MUSIC', 'ANIME', 'GAME'] as const
export type Category = (typeof CATEGORIES)[number]

/** The path segment each category uses: `/api/catalog/{segment}/search`. */
export const CATEGORY_PATH: Readonly<Record<Category, string>> = Object.freeze({
  MUSIC: 'music',
  ANIME: 'anime',
  GAME: 'game',
})

/** The `FavoriteSelection` key each category uses. */
export const CATEGORY_SELECTION_KEY: Readonly<Record<Category, 'music' | 'anime' | 'games'>> = Object.freeze({
  MUSIC: 'music',
  ANIME: 'anime',
  GAME: 'games',
})

/** The label each tab shows, in the product's own words. */
export const CATEGORY_LABEL: Readonly<Record<Category, string>> = Object.freeze({
  MUSIC: 'Bands and artists',
  ANIME: 'Anime',
  GAME: 'Games',
})

/** Up to five per category, from the frozen `FavoriteSelection`. */
export const MAX_PER_CATEGORY = 5

/** The contract's `CatalogQuery.q` bound. */
export const MAX_QUERY_LENGTH = 80
/** The contract's `CatalogQuery.limit` bound. */
export const MAX_PAGE = 100

export const categorySchema = z.enum(CATEGORIES)

/**
 * The contract's `FavoriteItem`. `category` must agree with the array the item
 * is stored in; that is checked by `selectionSchema`, not here.
 */
export const favoriteItemSchema = z.strictObject({
  catalog_item_id: uuid,
  label: z.string().min(1).max(80),
  category: categorySchema,
  external_url: z.url().max(2048).nullable(),
})
export type FavoriteItem = z.infer<typeof favoriteItemSchema>

/** A `FavoriteItem` is also a catalog search result (`core.CatalogItem`). */
export const catalogItemSchema = favoriteItemSchema
export type CatalogItem = FavoriteItem

export const catalogPageSchema = z.strictObject({
  category: categorySchema,
  items: z.array(catalogItemSchema).max(MAX_PAGE),
  next_cursor: z.string().min(1).max(1024).regex(/^[A-Za-z0-9_-]+$/).nullable(),
})
export type CatalogPage = z.infer<typeof catalogPageSchema>

/** The contract's `FavoriteSelection`: three arrays, at most five each. */
export const selectionSchema = z.strictObject({
  music: z.array(favoriteItemSchema).max(MAX_PER_CATEGORY),
  anime: z.array(favoriteItemSchema).max(MAX_PER_CATEGORY),
  games: z.array(favoriteItemSchema).max(MAX_PER_CATEGORY),
})
export type FavoriteSelection = z.infer<typeof selectionSchema>

export const emptySelection = (): FavoriteSelection =>
  freeze({ music: [], anime: [], games: [] })

/** The items selected in one category, in display order. */
export function itemsFor(selection: FavoriteSelection, category: Category): readonly FavoriteItem[] {
  return selection[CATEGORY_SELECTION_KEY[category]]
}

/** Whether the item is already selected in its category. */
export function isSelected(selection: FavoriteSelection, item: FavoriteItem): boolean {
  return itemsFor(selection, item.category).some(
    (existing) => existing.catalog_item_id === item.catalog_item_id,
  )
}

/** Whether another item may be added to this category. */
export function canAdd(selection: FavoriteSelection, category: Category): boolean {
  return itemsFor(selection, category).length < MAX_PER_CATEGORY
}

export type SelectionResult =
  | { readonly ok: true; readonly selection: FavoriteSelection }
  | { readonly ok: false; readonly reason: 'full' | 'duplicate' | 'wrong-category' }

/**
 * Add one item. The cap is enforced here as well as on the server, because a
 * picker that lets a user build a sixth selection and then fails the save is a
 * worse experience than one that never offers it.
 */
export function addFavorite(selection: FavoriteSelection, item: FavoriteItem): SelectionResult {
  const parsed = favoriteItemSchema.safeParse(item)
  if (!parsed.success) return { ok: false, reason: 'wrong-category' }
  const key = CATEGORY_SELECTION_KEY[parsed.data.category]
  const existing = selection[key]
  if (existing.some((row) => row.catalog_item_id === parsed.data.catalog_item_id)) {
    return { ok: false, reason: 'duplicate' }
  }
  if (existing.length >= MAX_PER_CATEGORY) return { ok: false, reason: 'full' }
  return {
    ok: true,
    selection: freeze({ ...selection, [key]: [...existing, parsed.data] }),
  }
}

/** Remove one item from its category. */
export function removeFavorite(selection: FavoriteSelection, category: Category, id: string): FavoriteSelection {
  const key = CATEGORY_SELECTION_KEY[category]
  return freeze({
    ...selection,
    [key]: selection[key].filter((row) => row.catalog_item_id !== id),
  })
}

/**
 * Validate a selection against the contract: at most five per category, no
 * duplicate ids, and every item's `category` matching its array.
 */
export function validateSelection(value: unknown): FavoriteSelection | null {
  const parsed = selectionSchema.safeParse(value)
  if (!parsed.success) return null
  for (const category of CATEGORIES) {
    const items = itemsFor(parsed.data, category)
    if (items.some((item) => item.category !== category)) return null
    if (new Set(items.map((item) => item.catalog_item_id)).size !== items.length) return null
  }
  return freeze(parsed.data)
}

/** The search term rules, matching the frozen `CatalogQuery.q`. */
export function validQuery(term: string): string | null {
  const trimmed = term.trim()
  if (trimmed.length === 0) return null
  if ([...trimmed].length > MAX_QUERY_LENGTH) return null
  if (/[\u0000-\u001f\u007f]/.test(trimmed)) return null
  return trimmed
}

/** Decode a catalog page, refusing a page from a different category. */
export function decodeCatalogPage(value: unknown, category: Category): CatalogPage {
  const page = catalogPageSchema.parse(value)
  if (page.category !== category) throw new Error('Invalid catalog category')
  if (page.items.some((item) => item.category !== category)) throw new Error('Invalid catalog item')
  return freeze(page)
}
