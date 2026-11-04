import { expect, it } from 'vitest'
import { admitHTTP } from '../../../../../e2e/real/social/admission.mjs'
it('missing runtime input names exact owners without reporting a real pass', () => {
  const result = admitHTTP(undefined)
  expect(result.status).toBe('NOT_RUN'); expect(result.IN03).toBe('NOT_RUN')
  expect(result.missing.length).toBeGreaterThan(8)
  expect(result.unavailable.some(row => row.input.includes('WS'))).toBe(true)
})
it('fixture principals, token injection and refresh qualification cannot be admitted', () => {
  const result = admitHTTP({ mode: 'fixture', origin: 'file:///fixture', eventId: '00000000-0000-4000-8000-000000000001',
    availabilityModule: '/src/app/__tests__/fixture.ts', qualifiedRefresh: true,
    principals: { requester: { email: 'fixture@example.invalid', password: 'DO_NOT_PRINT', accessToken: 'DO_NOT_PRINT' } } })
  expect(result.status).toBe('NOT_RUN'); expect(JSON.stringify(result)).not.toContain('DO_NOT_PRINT')
})
it.each([null, 'invalid', { principals: { requester: 1, recipient: 'bad', restricted: [] } }])('malformed configuration still produces redacted NOT_RUN diagnostics: %j', input => {
  const result = admitHTTP(input)
  expect(result.status).toBe('NOT_RUN'); expect(result.missing.length).toBeGreaterThan(0)
})
it('well-shaped input means INPUTS_PRESENT, never runtime or full IN03 qualification', () => {
  const uuid = (tail: number) => `b0000000-0000-4000-8000-${String(tail).padStart(12, '0')}`
  const result = admitHTTP({ mode: 'real', origin: 'https://mcr.internal', eventId: uuid(1), availabilityModule: '/src/app/social-production.ts', availabilityRevision: 'a'.repeat(40),
    principals: Object.fromEntries(['requester', 'recipient', 'restricted'].map((role, index) => [role, { userId: uuid(index + 2), email: `${role}@mcr.internal`, password: 'shape-test-only' }])),
    resources: { introId: uuid(6), postId: uuid(7), approvedRequestId: uuid(8) }, qualifiedRefresh: false })
  expect(result.status).toBe('INPUTS_PRESENT'); expect(result.IN03).toBe('NOT_RUN'); expect(result.unavailable).toHaveLength(2)
})
