import { QueryClient } from '@tanstack/react-query'
import { createMemorySession, type HttpClient, type HttpRequest } from '../../../../shared/api/http'
import { createQueryPrivacy } from '../../../../shared/query/privacy'
import { createEnrollment } from '../../model/enrollment'
export const revision = '11111111-1111-4111-8111-111111111111', attemptId = '22222222-2222-4222-8222-222222222222'
export const userId = '33333333-3333-4333-8333-333333333333'
export function policy() {
  const time = Date.now()
  return { contractVersion: '1.0', revision, documents: [{ id: 'test-terms', title: 'Synthetic test agreements',
    url: 'https://example.invalid/test-terms', sha256: 'a'.repeat(64), requiredAcknowledgementId: 'test-terms-v1' }],
  publishedAt: new Date(time - 1000).toISOString(), expiresAt: new Date(time + 600_000).toISOString(), serverTime: new Date(time).toISOString() }
}
export function attempt() { const { expiresAt, serverTime } = policy(); return { attemptId, policyRevision: revision, expiresAt, serverTime } }
export function accepted() { const { expiresAt, serverTime } = policy(); return { status: 'ACCEPTED', attemptId, expiresAt,
  resendAvailableAt: new Date(Date.parse(serverTime) + 2000).toISOString(), serverTime } }
export const admission = (decision = 'ADMITTED') => ({ decision, policyRevision: revision, acceptedRevision: decision === 'ADMITTED' ? revision : null, serverTime: new Date().toISOString() })
export const ack = ['test-terms-v1'], email = 'synthetic@example.invalid', password = '  synthetic password  '
export type Handler = (request: HttpRequest<unknown>) => { status: number; data: unknown } | Promise<{ status: number; data: unknown }>
export const defaultHandler: Handler = request => {
  const key = request.path.split('/').at(-1)
  if (key === 'policy') return { status: 200, data: policy() }
  if (key === 'attempts') return { status: 201, data: attempt() }
  if (key === 'admission' || key === 'consent') return { status: 200, data: admission() }
  if (key === 'verify') return { status: 200, data: { status: 'VERIFIED', attemptId, serverTime: new Date().toISOString() } }
  return { status: 202, data: accepted() }
}
export function fixture(handler: Handler = defaultHandler) {
  const session = createMemorySession(), client = new QueryClient(), privacy = createQueryPrivacy({ client, session })
  const requests: HttpRequest<unknown>[] = []
  let canStart = true, available = true
  const http: HttpClient = { async request<T>(request: HttpRequest<T>) {
    requests.push(request); const response = await handler(request)
    return { data: request.decode(response.data, { status: response.status, path: request.path, scope: session.read() }), scope: session.read() }
  }, refreshSession: async () => { throw new Error('No enrollment refresh') } }
  const enrollment = createEnrollment({ session, http, privacy, syntheticQualification: true, canStart: () => canStart, available: () => available })
  return { enrollment, session, privacy, client, requests, setCanStart(value: boolean) { canStart = value }, setAvailable(value: boolean) { available = value },
    dispose() { enrollment.dispose(); privacy.dispose(); client.clear() } }
}
