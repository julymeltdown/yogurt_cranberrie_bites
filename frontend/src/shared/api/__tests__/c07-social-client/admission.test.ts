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
