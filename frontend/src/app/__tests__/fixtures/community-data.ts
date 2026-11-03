import { actorId, eventId, giveaway, reservation } from '../../../features/gift-discovery/__tests__/fixtures/data'
import { activity, application, party, pendingReceipt, profile } from '../../../features/party-discovery/__tests__/fixtures/data'
import type { Application } from '../../../features/party-discovery/model/contract'

/** Stateful, synthetic server responses. No production handler imports this module. */
export function createCommunityData() {
  let current: Application | null = null
  let originalKey: string | null = null
  return {
    handle(request: Request) {
      const url = new URL(request.url), path = url.pathname
      if (request.method === 'POST' && path.endsWith('/applications')) {
        originalKey = request.headers.get('Idempotency-Key')!
        return Response.json(pendingReceipt(originalKey, { actor_id: actorId, event_id: eventId }), { status: 202 })
      }
      if (path.endsWith('/commands') && originalKey && url.searchParams.get('idempotency_key') === originalKey) {
        current = application({ event_id: eventId, applicant: profile(actorId) })
        return Response.json(pendingReceipt(originalKey, { actor_id: actorId, event_id: eventId, state: 'SUCCEEDED',
          resource_id: current.application_id, resource_revision: current.revision, result_status: 200 }))
      }
      if (request.method === 'DELETE' && current && path.endsWith(`/party-applications/${current.application_id}`)) {
        current = { ...current, state: 'CANCELLED', revision: current.revision + 1 }
        return Response.json(current)
      }
      if (path.endsWith('/me/parties')) return Response.json({ ...activity(), applications: { items: current ? [current] : [], next_cursor: null } })
      if (path.endsWith('/parties')) return Response.json({ items: [party({ event_id: eventId })], next_cursor: null })
      if (path.includes('/parties/')) return Response.json(party({ event_id: eventId }))
      if (path.endsWith('/me/giveaways')) return Response.json({
        giving: { items: [giveaway({ owner: { ...giveaway().owner, user_id: actorId }, state: 'DRAFT' })], next_cursor: null },
        receiving: { items: [reservation({ recipient_id: actorId })], next_cursor: null },
      })
      if (path.endsWith('/giveaways')) return Response.json({ items: [giveaway()], next_cursor: null })
      if (path.includes('/giveaways/')) return Response.json(giveaway())
      return Response.json({ code: 'FIXTURE', message: '', details: { reason: 'RESOURCE_UNAVAILABLE' } }, { status: 404 })
    },
  }
}
