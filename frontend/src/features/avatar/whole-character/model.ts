import { z } from 'zod'
import type { PrivacyScope } from '../../../shared/query/privacy'
import { validateWholeCharacterConfig, sameWholeCharacterConfig, type WholeCharacterConfig } from '../../../entities/avatar/whole-character'

export type { WholeCharacterConfig }
const revision = z.number().int().positive().max(Number.MAX_SAFE_INTEGER)
const avatar = z.strictObject({ revision, config: z.unknown() })
const boundedText = (min: number, max: number) => z.string().refine(value => {
  const points = Array.from(value)
  return points.length >= min && points.length <= max && points.every(c => { const n = c.codePointAt(0)!; return n < 0xd800 || n > 0xdfff })
})
export interface WholeCharacterState { readonly revision: number; readonly config: WholeCharacterConfig }
/** Producer validates the actual published C06 branch; no parallel avatar wire schema. */
export const decodeWholeConfig = validateWholeCharacterConfig
export function decodeWholeState(value: unknown): WholeCharacterState {
  const parsed = avatar.parse(value)
  return Object.freeze({ revision: parsed.revision, config: decodeWholeConfig(parsed.config) })
}
export function decodeWholeParticipant(value: unknown, scope: PrivacyScope): WholeCharacterState {
  const parsed = z.strictObject({ event_id: z.literal(scope.eventId), user_id: z.literal(scope.userId), revision,
    profile: z.strictObject({ user_id: z.literal(scope.userId), display_name: boundedText(1, 40), bio: boundedText(0, 160).nullable(), avatar }),
    preferences: z.strictObject({ theme: z.enum(['REVENGE', 'PARADE', 'DANGER']), low_power: z.boolean(), reduced_motion: z.boolean(), sound_enabled: z.boolean() }),
    age_status: z.enum(['UNKNOWN', 'VERIFIED', 'EXPIRED', 'PROVIDER_UNAVAILABLE']),
  }).parse(value)
  return decodeWholeState(parsed.profile.avatar)
}
export const sameWholeConfig = sameWholeCharacterConfig
