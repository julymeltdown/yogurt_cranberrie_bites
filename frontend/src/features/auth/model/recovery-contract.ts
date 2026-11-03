import { z } from 'zod'
import contract from '../../../../../contracts/generated/mcr-core.openapi.json'
import { object, requireValue } from './contract'
import type { DecodeContext } from '../../../shared/api/http'

export const resetPaths = { request: '/api/auth/password/reset/request', confirm: '/api/auth/password/reset/confirm' } as const
// Consume the frozen public artifact; this is not a second wire-schema authority.
const schemas = contract.components.schemas
requireValue(contract.paths[resetPaths.request].post.operationId === 'requestPasswordReset'
  && contract.paths[resetPaths.confirm].post.operationId === 'confirmPasswordReset'
  && schemas.PasswordResetResponse.properties.status.type === 'string'
  && !('enum' in schemas.PasswordResetResponse.properties.status)
  && schemas.PasswordResetConfirmRequest.properties.newPassword['x-mcr-utf8-bytes'].minimum === 8
  && schemas.PasswordResetConfirmRequest.properties.newPassword['x-mcr-utf8-bytes'].maximum === 128)
const wellFormed = (value: string) => Array.from(value).every(c => { const n = c.codePointAt(0)!; return n < 0xd800 || n > 0xdfff })
export const validRecoveryEmail = (value: string) => value.length <= 254 && z.email().safeParse(value).success
export const validResetPassword = (value: string) => wellFormed(value)
  && new TextEncoder().encode(value).length >= 8 && new TextEncoder().encode(value).length <= 128
// Current source tokens use unpadded base64url. The length cap is a local resource bound.
export const validResetToken = (value: string) => value.length >= 1 && value.length <= 8192 && !/[^A-Za-z0-9_-]/.test(value)
export function decodeResetResponse(value: unknown, context: DecodeContext): undefined {
  const result = object(value)
  requireValue(context.status === 200 && Object.hasOwn(result, 'status') && typeof result.status === 'string')
  // Status is deliberately NOT an enum, delivery evidence, or product copy. Discard it.
  return undefined
}
