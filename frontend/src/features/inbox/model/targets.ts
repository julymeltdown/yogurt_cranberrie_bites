import { parse } from '../../event-feed/contract'
import type { HttpRequest } from '../../../shared/api/http'
import type { DomainOperationId } from '../../../shared/api/domain-operations.generated'
import { decoder, prefix, uuid, type NotificationTarget } from './contract'
import type { PrivacyScope } from '../../../shared/query/privacy'
export interface InboxDestination { readonly path: string; readonly scope: PrivacyScope }
type TargetRow = { event_id: string; [key: string]: unknown }
/** Only published shell paths. REPORT has no participant detail route and stays unavailable. */
export function targetRequest(scope: PrivacyScope, target: NotificationTarget): Omit<HttpRequest<string>, 'refresh'> | null {
  if (!uuid(target.target_id)) return null
  const id = target.target_id, base = prefix(scope)
  let path: string, operation: DomainOperationId, schema: string
  switch (target.target_type) {
    case 'PARTY': path = `${base}/parties/${id}`; operation = 'getConcertParty'; schema = 'Domain_party_PartyPublic'; break
    case 'APPLICATION': path = `${base}/party-applications/${id}`; operation = 'getConcertApplication'; schema = 'Domain_party_Application'; break
    case 'GIVEAWAY': path = `${base}/giveaways/${id}`; operation = 'getConcertGiveaway'; schema = 'Domain_giveaway_GiveawayPublic'; break
    case 'RESERVATION': path = `${base}/reservations/${id}`; operation = 'getConcertReservation'; schema = 'Domain_giveaway_Reservation'; break
    case 'SOCIAL_REQUEST': path = `${base}/social-requests/${id}`; operation = 'getConcertSocialRequest'; schema = 'Domain_social_SocialRequest'; break
    case 'POST': path = `${base}/posts/${id}`; operation = 'getConcertPost'; schema = 'Domain_feed_PostDetail'; break
    case 'NOTICE': path = `${base}/notices/${id}`; operation = 'getConcertNotice'; schema = 'Domain_feed_PublishedNotice'; break
    default: return null
  }
  return { path, method: 'GET', access: 'protected', errorFamily: 'domain', domainOperation: operation,
    decode: decoder(scope, path, (value, status) => {
      if (status !== 200) throw new Error('Invalid target status')
      const result = parse<TargetRow>(schema, value)
      const row = target.target_type === 'POST' ? result.post as TargetRow : result
      if (row.event_id !== scope.eventId) throw new Error('Wrong target event')
      const keys = { PARTY: 'party_id', APPLICATION: 'application_id', GIVEAWAY: 'giveaway_id', RESERVATION: 'reservation_id', SOCIAL_REQUEST: 'request_id', POST: 'post_id', NOTICE: 'notice_id', REPORT: 'report_id' }
      if (row[keys[target.target_type]] !== id) throw new Error('Wrong target ID')
      switch (target.target_type) {
        case 'PARTY': return `/meet/parties/${id}`
        case 'APPLICATION': if (!uuid(row.party_id)) break; return `/meet/parties/${row.party_id}`
        case 'GIVEAWAY': if (row.state === 'DRAFT') break; return `/gifts/${id}`
        case 'RESERVATION': if (row.recipient_id !== scope.userId) break; return `/gifts/pickups/${id}`
        case 'SOCIAL_REQUEST': if (row.requester_id !== scope.userId && row.recipient_id !== scope.userId) break; return '/me/requests'
        case 'POST': return `/feed/${id}`
        case 'NOTICE': return `/notices/${id}`
      }
      throw new Error('Unavailable target')
    }),
  }
}
