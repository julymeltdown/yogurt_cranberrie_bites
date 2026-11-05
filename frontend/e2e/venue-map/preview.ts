// TEST ONLY: unchanged actual-main reviewed character/HTTP/C05 fixture.
import { admission } from '../../src/features/auth/__tests__/enrollment-consent/fixtures'
import { authResponse } from '../../src/features/auth/__tests__/fixtures/data'
await import('../../src/app/character-entry/__tests__/reviewed-production/default-entry/preview')
const fallback = window.fetch.bind(window)
window.fetch = async (input, init) => {
  const request = input instanceof Request ? input : new Request(new URL(String(input), location.origin), init)
  if (new URL(request.url).pathname !== '/api/auth/enrollment/admission') return fallback(input, init)
  if (request.method !== 'GET' || request.headers.get('Authorization') !== 'Bearer ' + authResponse().accessToken) throw Error('Canonical protected admission required')
  return Response.json(admission())
}
