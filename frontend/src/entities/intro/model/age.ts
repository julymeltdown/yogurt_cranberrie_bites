import { z } from 'zod'

export const ageBoundsSchema = z.union([
  z.strictObject({ minimum_age: z.null(), maximum_age_exclusive: z.null() }),
  z.strictObject({ minimum_age: z.number().int().min(14).max(100), maximum_age_exclusive: z.number().int().min(15).max(101) })
    .refine(value => value.minimum_age < value.maximum_age_exclusive, 'Maximum age must exceed minimum age.'),
])
export type AgeBounds = z.infer<typeof ageBoundsSchema>

/** Arithmetic only. A number is never evidence of verified age or access permission. */
export function matchesKnownAge(bounds: AgeBounds, age: number): boolean {
  const parsed = ageBoundsSchema.parse(bounds)
  return Number.isSafeInteger(age) && age >= 14 && age <= 100
    && (parsed.minimum_age === null || age >= parsed.minimum_age && age < parsed.maximum_age_exclusive)
}
/** Unknown age can match an unrestricted discovery condition; this does not authorize discovery. */
export function matchesDiscoveryAge(bounds: AgeBounds, age: number | null): boolean {
  const parsed = ageBoundsSchema.parse(bounds)
  return age === null ? parsed.minimum_age === null : matchesKnownAge(parsed, age)
}
/** Sharing requires a known age even without bounds; server verification/consent are still required. */
export function matchesSharingAge(bounds: AgeBounds, age: number | null): boolean {
  const parsed = ageBoundsSchema.parse(bounds)
  return age !== null && matchesKnownAge(parsed, age)
}
