import { useEffect, useRef, useState, useSyncExternalStore } from 'react'
import type { GiftImageRequest } from '../../features/gift-discovery'
import type { MediaPreviewPort } from '../../features/event-feed'
import { uuid } from '../../shared/media/contract'
import type { PrivacyScope } from '../../shared/query/privacy'
import type { MediaOwnerAuth, RetainedMediaPurpose } from './index'
import { mediaScopeKey, useRootMediaOwner } from './provider'
import './root-media.css'

interface PostImageProps {
  readonly auth: MediaOwnerAuth; readonly scope: PrivacyScope; readonly mediaId: string; readonly eventId: string; readonly alt: string
  readonly createPreview?: ((mediaId: string) => MediaPreviewPort) | undefined
}
interface ImageProps extends PostImageProps { readonly purpose: RetainedMediaPurpose }
export function RootPostImage(props: PostImageProps) { return <RootMediaView {...props} purpose="POST" /> }
export function RootGiftImage(request: GiftImageRequest) {
  const binding = useRootMediaOwner()
  return binding && uuid(request.giveawayId)
    ? <RootMediaView auth={binding.auth} scope={request.scope} mediaId={request.mediaId} eventId={request.scope.eventId ?? ''} alt={request.alt} purpose="GIVEAWAY" />
    : <div className="root-media-view root-media-view--giveaway"><p>Image access unavailable.</p></div>
}
function RootMediaView(props: ImageProps) {
  const { auth, scope, mediaId, eventId, purpose, createPreview } = props, binding = useRootMediaOwner()
  const target = useRef<HTMLDivElement>(null)
  const [visibility, setVisibility] = useState<'waiting' | 'near' | 'outside' | 'manual'>('waiting')
  useEffect(() => {
    const element = target.current
    if (!element) return
    if (typeof IntersectionObserver === 'undefined') { setVisibility('manual'); return }
    const observer = new IntersectionObserver(entries => {
      const entry = entries.find(entry => entry.target === element)
      if (entry) setVisibility(entry.isIntersecting ? 'near' : 'outside')
    }, { rootMargin: '200px 0px', threshold: 0 })
    observer.observe(element)
    return () => observer.disconnect()
  }, [])
  const scoped = auth.privacy.isCurrent(scope) && scope.eventId === eventId && uuid(eventId) && uuid(mediaId)
  const provided = createPreview || binding?.auth === auth && mediaScopeKey(binding.scope) === mediaScopeKey(scope)
  const factory = createPreview ?? (binding ? (id: string) => binding.owner.acquire({ mediaId: id, purpose }) : null)
  // The factory identity is intentionally not an effect dependency: its source owner
  // is. A route rerender must not release and reacquire a visible image.
  return <div ref={target} className={`root-media-view root-media-view--${purpose.toLowerCase()}`}>
    {scoped && provided && factory && visibility === 'near'
      ? <ActiveImage key={`${mediaScopeKey(scope)}:${purpose}:${mediaId}`} {...props} factory={factory} source={createPreview ?? binding?.owner} />
      : !scoped || !provided ? <p>Image access unavailable.</p>
        : visibility === 'manual' ? <button type="button" onClick={() => setVisibility('near')}>Load image</button>
          : <span className="root-media-placeholder" aria-hidden="true">Image preview</span>}
  </div>
}
function ActiveImage({ auth, scope, mediaId, eventId, alt, purpose, factory, source }: ImageProps & {
  readonly factory: (id: string) => MediaPreviewPort; readonly source: unknown
}) {
  const [attempt, setAttempt] = useState(0)
  const [mounted, setMounted] = useState<{ source: unknown; attempt: number; port: MediaPreviewPort } | null>(null)
  const [failed, setFailed] = useState(false)
  const factoryRef = useRef(factory); factoryRef.current = factory
  const expected = mediaScopeKey(scope)
  useEffect(() => {
    if (!auth.privacy.isCurrent(scope) || scope.eventId !== eventId) return
    let port: MediaPreviewPort | null = null, remove: () => void = () => undefined
    setFailed(false)
    try {
      port = factoryRef.current(mediaId)
      const acquired = port
      remove = auth.privacy.registerCleanup(() => acquired.cancel())
      setMounted({ source, attempt, port })
      void port.load().catch(() => acquired.cancel())
    } catch { port?.dispose(); setFailed(true) }
    return () => { remove(); port?.dispose() }
  }, [auth, expected, mediaId, eventId, source, attempt])
  const retry = () => setAttempt(value => value + 1)
  return failed ? <ImageFailure purpose={purpose} retry={retry} />
    : mounted && mounted.source === source && mounted.attempt === attempt
      ? <ImageContent port={mounted.port} auth={auth} scope={scope} mediaId={mediaId} eventId={eventId} alt={alt} purpose={purpose} retry={retry} />
      : <span className="root-media-placeholder" aria-hidden="true">Loading image…</span>
}
function ImageFailure({ purpose, retry }: { readonly purpose: RetainedMediaPurpose; readonly retry: () => void }) {
  return <div><p>Image unavailable.{purpose === 'POST' ? ' Post text remains available.' : ''}</p><button type="button" aria-label="Retry image read" onClick={retry}>Retry</button></div>
}
function ImageContent({ port, auth, scope, mediaId, eventId, alt, purpose, retry }: Omit<ImageProps, 'createPreview'> & {
  readonly port: MediaPreviewPort; readonly retry: () => void
}) {
  const state = useSyncExternalStore(port.subscribe, port.read, port.read)
  const valid = auth.privacy.isCurrent(scope) && state.scope && state.scope.generation === scope.generation
    && state.scope.userId === scope.userId && state.scope.eventId === scope.eventId && scope.eventId === eventId
  if (state.phase === 'ready' && valid && state.media?.media_id === mediaId && state.media.purpose === purpose
    && state.media.read_path === `/api/concert/events/${eventId}/media/${mediaId}/content` && state.previewUrl?.startsWith('blob:')) {
    return <img className={purpose === 'POST' ? 'feed-image root-media-image' : 'root-media-image'} src={state.previewUrl} alt={alt}
      width={state.media.width} height={state.media.height} decoding="async" />
  }
  if (state.phase === 'failed' || state.phase === 'ready') return <ImageFailure purpose={purpose} retry={retry} />
  return <span className="root-media-placeholder" aria-hidden="true">{state.phase === 'cancelled' ? 'Image access ended.' : 'Loading image…'}</span>
}
