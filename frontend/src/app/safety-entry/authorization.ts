import type { SessionController } from '../../features/auth/model/session'
import { decodeProfile } from '../../features/people/contract'
import type { ReportAuthorizationPort } from '../../features/safety/controller'
import { targetRequest } from '../../features/inbox/model/targets'
import type { PrivacyScope } from '../../shared/query/privacy'

/** C03 getConcertPublicProfile. A URL supplies a candidate, never authority. */
export async function authorizeUser(auth: SessionController, scope: PrivacyScope, id: string, signal: AbortSignal) {
  await auth.privacy.run(scope, async context => {
    const result = await auth.request({ path: `/api/concert/events/${scope.eventId}/people/${id}`, method: 'GET', access: 'protected', errorFamily: 'core', signal: context.signal,
      decode(value, response) { if (response.status !== 200) throw new Error('User unavailable'); return decodeProfile(value, id) },
    })
    if (result.scope.userId !== scope.userId || result.scope.eventId !== scope.eventId || result.scope.generation !== scope.generation) throw new Error('Changed user identity')
  }, signal)
}

export function reportAuthorization(auth: SessionController, now = Date.now): ReportAuthorizationPort {
  return { async check(target, evidenceMediaIds, scope, signal) {
    // There is no qualified REPORT attachment producer at this entry. A media
    // ID from a URL, another purpose or a stale cache is never accepted as proof.
    if (evidenceMediaIds.length) throw new Error('Report attachments unavailable')
    if (target.target_type === 'USER') await authorizeUser(auth, scope, target.target_id, signal)
    else if (target.target_type === 'POST' || target.target_type === 'PARTY' || target.target_type === 'GIVEAWAY') {
      const input = targetRequest(scope, { target_type: target.target_type, target_id: target.target_id })
      if (!input) throw new Error('Report target unavailable')
      await auth.privacy.run(scope, async context => {
        const result = await auth.request({ ...input, signal: context.signal })
        if (result.scope.userId !== scope.userId || result.scope.eventId !== scope.eventId || result.scope.generation !== scope.generation) throw new Error('Changed target identity')
      }, signal)
    } else throw new Error('No qualified target lookup')
    if (!auth.privacy.isCurrent(scope) || signal.aborted) throw new Error('Report lease withdrawn')
    return { scope, target, evidenceMediaIds: [], allowed: true, expiresAt: now() + 10_000 }
  } }
}
