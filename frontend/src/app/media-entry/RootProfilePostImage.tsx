import { useCallback, useLayoutEffect, useState, useSyncExternalStore } from 'react'
import type { Post } from '../../features/event-feed/contract'
import { ConcertIcon } from '../../shared/ui/concert-window'
import type { PrivacyScope } from '../../shared/query/privacy'
import { uuid } from '../../shared/media/contract'
import type { MediaLease, MediaOwnerAuth } from './index'
import { mediaScopeKey, useRootMediaOwner } from './provider'

export interface RootProfilePostImageProps { readonly auth: MediaOwnerAuth; readonly scope: PrivacyScope; readonly post: Post }
/** A decorative view of a retained authorized reader, never a new media request. */
export function RootProfilePostImage({ auth, scope, post }: RootProfilePostImageProps) {
  const binding = useRootMediaOwner(), mediaId = post.media_ids[0], key = `${mediaScopeKey(scope)}:${post.post_id}:${mediaId}`
  const subscribe = useCallback((notify: () => void) => {
    try { return auth.privacy.subscribe(notify) } catch { return () => undefined }
  }, [auth])
  const current = useSyncExternalStore(subscribe, () => auth.privacy.isCurrent(scope), () => false)
  const eligible = current && binding?.auth === auth && mediaScopeKey(binding.scope) === mediaScopeKey(scope)
    && scope.eventId === post.event_id && uuid(post.event_id) && uuid(mediaId)
  const [mounted, setMounted] = useState<{ owner: NonNullable<typeof binding>; key: string; lease: MediaLease } | null>(null)
  useLayoutEffect(() => {
    if (!eligible || !binding || !mediaId) return
    const lease = binding.owner.acquireCached({ mediaId, purpose: 'POST' })
    if (!lease) { setMounted(null); return }
    setMounted({ owner: binding, key, lease })
    return lease.dispose
  }, [eligible, binding, mediaId, key])
  return <span aria-hidden="true" style={{ display: 'grid', placeItems: 'center', inlineSize: '100%', blockSize: '100%', maxInlineSize: 52, maxBlockSize: 52, aspectRatio: '1', overflow: 'hidden' }}>
    {eligible && mounted && mounted.owner === binding && mounted.key === key && mediaId
      ? <CachedImage key={key} lease={mounted.lease} auth={auth} scope={scope} mediaId={mediaId} /> : <ConcertIcon name="feed" />}
  </span>
}
function CachedImage({ lease, auth, scope, mediaId }: { lease: MediaLease; auth: MediaOwnerAuth; scope: PrivacyScope; mediaId: string }) {
  const state = useSyncExternalStore(lease.subscribe, lease.read, lease.read), [failed, setFailed] = useState(false)
  const valid = auth.privacy.isCurrent(scope) && state.phase === 'ready' && state.scope?.generation === scope.generation
    && state.scope.userId === scope.userId && state.scope.eventId === scope.eventId
    && state.media?.media_id === mediaId && state.media.purpose === 'POST'
    && state.media.read_path === `/api/concert/events/${scope.eventId}/media/${mediaId}/content` && state.previewUrl?.startsWith('blob:')
  return valid && !failed ? <img src={state.previewUrl ?? undefined} alt="" decoding="async" onError={() => setFailed(true)}
    style={{ display: 'block', inlineSize: '100%', blockSize: '100%', maxInlineSize: 52, maxBlockSize: 52, objectFit: 'cover' }} /> : <ConcertIcon name="feed" />
}
