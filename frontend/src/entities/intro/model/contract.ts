import { z } from 'zod'
import { boundedText } from '../../../shared/validation/text'
import { schemaValid } from '../../avatar/runtime/schema'
import { ageBoundsSchema } from './age'

// C04 canonical UUID spelling, without legacy prefixed identities or provider-specific IDs.
export const canonicalUuidSchema = z.string().regex(/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/)
export const utcTimestampSchema = z.iso.datetime().regex(/Z$/)
const revision = z.number().int().min(1).max(Number.MAX_SAFE_INTEGER)
export const introDraftSchema = z.strictObject({ text: boundedText(0, 1000), age_bounds: ageBoundsSchema, expires_at: utcTimestampSchema.nullable() })
export const setIntroSchema = introDraftSchema.extend({ expected_revision: z.number().int().min(0).max(Number.MAX_SAFE_INTEGER) })
export const privateIntroSchema = introDraftSchema.extend({
  intro_id: canonicalUuidSchema, event_id: canonicalUuidSchema, owner_id: canonicalUuidSchema,
  created_at: utcTimestampSchema, state: z.enum(['DRAFT', 'PUBLISHED', 'CLOSED']), revision,
})
const publicProfileSchema = z.strictObject({
  user_id: canonicalUuidSchema, display_name: boundedText(1, 40), bio: boundedText(0, 160).nullable(),
  avatar: z.strictObject({ revision: z.number().int().min(1), config: z.unknown().refine(value => schemaValid(value, 'config'), 'Invalid C06 avatar config.') }),
})
export const publicIntroSchema = z.strictObject({
  intro_id: canonicalUuidSchema, event_id: canonicalUuidSchema, author: publicProfileSchema,
  text: boundedText(1, 1000), created_at: utcTimestampSchema, state: z.literal('PUBLISHED'), revision,
})
/** Validates a server DTO shape. Parsing VERIFIED does not establish verification or authorization. */
export const ageStatusSchema = z.strictObject({
  user_id: canonicalUuidSchema, event_id: canonicalUuidSchema,
  status: z.enum(['UNKNOWN', 'VERIFIED', 'EXPIRED', 'UNSUPPORTED', 'PROVIDER_UNAVAILABLE', 'REVOKED']),
  verified_at: utcTimestampSchema.nullable(), expires_at: utcTimestampSchema.nullable(), verification_available: z.boolean(),
})
export type IntroDraft = z.infer<typeof introDraftSchema>
