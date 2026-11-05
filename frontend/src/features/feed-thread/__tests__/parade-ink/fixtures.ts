import { setup as originalSetup, post, id } from '../../../event-feed/__tests__/fixtures'
import { enrollmentPaths } from '../../../auth/model/enrollment-contract'

export { post, id }
export const comment = { comment_id: id(71), post_id: post.post_id, author: post.author,
  text: 'A test reply', created_at: post.created_at, revision: 1 }

/** Canonical Auth still requests and decodes admission in this synthetic transport. */
export function setup(fetcher: typeof fetch) {
  return originalSetup(async (input, init) => {
    const request = input instanceof Request ? input : new Request(input, init)
    if (request.method === 'GET' && new URL(request.url).pathname === enrollmentPaths.admission
      && request.headers.get('Authorization') === 'Bearer test-only-token') {
      return Response.json({ decision: 'ADMITTED', policyRevision: 'aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa',
        acceptedRevision: 'aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa', serverTime: new Date().toISOString() })
    }
    return fetcher(input, init)
  })
}
