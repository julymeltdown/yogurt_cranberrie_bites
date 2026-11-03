import { useEffect, useState, useSyncExternalStore } from 'react'
import type { SessionScope } from '../../shared/api/http'
import type { PrivacyScope } from '../../shared/query/privacy'
import type { FeedAuth } from './runtime'
export interface MediaPreviewPort {
  read(): {
    readonly phase: 'idle' | 'loading' | 'ready' | 'failed' | 'cancelled'
    readonly scope: SessionScope | null; readonly previewUrl: string | null
    readonly media: { readonly media_id: string; readonly purpose: 'POST' | 'GIVEAWAY' | 'REPORT'; readonly read_path: string; readonly width: number; readonly height: number } | null
    readonly error: string | null
  }
  subscribe(listener: () => void): () => void
  load(): Promise<void>; cancel(): void; dispose(): void
}
interface ImageProps {
  createPreview: (mediaId: string) => MediaPreviewPort; privacy: FeedAuth['privacy']; mediaId: string; eventId: string; alt: string
}
interface MountedImage { controller: MediaPreviewPort; scope: PrivacyScope; mediaId: string; eventId: string }
/** A new disposable controller per effect lifetime, including StrictMode's setup-cleanup-setup. */
export function FeedImage({ createPreview, privacy, mediaId, eventId, alt }: ImageProps) {
  const [mounted, setMounted] = useState<MountedImage | null>(null)
  useEffect(() => {
    const scope = privacy.capture()
    if (!privacy.isCurrent(scope) || scope.eventId !== eventId) { setMounted(null); return }
    const controller = createPreview(mediaId)
    const remove = privacy.registerCleanup(() => controller.cancel())
    setMounted({ controller, scope, mediaId, eventId })
    void controller.load().catch(() => controller.cancel())
    return () => { remove(); controller.dispose() }
  }, [createPreview, privacy, mediaId, eventId])
  if (!mounted || mounted.mediaId !== mediaId || mounted.eventId !== eventId) return <p role="status">Loading image…</p>
  return <ImageContent {...mounted} privacy={privacy} alt={alt} />
}
function ImageContent({ controller, scope, privacy, mediaId, eventId, alt }: MountedImage & Pick<ImageProps, 'privacy' | 'alt'>) {
  const state = useSyncExternalStore(controller.subscribe, controller.read, controller.read)
  const scoped = state.scope && privacy.isCurrent(scope) && state.scope.generation === scope.generation
    && state.scope.userId === scope.userId && state.scope.eventId === scope.eventId && scope.eventId === eventId
  if (state.phase === 'ready' && scoped && state.media?.media_id === mediaId && state.media.purpose === 'POST'
    && state.media.read_path === `/api/concert/events/${eventId}/media/${mediaId}/content` && state.previewUrl?.startsWith('blob:')) {
    return <img className="feed-image" src={state.previewUrl} alt={alt} width={state.media.width} height={state.media.height} loading="lazy" decoding="async" />
  }
  if (state.phase === 'failed' && privacy.isCurrent(scope)) return <div><p>Image unavailable. Post text is still available.</p><button type="button" onClick={() => { void controller.load().catch(() => controller.cancel()) }}>Retry image read</button></div>
  return <p role="status">{state.phase === 'cancelled' ? 'Image access ended.' : 'Loading image…'}</p>
}
