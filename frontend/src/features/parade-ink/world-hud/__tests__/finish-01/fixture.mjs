// TEST ONLY: actual main/Auth/current service gate; no product provider or policy override.
import { policyFixtureRelease, servicePolicyFixture } from '/src/app/pre-world-policy/__tests__/fixtures'
import { authResponse } from '/src/features/auth/__tests__/fixtures/data'
await import('/src/features/parade-ink/world-hud/native-fixture.mjs')
if (!import.meta.env.DEV) throw Error('Development-only PI08 policy input fixture')
if (JSON.stringify(import.meta.env.VITE_MCR_APPLICATION_RELEASE) !== JSON.stringify(policyFixtureRelease)) throw Error('Exact synthetic catalog pair required')
const retained = globalThis.fetch, reads = []
const expectedPath = `/api/concert/events/${import.meta.env.VITE_MCR_EVENT_ID}/service-policy`
globalThis.fetch = async (input, init) => {
  const request = input instanceof Request ? new Request(input, init) : new Request(new URL(String(input), location.origin), init)
  const url = new URL(request.url)
  if (!url.pathname.endsWith('/service-policy')) return retained(request)
  const authorized = request.headers.get('Authorization') === 'Bearer ' + authResponse().accessToken
  const recognized = request.method === 'GET' && url.pathname === expectedPath && authorized
    && url.searchParams.size === 2 && url.searchParams.get('client_version') === policyFixtureRelease.applicationVersion
    && url.searchParams.get('client_release') === policyFixtureRelease.buildId
  reads.push({path: url.pathname, method: request.method, query: url.search, authorized,
              recognized, cache: request.cache, syntheticKind: recognized ? 'NORMAL' : 'UNAVAILABLE'})
  return Response.json(servicePolicyFixture(recognized ? 'NORMAL' : 'UNAVAILABLE', {ttl: 30000}))
}
Object.assign(window, {__PI08_POLICY__: {qualification: 'SYNTHETIC_CONTROLLED_HTTP_POLICY_INPUT_NOT_PRODUCTION_RECOGNITION',
  release: policyFixtureRelease, reads}})
