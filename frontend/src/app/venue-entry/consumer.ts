import type { SessionController } from '../../features/auth/model/session'
import type { PrivacyScope } from '../../shared/query/privacy'
import type { CommunityBootstrapBinding } from '../community-adoption/bootstrap'
import type { MeetingConsumerPort, PhoneLocationPort, VenueCapabilities, VenueView } from '../../features/paradise-city-map/ports'
import { createVenueMapController } from '../../features/paradise-city-map/model/controller'
import { createBrowserLocation } from './browser-location'
import { bindCapabilitiesToCommunity, unavailableMeetingConsumer, unavailableVenueCapabilities } from './capability-adapters'

export interface VenueConsumerInput {
  readonly auth: Pick<SessionController, 'privacy'>; readonly scope: PrivacyScope; readonly signal: AbortSignal
  readonly community: CommunityBootstrapBinding | null
  readonly venue: VenueView; readonly capabilities?: VenueCapabilities; readonly meeting?: MeetingConsumerPort
  /** A separately owned test/device adapter, if supplied. This consumer only stops its watch. */
  readonly phone?: PhoneLocationPort
}
/** Borrows the original canonical privacy/shell lifetime. No Auth, query store, game or socket is created. */
export function createVenueConsumer(input: VenueConsumerInput) {
  const ownPhone = input.phone ? null : createBrowserLocation({ signal: input.signal })
  const phone = input.phone ?? ownPhone!
  const controller = createVenueMapController({ venue: input.venue,
    authority: { scope: input.scope, signal: input.signal,
      isCurrent: () => !input.signal.aborted && input.auth.privacy.isCurrent(input.scope),
      subscribe: listener => input.auth.privacy.registerCleanup(listener) },
    capabilities: bindCapabilitiesToCommunity(input.capabilities ?? unavailableVenueCapabilities, input.community, input.scope,
      () => !input.signal.aborted && input.auth.privacy.isCurrent(input.scope)),
    meeting: input.meeting ?? unavailableMeetingConsumer, phone })
  let disposed = false
  return { controller, dispose() { if (!disposed) { disposed = true; controller.dispose(); ownPhone?.dispose() } } }
}
