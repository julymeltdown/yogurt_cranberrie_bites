import type { GiftPickupController } from '../../features/gift-pickup/model/controller'
import type { RenderPickupImage } from '../../features/gift-pickup/ui/GiftPickup'
import { RootGiftImage } from '../media-entry/RootMediaView'
import { mediaScopeKey, useRootMediaOwner, type RootMediaBinding } from '../media-entry/provider'
import type { CommunityEntry } from './bridge'
import { canonicalId } from '../routes'
import type { CommunityEntryAuth, CommunityEntryRelease } from './ports'

/** Borrow the original entry lease. This bridge creates no controller or media owner. */
export interface PickupMediaLease {
  readonly auth: CommunityEntryAuth | null
  readonly release: CommunityEntryRelease | null
  readonly entry: CommunityEntry
}

export function pickupMediaScope(controller: GiftPickupController | null, lease: PickupMediaLease | null,
  binding: RootMediaBinding | null, request: Parameters<RenderPickupImage>[0]) {
  try {
  if (!canonicalId(request.giveawayId) || !canonicalId(request.mediaId)) return null
  if (!controller || !lease?.auth || !lease.release || !binding || binding.auth !== lease.auth
    || lease.release.signal.aborted || lease.entry.read().phase !== 'available') return null
  const scope = lease.release.scope
  if (scope.gate !== 'open' || !scope.userId || !scope.eventId || !lease.auth.privacy.isCurrent(scope)
    || mediaScopeKey(scope) !== mediaScopeKey(binding.scope) || binding.owner.inspect().disposed) return null
  const context = lease.release.context.read(), state = controller.read(), gift = state.gift
  if (!context || !context.capabilities.gifts.available || mediaScopeKey(context.scope) !== mediaScopeKey(scope)
    || !state.access || state.phase === 'denied' || !gift || gift.event_id !== scope.eventId
    || gift.giveaway_id !== request.giveawayId || gift.image_media_id !== request.mediaId
    || gift.state === 'DRAFT' && gift.owner.user_id !== scope.userId) return null
  return scope
  } catch { return null }
}

export function usePickupMedia(controller: GiftPickupController | null, lease: PickupMediaLease | null): RenderPickupImage {
  const binding = useRootMediaOwner()
  return request => {
    const scope = pickupMediaScope(controller, lease, binding, request)
    return scope ? <RootGiftImage {...request} scope={scope} /> : null
  }
}
