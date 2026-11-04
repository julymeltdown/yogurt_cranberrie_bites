import { createSessionController } from '../../../features/auth/model/session'
import { decodeBootstrap } from '../../../features/auth/model/bootstrap'
import { createAuthorityTimeline } from '../../community-authority'
import { beginCommunityBootstrap, type CommunityClockProfile } from '../bootstrap'
import { createCommunityOwner } from '../owner'
import { CAPABILITIES, projectCapabilities } from '../../capabilities'
import { checkedBootstrap, physicalBootstrap, eventId, userId } from '../../community-authority/__tests__/fixtures'
import type { ShellBootstrap } from '../../ports'

export const testProfile: CommunityClockProfile = Object.freeze({ classification: 'TEST_ONLY', evidence: 'Synthetic boundary test, not platform measurement',
  maxAgeMs: 30000, uncertaintyMs: 1000, maxElapsedRate: 1.01, maxObservationGapMs: 5000 })
export async function bootstrap(allCapabilities = false, physical = false) {
  const source = physical ? await physicalBootstrap() : await checkedBootstrap()
  if (!allCapabilities) return source
  const document = { ...structuredClone(source.document!), capabilities: projectCapabilities(Object.fromEntries(CAPABILITIES.map(key => [key, { available: true, reason: null }]))) }
  return decodeBootstrap(document, { userId, eventId, generation: 1 })
}
export function fixture(source: ShellBootstrap, profile: CommunityClockProfile | undefined, mode?: 'invalid' | 'held') {
  const requests: Request[] = []
  let ms = 100
  let releaseResponse = () => {}
  const auth = createSessionController({ eventId, cookiePolicy: { available: true, onInvalidate: () => () => {}, run: (_key, work) => work(), dispose() {} },
    fetch: async input => {
      const request = input as Request
      if (request.method === 'GET' && new URL(request.url).pathname === '/api/auth/enrollment/admission') {
        // SYNTHETIC TEST fixture for canonical Auth's authenticated admission read.
        // Preserve requests as the original C03/product request accounting.
        const revision = '00000000-0000-4000-8000-000000000701'
        return Response.json({ decision: 'ADMITTED', policyRevision: revision, acceptedRevision: revision, serverTime: source.document?.server_time })
      }
      requests.push(request.clone())
      if (new URL(request.url).pathname.endsWith('/bootstrap')) {
        if (mode === 'invalid') return Response.json({ invalid: true })
        if (mode === 'held') return new Promise<Response>(resolve => { releaseResponse = () => { ms = 600; resolve(Response.json(source.document)) } })
        ms = 600; return Response.json(source.document)
      }
      return new Response(null, { status: 503 })
    } })
  auth.session.replace(0, { userId, eventId, accessToken: 'SYNTHETIC_ONLY' })
  const scope = auth.privacy.capture(), lifetime = new AbortController()
  let timelineCalls = 0
  const timing = beginCommunityBootstrap({ privacy: auth.privacy, scope, signal: lifetime.signal, profile }, (gap, signal) => {
    timelineCalls++
    return createAuthorityTimeline({ maxObservationGapMs: gap, observe: () => ({ nowMs: ms, origin: 1 }),
      onInterruption(invalidate) { signal.addEventListener('abort', invalidate); return () => signal.removeEventListener('abort', invalidate) } })
  })
  async function load() {
    try {
      const value = await auth.privacy.run(scope, ({ signal }) => auth.loadBootstrap({ userId, eventId, generation: scope.generation }, signal), lifetime.signal)
      return timing.complete(value)
    } catch (error) { timing.dispose(); throw error }
  }
  return { auth, scope, lifetime, requests, timing, load, timelineCalls: () => timelineCalls, releaseResponse: () => releaseResponse(),
    owner: (binding: Awaited<ReturnType<typeof load>>) => createCommunityOwner({ auth, bootstrap: binding.source, scope, binding }),
    setTime(value: number) { ms = value }, dispose() { timing.dispose(); auth.dispose() } }
}
