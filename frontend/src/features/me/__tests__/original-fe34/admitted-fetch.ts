import { enrollmentPaths } from '../../../auth/model/enrollment-contract'

/** Controlled Me fixture transport, not a production admission or consent bypass.
 * Canonical Auth still requests and strictly decodes the admission response.
 * The supplied feature fetcher observes only its original application requests. */
export function withAdmittedMeFixture(fetcher: typeof fetch): typeof fetch {
  return async (input, init) => {
    const request = input instanceof Request ? input : new Request(input, init)
    if (request.method === 'GET' && new URL(request.url).pathname === enrollmentPaths.admission) {
      return Response.json({ decision: 'ADMITTED', policyRevision: 'aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa',
        acceptedRevision: 'aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa', serverTime: new Date().toISOString() })
    }
    return fetcher(input, init)
  }
}
