import { z } from 'zod'
import { boundedText } from '../../shared/validation/text'
import { freeze, revision, timestamp, uuid } from '../people/contract'
import type { PrivacyScope } from '../../shared/query/privacy'
export const boundsSchema = z.union([
  z.strictObject({ minimum_age: z.null(), maximum_age_exclusive: z.null() }),
  z.strictObject({ minimum_age: z.number().int().min(14).max(100), maximum_age_exclusive: z.number().int().min(15).max(101) }).refine(v => v.minimum_age < v.maximum_age_exclusive),
])
export type AgeBounds = z.infer<typeof boundsSchema>
/** Form boundary helper only. Never authorizes discovery or computes a verified claim. */
export function withinBounds(bounds: AgeBounds, age: number) { const b = boundsSchema.parse(bounds); return Number.isSafeInteger(age) && age >= 14 && age <= 100 && (b.minimum_age === null || (age >= b.minimum_age && age < b.maximum_age_exclusive)) }
export const draftSchema = z.strictObject({ text: boundedText(0, 1000), age_bounds: boundsSchema, expires_at: timestamp.nullable() })
export type IntroDraft = z.infer<typeof draftSchema>
export const emptyDraft = (): IntroDraft => ({ text: '', age_bounds: { minimum_age: null, maximum_age_exclusive: null }, expires_at: null })
const ownSchema = z.strictObject({ intro_id: uuid, event_id: uuid, owner_id: uuid, text: boundedText(0, 1000), age_bounds: boundsSchema, expires_at: timestamp.nullable(), created_at: timestamp, state: z.enum(['DRAFT', 'PUBLISHED', 'CLOSED']), revision })
export type OwnIntro = z.infer<typeof ownSchema>
export function decodeOwn(value: unknown, scope: PrivacyScope): OwnIntro {
  const row = ownSchema.parse(value)
  if (row.event_id !== scope.eventId || row.owner_id !== scope.userId || (row.state === 'PUBLISHED' && row.text.trim() === '')) throw new Error('Invalid owner intro')
  return freeze(row)
}
export const fromIntro = (row: OwnIntro): IntroDraft => freeze({ text: row.text, age_bounds: row.age_bounds, expires_at: row.expires_at })
export const sameDraft = (a: IntroDraft, b: IntroDraft) => a.text === b.text && a.expires_at === b.expires_at && a.age_bounds.minimum_age === b.age_bounds.minimum_age && a.age_bounds.maximum_age_exclusive === b.age_bounds.maximum_age_exclusive
