import type { LoadedSiteplan } from '../spatial/image-resource'
import type { LandmarkId } from '../spatial/contract'
import type { MeetingLandmarkContext, VenueView } from '../ports'

/** Borrow the producer's verified image lease and geometry; no raw-image refetch or ID conversion. */
export function projectLoadedSiteplan(loaded: LoadedSiteplan, meetings: ReadonlyMap<LandmarkId, MeetingLandmarkContext> = new Map()): VenueView {
  const source = loaded.image.source
  if (typeof HTMLImageElement === 'undefined' || !(source instanceof HTMLImageElement) || !source.src.startsWith('blob:')) throw Error('A live verified siteplan image lease is required')
  const siteplan = loaded.snapshot.siteplan
  return Object.freeze({ identity: Object.freeze({ venueId: siteplan.venueId, siteplanId: siteplan.siteplanId, revision: siteplan.revision, variantId: siteplan.variantId }),
    title: 'Paradise City', classification: 'ILLUSTRATED_UNCALIBRATED',
    image: Object.freeze({ src: source.src, width: siteplan.image.width, height: siteplan.image.height, description: 'Paradise City illustrated siteplan, uncalibrated' }),
    landmarks: Object.freeze(siteplan.landmarks.map(place => Object.freeze({ id: place.id, label: place.displayName, point: place.anchor,
      meetingContext: meetings.has(place.id) ? Object.freeze({ ...meetings.get(place.id)! }) : null }))),
  })
}
