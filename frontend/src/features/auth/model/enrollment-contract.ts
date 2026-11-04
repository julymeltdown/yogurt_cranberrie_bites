import { z } from 'zod'
import type { Decoder } from '../../../shared/api/http'
import { validResetPassword } from './recovery-contract'

/** Public wire source: backend207 ed50c2f1, registered core/v1 Enrollment* DTOs. */
export const enrollmentContractRevision = 'ed50c2f1'
export const enrollmentPaths = {
  policy: '/api/auth/enrollment/policy', attempts: '/api/auth/enrollment/attempts',
  signup: '/api/auth/enrollment/signup', verify: '/api/auth/enrollment/verify',
  resend: '/api/auth/enrollment/resend', admission: '/api/auth/enrollment/admission', consent: '/api/auth/enrollment/consent',
} as const
export const enrollmentUuid = z.string().regex(/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/)
  .refine(value => value !== '00000000-0000-0000-0000-000000000000')
const timestamp = z.iso.datetime().refine(value => Number.isFinite(Date.parse(value)))
const identifier = z.string().min(1).max(80).regex(/^[A-Za-z0-9_.-]+$/)
const document = z.strictObject({ id: identifier, title: z.string().min(1).max(240)
  .refine(value => value.trim().length > 0 && new TextEncoder().encode(value).length <= 240),
  url: z.url().max(2048).refine(value => value.startsWith('https://')),
  sha256: z.string().regex(/^[0-9a-f]{64}$/), requiredAcknowledgementId: identifier })
export const policySchema = z.strictObject({ contractVersion: z.literal('1.0'), revision: enrollmentUuid,
  documents: z.array(document).min(1).max(8), publishedAt: timestamp, expiresAt: timestamp, serverTime: timestamp })
  .refine(value => Date.parse(value.publishedAt) <= Date.parse(value.serverTime)
    && Date.parse(value.expiresAt) > Date.parse(value.serverTime)
    && new Set(value.documents.map(item => item.id)).size === value.documents.length
    && new Set(value.documents.map(item => item.requiredAcknowledgementId)).size === value.documents.length)
export const attemptSchema = z.strictObject({ attemptId: enrollmentUuid, policyRevision: enrollmentUuid,
  expiresAt: timestamp, serverTime: timestamp })
  .refine(value => Date.parse(value.expiresAt) > Date.parse(value.serverTime)
    && Date.parse(value.expiresAt) - Date.parse(value.serverTime) <= 30 * 60_000)
export const acceptedSchema = z.strictObject({ status: z.literal('ACCEPTED'), attemptId: enrollmentUuid,
  expiresAt: timestamp, resendAvailableAt: timestamp, serverTime: timestamp })
  .refine(value => Date.parse(value.expiresAt) > Date.parse(value.serverTime)
    && Date.parse(value.expiresAt) - Date.parse(value.serverTime) <= 30 * 60_000)
export const verifiedSchema = z.strictObject({ status: z.literal('VERIFIED'), attemptId: enrollmentUuid, serverTime: timestamp })
export const admissionSchema = z.strictObject({ decision: z.enum(['ADMITTED', 'CONSENT_REQUIRED', 'POLICY_UNAVAILABLE', 'ACCOUNT_INACTIVE']),
  policyRevision: enrollmentUuid.nullable(), acceptedRevision: enrollmentUuid.nullable(), serverTime: timestamp })
  .refine(value => value.decision !== 'ADMITTED' || value.policyRevision !== null && value.policyRevision === value.acceptedRevision)
export type EnrollmentPolicy = z.infer<typeof policySchema>
export type EnrollmentAttempt = z.infer<typeof attemptSchema>
export type EnrollmentAccepted = z.infer<typeof acceptedSchema>
export type EnrollmentAdmission = z.infer<typeof admissionSchema>
function decode<T>(schema: z.ZodType<T>, status: number): Decoder<T> {
  return (value, context) => {
    if (context.status !== status) throw new Error('Invalid enrollment status')
    return schema.parse(value)
  }
}
export const decodePolicy = decode(policySchema, 200), decodeAttempt = decode(attemptSchema, 201)
export const decodeAccepted = decode(acceptedSchema, 202), decodeVerified = decode(verifiedSchema, 200)
export const decodeAdmission = decode(admissionSchema, 200)
export const validEnrollmentPassword = validResetPassword
export const validEnrollmentEmail = (email: string) => email.length <= 254 && z.email().safeParse(email).success
export const validVerificationCode = (code: string) => /^[0-9]{6}$/.test(code)
/** Exact sets; neither a checked boolean nor a synthetic age claim establishes acceptance. */
export function exactAcknowledgements(policy: EnrollmentPolicy, ids: readonly string[]) {
  const required = policy.documents.map(item => item.requiredAcknowledgementId)
  return ids.length === required.length && new Set(ids).size === ids.length && required.every(id => ids.includes(id))
}
