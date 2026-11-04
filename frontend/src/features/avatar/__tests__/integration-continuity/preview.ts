// Controlled producer extension; the retained fixture mounts the actual main application.
// No editor, Auth, renderer, controller, session or frame clock is replaced.
import { preset } from '../reviewed-production/fixtures'
await import('../../../../app/character-entry/__tests__/reviewed-production/default-entry/preview')
const retainedFetch = window.fetch.bind(window)
let remote: ReturnType<typeof window.__REVIEWED_ROOT_ENTRY__.state>['avatar'] | null = null
const requests: { method: string; path: string; expectedRevision?: number }[] = []
window.fetch = async (input, init) => {
  const request = input instanceof Request ? input : new Request(new URL(String(input), location.origin), init)
  const path = new URL(request.url).pathname
  if (remote && path.startsWith('/api/concert/events/') && path.endsWith('/me/avatar') && request.method === 'PUT') {
    const body = await request.clone().json()
    requests.push({ method: 'PUT', path, expectedRevision: body.expected_revision })
    if (body.expected_revision !== remote.revision) return Response.json({ code: 'CONFLICT', message: 'Changed elsewhere',
      details: { reason: 'VERSION_CONFLICT', current_revision: remote.revision } }, { status: 409 })
    throw Error('Continuity scenario must not submit another write after reconciliation')
  }
  const response = await retainedFetch(request)
  if (remote && path.startsWith('/api/concert/events/') && path.endsWith('/me') && request.method === 'GET' && response.ok) {
    const participant = await response.json(); participant.profile.avatar = structuredClone(remote)
    requests.push({ method: 'GET', path }); return Response.json(participant)
  }
  return response
}
Object.assign(window, { __AVATAR_CONTINUITY__: {
  changedElsewhere() { remote = { revision: 10, config: preset(4) }; return structuredClone(remote) },
  state: () => ({ remote: structuredClone(remote), requests: structuredClone(requests) }),
} })
