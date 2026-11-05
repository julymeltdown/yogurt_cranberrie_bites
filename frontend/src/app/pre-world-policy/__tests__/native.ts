// TEST ONLY: borrow the retained actual-main Auth/C03/C05/selected-art fixture.
// Wrap its in-memory fetch before UI login, rather than ineffective page.route.
import { policyFixtureRelease, servicePolicyFixture } from './fixtures'

type Scenario = { kind: 'NORMAL' | 'MAINTENANCE' | 'CLIENT_VERSION_REQUIRED' | 'UNAVAILABLE' | 'HTTP503' | 'HTTP404'; ttl?: number; delay?: number; bootstrapDelay?: number; bootstrapStatus?: 401 | 403 }
const scenario: Scenario = { ...window.__MCR_POLICY_CASE__ }
await import('../../social-composition/__tests__/sharing-safety-default/native.tsx')
const retained = globalThis.fetch
const reads: { path: string; query: string; method: string; authorized: boolean; cache: RequestCache }[] = []
let bootstrapDispatches = 0
const delay = (ms: number) => new Promise(resolve => setTimeout(resolve, ms))
globalThis.fetch = async (input, init) => {
  const request = input instanceof Request ? new Request(input, init) : new Request(new URL(String(input), location.origin), init)
  const url = new URL(request.url)
  if (url.pathname.endsWith('/service-policy')) {
    reads.push({ path: url.pathname, query: url.search, method: request.method,
      authorized: /^Bearer .+/.test(request.headers.get('Authorization') ?? ''), cache: request.cache })
    const selected = { ...scenario }
    const wire = servicePolicyFixture(selected.kind === 'HTTP503' || selected.kind === 'HTTP404' ? 'UNAVAILABLE' : selected.kind, { ttl: selected.ttl ?? 5_000 })
    if (selected.delay) await delay(selected.delay) // Deliberately ignored abort proves stale completion fencing.
    if (selected.kind === 'HTTP503') return Response.json({}, { status: 503 })
    if (selected.kind === 'HTTP404') return Response.json({}, { status: 404 })
    const recognized = request.method === 'GET' && request.headers.has('Authorization')
      && url.searchParams.size === 2 && url.searchParams.get('client_version') === policyFixtureRelease.applicationVersion
      && url.searchParams.get('client_release') === policyFixtureRelease.buildId
    return Response.json(recognized ? wire : servicePolicyFixture('UNAVAILABLE'))
  }
  if (url.pathname.endsWith('/bootstrap')) {
    bootstrapDispatches++
    if (scenario.bootstrapDelay) await delay(scenario.bootstrapDelay)
    if (scenario.bootstrapStatus) return Response.json({}, { status: scenario.bootstrapStatus })
  }
  // Request construction can transfer a POST body; forward that request once.
  return retained(request)
}
const fixture = { mode(next: Scenario) { Object.assign(scenario, { delay: 0, bootstrapDelay: 0, ttl: 5_000 }, next) },
  state: () => ({ reads: [...reads], bootstrapDispatches, retained: window.__SHARING_SAFETY__.state() }) }
Object.assign(window, { __PRE_WORLD_POLICY__: fixture })
declare global { interface Window { __MCR_POLICY_CASE__: Scenario; __PRE_WORLD_POLICY__: typeof fixture } }
