import type { openRealIdentity } from './real-entry'
import { waitForController } from './real-entry'
import { decode, reveal, socialRequest, type Platform } from '../../../src/features/social-sharing/model/contract'
import { TransportError } from '../../../src/shared/api/errors'

type Identity = Awaited<ReturnType<typeof openRealIdentity>>
type BoundSocial = ReturnType<Identity['bindSocial']>
function requireResult(value: unknown, message: string): asserts value { if (!value) throw new Error(message) }
const pause = () => new Promise<void>(resolve => setTimeout(resolve, 250))

/** Poll the original command receipt. Never issue a replacement mutation/key. */
async function confirmMutation(read: () => { phase: string; key: string | null }, check: () => Promise<unknown>) {
  const key = read().key
  requireResult(key, 'IN03 command did not allocate an idempotency key')
  for (let attempt = 0; attempt < 12; attempt++) {
    const current = read()
    requireResult(current.key === key, 'IN03 command identity changed while confirming')
    if (current.phase === 'succeeded') return
    requireResult(['pending', 'uncertain'].includes(current.phase), 'IN03 command was not confirmed')
    await pause(); await check()
  }
  throw new Error('IN03 original command remains unresolved; no replacement was sent')
}

/** Uses only canonical production controllers and the actor's existing real HTTP client. */
export function createPublicJourney(identity: Identity, social: BoundSocial) {
  const { owner, sharing } = social
  const checkSharing = () => confirmMutation(() => sharing.read().mutation, sharing.checkStatus)
  function requireService(service: Parameters<typeof owner.available>[0]) {
    requireResult(owner.available(service), `NOT_RUN: production ${service} producer unavailable`)
  }
  return {
    async discover(introId: string, authorId: string) {
      requireService('people')
      owner.people.start()
      await waitForController(() => owner.people.read().page.phase, owner.people.subscribe)
      for (let page = 0; page < 10; page++) {
        const current = owner.people.read().page
        requireResult(current.phase === 'ready', 'IN03 Intro list was not an actual successful read')
        if (current.data?.items.some(row => row.intro_id === introId && row.author.user_id === authorId)) {
          await owner.people.openProfile(authorId)
          requireResult(owner.people.read().profile.data?.user_id === authorId, 'IN03 Intro profile context mismatch')
          return { discovered: true, profile: true }
        }
        if (!current.data?.next_cursor) break
        await owner.people.load(true)
      }
      throw new Error('IN03 admitted Intro was not discoverable in bounded actual pages')
    },
    async comment(postId: string, text: string) {
      requireService('feed')
      owner.feed.start()
      await waitForController(() => owner.feed.read().feed.phase, owner.feed.subscribe)
      requireResult(owner.feed.read().feed.phase === 'ready', 'IN03 event feed unavailable')
      owner.thread.start(); owner.thread.select(postId)
      await waitForController(() => owner.thread.read().detail.phase, owner.thread.subscribe)
      requireResult(owner.thread.read().detail.data?.post.post_id === postId, 'IN03 post detail unavailable')
      owner.thread.setReplyText(text)
      await owner.thread.submitComment()
      await confirmMutation(() => owner.thread.read().mutation, owner.thread.checkStatus)
      const commentId = owner.thread.read().mutation.resourceId
      requireResult(commentId, 'IN03 successful comment has no resource identity')
      // Confirm persisted server data, not just the mutation response or local draft.
      await owner.thread.loadPost()
      for (let page = 0; page < 10; page++) {
        const state = owner.thread.read()
        requireResult(state.detail.phase === 'ready' && state.comments.phase === 'ready', 'IN03 comment read-back unavailable')
        if (state.comments.data?.items.some(row => row.comment_id === commentId && row.author.user_id === identity.scope.userId && row.text === text))
          return { persisted: true, commentId }
        if (!state.comments.data?.next_cursor) break
        await owner.thread.moreComments()
      }
      throw new Error('IN03 comment was not present in actual bounded read-back')
    },
    async request(introId: string, recipientId: string, platform: Platform) {
      requireService('requests')
      await sharing.loadHandles(); await sharing.loadOffer(introId)
      requireResult(sharing.read().offer.data?.recipient_id === recipientId, 'IN03 sharing offer recipient mismatch')
      const writes = () => identity.http().filter(row => row.method === 'POST' && row.path.endsWith('/social-requests')).length
      const before = writes()
      requireResult(!await sharing.createRequest(), 'IN03 request bypassed explicit requester consent')
      requireResult(writes() === before, 'IN03 no-consent action dispatched HTTP')
      sharing.setRequesterConsent(platform, true)
      await sharing.createRequest(); await checkSharing()
      const request = sharing.read().detail.data
      requireResult(request?.state === 'PENDING' && request.requester_id === identity.scope.userId && request.recipient_id === recipientId,
        'IN03 created request was not confirmed for the two actual principals')
      return { requestId: request!.request_id, state: request!.state, explicitConsent: true }
    },
    async decide(requestId: string, platform: Platform) {
      requireService('requests')
      await sharing.loadRequest(requestId)
      requireResult(sharing.read().detail.data?.state === 'PENDING', 'IN03 recipient did not read pending request')
      const writes = () => identity.http().filter(row => row.method === 'POST' && row.path.endsWith(`/social-requests/${requestId}/decision`)).length
      const before = writes()
      requireResult(!await sharing.decide(true), 'IN03 decision bypassed reciprocal consent')
      requireResult(writes() === before, 'IN03 no-consent decision dispatched HTTP')
      sharing.setDecisionPlatform(platform, true); sharing.setRecipientConsent(true)
      await sharing.decide(true); await checkSharing()
      requireResult(sharing.read().detail.data?.state === 'APPROVED', 'IN03 approved decision not confirmed')
      return { approved: true, reciprocalConsent: true }
    },
    async reveal(requestId: string, platform: Platform) {
      requireService('requests')
      await sharing.loadRequest(requestId)
      requireResult(sharing.read().secret.data === null, 'IN03 private handle appeared before explicit reveal')
      const before = identity.http().length
      const revealed = await sharing.revealHandle(platform)
      const reads = identity.http().slice(before).filter(row => row.path.endsWith(`/social-requests/${requestId}/reveal`))
      requireResult(revealed && sharing.read().secret.phase === 'ready' && reads.length === 1 && reads[0]?.status === 200 && reads[0]?.noStore,
        'IN03 explicit reveal requires a successful no-store actual HTTP response')
      // Never serialize the handle, whole controller state or network body into evidence.
      return { revealed: true, noStore: true }
    },
    async denyReveal(requestId: string, platform: Platform, expectedStatuses: readonly number[]) {
      requireResult(expectedStatuses.length > 0 && expectedStatuses.every(status => [403, 404, 410].includes(status)),
        'NOT_RUN: authorization denial statuses must come from the actual scenario authority')
      const before = identity.http().length
      let failure: TransportError | null = null
      try {
        await identity.auth.requestSensitive({ method: 'GET', access: 'protected', errorFamily: 'domain', domainOperation: 'revealConcertSocialHandle',
          path: `/api/concert/events/${identity.scope.eventId}/social-requests/${requestId}/reveal`, query: { platform }, decode: decode(reveal) })
      } catch (error) { if (error instanceof TransportError) failure = error; else throw new Error('IN03 reveal failed without a qualified HTTP authorization response') }
      const reads = identity.http().slice(before).filter(row => row.path.endsWith(`/social-requests/${requestId}/reveal`))
      requireResult(failure?.kind === 'http' && failure.status !== null && expectedStatuses.includes(failure.status)
        && reads.length === 1 && reads[0]?.status === failure.status, 'IN03 local refusal, malformed response or outage is not server authorization denial')
      return { denied: true, status: failure!.status }
    },
    async denyExpiredReveal(requestId: string, platform: Platform, expectedStatuses: readonly number[]) {
      const scope = identity.auth.privacy.capture()
      const response = await identity.auth.privacy.run(scope, context => identity.auth.request({ method: 'GET', access: 'protected',
        errorFamily: 'domain', domainOperation: 'getConcertSocialRequest', signal: context.signal,
        path: `/api/concert/events/${scope.eventId}/social-requests/${requestId}`,
        decode: decode(socialRequest, value => value.request_id === requestId && value.event_id === scope.eventId
          && [value.requester_id, value.recipient_id].includes(scope.userId!)) }))
      requireResult(response.data.state === 'EXPIRED', 'IN03 expired denial requires actual expired state for this participant')
      return this.denyReveal(requestId, platform, expectedStatuses)
    },
    async revoke(requestId: string) {
      requireService('requests')
      await sharing.loadRequest(requestId)
      const before = identity.auth.privacy.capture()
      await sharing.revokeRequest()
      for (let attempt = 0; identity.auth.privacy.isCurrent(before) && attempt < 12; attempt++) {
        requireResult(['pending', 'uncertain'].includes(sharing.read().mutation.phase), 'IN03 revoke command did not reach confirmed withdrawal')
        await pause(); await sharing.checkStatus()
      }
      // Successful revoke changes policy and clears its own mutation/controller state.
      const after = identity.auth.privacy.capture()
      requireResult(after.generation === before.generation && after.policyGeneration === before.policyGeneration + 1
        && sharing.read().secret.data === null && owner.read().phase === 'withdrawn', 'IN03 revoke did not withdraw the old private owner')
      // A policy event alone cannot certify that this particular command committed.
      const confirmed = await identity.auth.privacy.run(after, async context => {
        const response = await identity.auth.request({ method: 'GET', access: 'protected', errorFamily: 'domain', domainOperation: 'getConcertSocialRequest',
          path: `/api/concert/events/${after.eventId}/social-requests/${requestId}`, signal: context.signal,
          decode: decode(socialRequest, value => value.request_id === requestId && value.event_id === after.eventId
            && [value.requester_id, value.recipient_id].includes(after.userId!)) })
        return response.data.state
      })
      requireResult(confirmed === 'REVOKED', 'IN03 revoke did not persist server state')
      return { localWithdrawn: true, peerWS: 'NOT_RUN' as const }
    },
    async notification(requestId: string) {
      requireService('inbox')
      owner.inbox.start()
      await waitForController(() => owner.inbox.read().phase, owner.inbox.subscribe)
      for (let attempt = 0; attempt < 12; attempt++) {
        requireResult(owner.inbox.read().phase === 'ready', 'IN03 actual inbox unavailable')
        const row = owner.inbox.read().rows.find(value => value.kind === 'SOCIAL'
          && value.target?.target_type === 'SOCIAL_REQUEST' && value.target.target_id === requestId)
        if (row) {
          requireResult(row.read_at === null, 'IN03 isolated new notification was already read')
          await owner.inbox.loadUnreadCount()
          requireResult(owner.inbox.read().count.phase === 'ready' && (owner.inbox.read().count.total ?? 0) > 0,
            'IN03 own unread count was not available')
          await owner.inbox.open(row.notification_id)
          requireResult(owner.inbox.read().target.phase === 'idle', 'IN03 notification target failed server reauthorization')
          await owner.inbox.markRead(row.notification_id)
          for (let poll = 0; owner.inbox.read().command.phase === 'uncertain' && poll < 12; poll++) {
            await pause(); await owner.inbox.recover()
          }
          requireResult(owner.inbox.read().command.phase === 'succeeded', 'IN03 notification read command unresolved')
          await owner.inbox.load()
          requireResult(owner.inbox.read().rows.some(value => value.notification_id === row.notification_id && value.read_at !== null),
            'IN03 notification read state did not persist')
          return { targeted: true, reauthorized: true, readPersisted: true }
        }
        if (owner.inbox.read().nextCursor) await owner.inbox.load(true)
        else { await pause(); await owner.inbox.load() }
      }
      throw new Error('IN03 real targeted notification was not delivered within the bounded observation')
    },
  }
}
