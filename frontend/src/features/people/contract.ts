import { z } from 'zod'
import { boundedText } from '../../shared/validation/text'
import { decodeAvatar } from '../avatar/contract'
import type { PrivacyScope } from '../../shared/query/privacy'
export const uuid = z.string().regex(/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/)
export const revision = z.number().int().min(1).max(Number.MAX_SAFE_INTEGER)
export const timestamp = z.iso.datetime().regex(/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?Z$/)
export const profileSchema = z.strictObject({ user_id: uuid, display_name: boundedText(1, 40), bio: boundedText(0, 160).nullable(), avatar: z.unknown().transform(decodeAvatar) })
export const introSchema = z.strictObject({ intro_id: uuid, event_id: uuid, author: profileSchema, text: boundedText(1, 1000), created_at: timestamp, state: z.literal('PUBLISHED'), revision })
export const pageSchema = z.strictObject({ items: z.array(introSchema).max(100), next_cursor: z.string().min(1).max(1024).regex(/^[A-Za-z0-9_-]+$/).nullable() })
export type PublicProfile = z.infer<typeof profileSchema>
export type PublicIntro = z.infer<typeof introSchema>
export type IntroPage = z.infer<typeof pageSchema>
export function freeze<T>(value: T): T { if (value && typeof value === 'object') { Object.values(value).forEach(freeze); Object.freeze(value) }; return value }
export function ordered(items: PublicIntro[]) { return items.every((row, index) => { const previous = items[index - 1]; return !previous || Date.parse(previous.created_at) > Date.parse(row.created_at) || Date.parse(previous.created_at) === Date.parse(row.created_at) && previous.intro_id > row.intro_id }) }
export function decodePage(value: unknown, scope: PrivacyScope): IntroPage {
  const page = pageSchema.parse(value)
  if (!ordered(page.items) || page.items.some(row => row.event_id !== scope.eventId) || new Set(page.items.map(row => row.intro_id)).size !== page.items.length) throw new Error('Invalid intro context')
  return freeze(page)
}
export function decodeProfile(value: unknown, id: string): PublicProfile { const profile = profileSchema.parse(value); if (profile.user_id !== id) throw new Error('Invalid profile context'); return freeze(profile) }
export const querySchema = z.string().trim().pipe(boundedText(0, 100)).refine(value => [...value].every(char => char.charCodeAt(0) >= 32 && char.charCodeAt(0) !== 127))
