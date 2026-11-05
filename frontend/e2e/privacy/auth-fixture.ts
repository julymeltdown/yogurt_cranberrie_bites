import { createSessionController } from '../../src/features/auth/model/session'
import { admission } from '../../src/features/auth/__tests__/enrollment-consent/fixtures'
import { isolatedCookiePolicy } from '../../src/features/auth/__tests__/fixtures/data'

type Options = Omit<Parameters<typeof createSessionController>[0], 'fetch' | 'cookiePolicy'> & { fetch: typeof globalThis.fetch; admissionDecision?: () => Parameters<typeof admission>[0] }

/** Synthetic transport only; current Auth still decodes and applies the owner's admission receipt. */
export function createFixtureSessionController(options: Options) {
  const { fetch: featureFetch, admissionDecision, ...sessionOptions } = options
  return createSessionController({ ...sessionOptions, cookiePolicy: isolatedCookiePolicy(), fetch: async (input, init) => {
    const request = input as Request
    if (request.method === 'GET' && new URL(request.url).pathname === '/api/auth/enrollment/admission') {
      return Response.json(admission(admissionDecision?.()), { headers: { 'Cache-Control': 'no-store' } })
    }
    return featureFetch(input, init)
  } })
}
