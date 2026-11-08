import type { CatalogItem } from './contract'

/**
 * WP-03 T-804: catalog presentation helpers.
 *
 * Adapted from the donor's `catalog-provider.ts`. What is kept is the part that
 * is still true: an external link is only offered when it is a safe `https`
 * URL, and the provider label is a closed map rather than raw server text.
 *
 * What is dropped is everything about the donor's five-category provider
 * integration: the `anilist`/`musicbrainz`/`spotify`/`tmdb`/`wikidata` id
 * synthesis, the TMDB attribution line, and the provider-derived URL building.
 * The destination catalog carries an optional `external_url` on the row itself
 * and nothing else, so there is no provider identity to render and no provider
 * to attribute.
 */

/** The only hosts an external link may point at when the row carries one. */
const SAFE_HOSTS = [
  'www.mychemicalromance.com',
  'falloutboy.com',
  'paramore.net',
  'www.piercetheveil.net',
  'www.bmthofficial.com',
  'www.sleepingwithsirens.net',
  'theused.net',
  'takingbacksunday.com',
  'www.jimmyeatworld.com',
  'www.americanfootballmusic.com',
  'www.radiohead.com',
  'www.nirvana.com',
  'www.linkinpark.com',
]

/**
 * A catalog row's external link, or `null` when it must not be offered.
 *
 * The check is deliberately strict: `https` only, and only a host this module
 * knows. A catalog row is operator-curated, but "curated" is not a security
 * boundary, so the client does not trust the URL it was given.
 */
export function catalogExternalUrl(item: Pick<CatalogItem, 'external_url'>): string | null {
  if (!item.external_url) return null
  try {
    const url = new URL(item.external_url)
    if (url.protocol !== 'https:') return null
    if (url.username || url.password) return null
    return SAFE_HOSTS.includes(url.hostname) ? url.toString() : null
  } catch {
    return null
  }
}

/** The link label for a catalog row, or `null` when no link is offered. */
export function catalogExternalLinkLabel(item: Pick<CatalogItem, 'label'>): string {
  return `Open ${item.label} in a new tab`
}

/**
 * The one-line summary under a search result. The destination catalog has a
 * label and a category and nothing else, so this never invents a subtitle.
 */
export function catalogSubtitle(item: Pick<CatalogItem, 'category'>): string {
  return item.category === 'MUSIC' ? 'Band or artist' : item.category === 'ANIME' ? 'Anime' : 'Game'
}
