import { z } from 'zod'
import { decodeAvatarState } from '../../../entities/avatar/whole-admission'
import { canonicalUuid } from '../../auth/model/contract'
import type { SessionScope } from '../../../shared/api/http'

const integer = z.number().int().safe().min(1)
const uuid = z.string().refine(canonicalUuid)
const text = (min: number, max: number) => z.string().refine(s => Array.from(s).length >= min && Array.from(s).length <= max
  && Array.from(s).every(c => { const n = c.codePointAt(0)!; return n < 0xd800 || n > 0xdfff }))
export const preferencesSchema = z.strictObject({ theme: z.enum(['REVENGE', 'PARADE', 'DANGER']),
  low_power: z.boolean(), reduced_motion: z.boolean(), sound_enabled: z.boolean() })
export type Preferences = z.infer<typeof preferencesSchema>
const participantSchema = z.strictObject({ event_id: uuid, user_id: uuid, revision: integer,
  profile: z.strictObject({ user_id: uuid, display_name: text(1, 40), bio: text(0, 160).nullable(),
    avatar: z.unknown().transform(decodeAvatarState) }),
  preferences: preferencesSchema, age_status: z.enum(['UNKNOWN', 'VERIFIED', 'EXPIRED', 'PROVIDER_UNAVAILABLE']) })
export type Participant = z.infer<typeof participantSchema>
export const profileSchema = z.strictObject({ display_name: text(1, 40), bio: text(0, 160).nullable() })
export type ProfileDraft = z.infer<typeof profileSchema>
export type Draft = ProfileDraft & { preferences: Preferences }
export const defaults: Readonly<Preferences> = Object.freeze({ theme: 'REVENGE', low_power: false, reduced_motion: false, sound_enabled: false })
export function freeze<T>(value: T): T {
  if (value && typeof value === 'object') { Object.values(value).forEach(freeze); Object.freeze(value) }
  return value
}
export function decodeParticipant(value: unknown, scope: SessionScope): Participant {
  const p = participantSchema.parse(value)
  if (p.event_id !== scope.eventId || p.user_id !== scope.userId || p.profile.user_id !== scope.userId) throw new Error('Profile identity mismatch')
  return freeze(p)
}
export const draftOf = (p: Participant): Draft => freeze({ display_name: p.profile.display_name, bio: p.profile.bio, preferences: { ...p.preferences } })
