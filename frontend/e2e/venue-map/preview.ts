// TEST ONLY: unchanged actual-main reviewed character/HTTP/C05 fixture.
import { admission } from '../../src/features/auth/__tests__/enrollment-consent/fixtures'
import { authResponse } from '../../src/features/auth/__tests__/fixtures/data'
import { policyFixtureRelease, servicePolicyFixture } from '../../src/app/pre-world-policy/__tests__/fixtures'
await import('../../src/app/character-entry/__tests__/reviewed-production/default-entry/preview')
const fallback = window.fetch.bind(window)
window.fetch = async (input, init) => {
  const request = input instanceof Request ? new Request(input, init) : new Request(new URL(String(input), location.origin), init)
  const url = new URL(request.url)
  if (url.pathname === '/api/concert/events/00000000-0000-4000-8000-000000000201/service-policy') {
    const recognized = request.method === 'GET' && request.headers.get('Authorization') === 'Bearer ' + authResponse().accessToken
      && url.searchParams.size === 2 && url.searchParams.get('client_version') === policyFixtureRelease.applicationVersion
      && url.searchParams.get('client_release') === policyFixtureRelease.buildId
    // TEST ONLY catalog pair and normal decision through the actual decoder;
    // this grants no production release, live-service or policy qualification.
    return Response.json(servicePolicyFixture(recognized ? 'NORMAL' : 'UNAVAILABLE', { ttl: 30_000 }))
  }
  if (url.pathname !== '/api/auth/enrollment/admission') return fallback(request)
  if (request.method !== 'GET' || request.headers.get('Authorization') !== 'Bearer ' + authResponse().accessToken) throw Error('Canonical protected admission required')
  return Response.json(admission())
}
