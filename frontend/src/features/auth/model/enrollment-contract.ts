import { z } from 'zod'
import contract from '../../../../../contracts/generated/mcr-core.openapi.json'
import type { Decoder } from '../../../shared/api/http'
import { canonicalUuid, object } from './contract'
import { validResetPassword } from './recovery-contract'
export const enrollmentPaths = { signup: '/api/auth/email/signup', verify: '/api/auth/email/verify', resend: '/api/auth/email/resend' } as const
const schemas = contract.components.schemas
if (schemas.SignupResponse.properties.status.type !== 'string' || 'enum' in schemas.SignupResponse.properties.status
  || schemas.ResendVerificationResponse.properties.status.enum.join() !== 'ACCEPTED'
  || !('202' in contract.paths['/api/auth/email/resend'].post.responses)
  || schemas.EmailSignupRequest.properties.password['x-mcr-utf8-bytes'].minimum !== 8
  || schemas.EmailSignupRequest.properties.password['x-mcr-utf8-bytes'].maximum !== 128) throw new Error('Enrollment contract changed')
export const validEnrollmentPassword = validResetPassword
export function validEnrollmentEmail(email: string) {
  const parts = email.split('@')
  return email.length <= 254 && z.email().safeParse(email).success && parts.length === 2 && parts[0]!.length > 1 && parts[1]!.includes('.')
}
export const validVerificationCode = (code: string) => /^[0-9]{6}$/.test(code)
/** SignupResponse.status is a string, not an enum. Unknown outcomes never imply completion. */
export const decodeEnrollment: Decoder<{ status: string }> = (value, context) => {
  const result = object(value)
  if (context.status !== 200 || !canonicalUuid(result.userId) || typeof result.status !== 'string') throw new Error('Invalid enrollment response')
  return { status: result.status }
}
export const decodeResend: Decoder<undefined> = (value, context) => {
  const result = object(value)
  if (context.status !== 202 || result.status !== 'ACCEPTED' || Object.keys(result).length !== 1) throw new Error('Invalid resend response')
  return undefined
}
export interface EnrollmentAcknowledgements { eligibility: boolean; consent: boolean }
