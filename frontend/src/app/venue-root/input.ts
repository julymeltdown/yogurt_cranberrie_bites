import type { PrivacyScope } from '../../shared/query/privacy'
import type { LoadedSiteplan } from '../../features/paradise-city-map/spatial/image-resource'
import { sameSiteplanIdentity, type LandmarkId } from '../../features/paradise-city-map/spatial/contract'
import { productionSiteplanRegistration } from '../../features/paradise-city-map/spatial/registration'
import { projectLoadedSiteplan } from '../../features/paradise-city-map/model/siteplan-view'
import type { MeetingLandmarkContext, PhoneLocationPort, VenueCapabilities, VenueView } from '../../features/paradise-city-map/ports'
import type { CommunityBootstrapBinding } from '../community-adoption/bootstrap'
import type { MeetingLeafOptions, MeetingLeafRelease } from '../venue-community-entry'

export interface VenueRootBorrowed {
  readonly auth: NonNullable<MeetingLeafOptions['auth']>
  readonly scope: PrivacyScope
  readonly signal: AbortSignal
  readonly community: CommunityBootstrapBinding | null
  /** Original verified ART image lease. This borrower must never dispose it. */
  readonly siteplan: LoadedSiteplan
  /** Explicit current real-provider associations, never derived from ART IDs. */
  readonly meetingPlaces?: ReadonlyMap<LandmarkId, MeetingLandmarkContext>
  readonly capabilities: VenueCapabilities
  readonly meeting: MeetingLeafRelease | null
  readonly phone: PhoneLocationPort
  readonly newCommandKey: () => string
  /** Root retains the router and actual /world/map destination. */
  readonly navigateToMap: () => void
}
export interface VenueRootInput extends Omit<VenueRootBorrowed, 'meetingPlaces'> {
  readonly venue: VenueView
}

/** Call once per actual shell/image/provider release, outside Outlet rendering.
 * Pure projection: no fetch, controller, subscription, key, clock or resource. */
export function projectVenueRootView(siteplan: LoadedSiteplan, meetingPlaces: ReadonlyMap<LandmarkId, MeetingLandmarkContext> = new Map()): VenueView {
  const registered = productionSiteplanRegistration, actual = siteplan.snapshot.siteplan
  if (siteplan.model.snapshot !== siteplan.snapshot || !sameSiteplanIdentity(actual, registered)
    || siteplan.producer.kind !== registered.producer.kind || siteplan.producer.sourceRevision !== registered.producer.sourceRevision
    || actual.image.path !== registered.image.path || actual.image.sha256 !== registered.image.sha256
    || actual.image.width !== registered.image.width || actual.image.height !== registered.image.height
    || actual.image.bytes !== registered.image.bytes) throw Error('The registered verified venue image lease is required')
  return projectLoadedSiteplan(siteplan, meetingPlaces)
}

/** Call once per actual shell/image/provider release, outside Outlet rendering.
 * Pure projection: no fetch, controller, subscription, key, clock or resource. */
export function createVenueRootInput(borrowed: VenueRootBorrowed): VenueRootInput {
  const venue = projectVenueRootView(borrowed.siteplan, borrowed.meetingPlaces)
  return Object.freeze({ auth: borrowed.auth, scope: Object.freeze({ ...borrowed.scope }), signal: borrowed.signal,
    community: borrowed.community, siteplan: borrowed.siteplan, venue, capabilities: borrowed.capabilities,
    meeting: borrowed.meeting, phone: borrowed.phone, newCommandKey: borrowed.newCommandKey, navigateToMap: borrowed.navigateToMap })
}
