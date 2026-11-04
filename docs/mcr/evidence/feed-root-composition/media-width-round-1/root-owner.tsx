import { useCallback, useEffect, useLayoutEffect, useRef, useState, useSyncExternalStore, type CSSProperties } from 'react'
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
type NaturalGeometry = Readonly<{ width: number; height: number }>
type ObserveGeometry = (value: NaturalGeometry | null) => void
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
  // Geometry owns numbers only. The original scoped media owner still owns all
  // reads, bounded cache entries, binary data and URL lifetime.
  const expected = `${mediaScopeKey(scope)}:${eventId}:${purpose}:${mediaId}`
  const source = createPreview ?? binding?.owner
  const subscribePrivacy = useCallback((notify: () => void) => {
    try { return auth.privacy.subscribe(notify) } catch { return () => undefined }
  }, [auth])
  const currentScope = useSyncExternalStore(subscribePrivacy, () => auth.privacy.isCurrent(scope), () => false)
  const [geometry, setGeometry] = useState<{ auth: MediaOwnerAuth; source: unknown; key: string; value: NaturalGeometry } | null>(null)
  const observeGeometry = useCallback<ObserveGeometry>(value => {
    if (purpose !== 'POST' || !auth.privacy.isCurrent(scope) || scope.eventId !== eventId) value = null
    if (value && (!Number.isSafeInteger(value.width) || !Number.isSafeInteger(value.height) || value.width <= 0 || value.height <= 0)) value = null
    setGeometry(previous => {
      if (!value) return previous?.auth === auth && previous.source === source && previous.key === expected ? null : previous
      if (previous?.auth === auth && previous.source === source && previous.key === expected && previous.value.width === value.width && previous.value.height === value.height) return previous
      return { auth, source, key: expected, value }
    })
  }, [auth, scope, eventId, purpose, source, expected])
  useLayoutEffect(() => {
    try { return auth.privacy.registerCleanup(() => setGeometry(null)) } catch { setGeometry(null); return undefined }
  }, [auth])
  useLayoutEffect(() => {
    if (geometry && (geometry.auth !== auth || geometry.source !== source || geometry.key !== expected)) setGeometry(null)
  }, [geometry, auth, source, expected])
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
  const scoped = currentScope && scope.eventId === eventId && uuid(eventId) && uuid(mediaId)
  const provided = createPreview || binding?.auth === auth && mediaScopeKey(binding.scope) === mediaScopeKey(scope)
  const factory = createPreview ?? (binding ? (id: string) => binding.owner.acquire({ mediaId: id, purpose }) : null)
  // The factory identity is intentionally not an effect dependency: its source owner
  // is. A route rerender must not release and reacquire a visible image.
  const natural = scoped && provided && purpose === 'POST' && geometry?.auth === auth && geometry.source === source && geometry.key === expected ? geometry.value : null
  const style = natural ? { '--root-media-natural-ratio': `${natural.width} / ${natural.height}` } as CSSProperties : undefined
  return <div ref={target} className={`root-media-view root-media-view--${purpose.toLowerCase()}`} style={style}
    data-root-media-layout={natural ? 'authorized' : undefined} data-root-media-visibility={visibility}>
    {scoped && provided && factory && visibility === 'near'
      ? <ActiveImage key={`${mediaScopeKey(scope)}:${purpose}:${mediaId}`} {...props} factory={factory} source={source} observeGeometry={observeGeometry} />
      : !scoped || !provided ? <p>Image access unavailable.</p>
        : visibility === 'manual' ? <button type="button" onClick={() => setVisibility('near')}>Load image</button>
          : <span className="root-media-placeholder" aria-hidden="true">Image preview</span>}
  </div>
}
function ActiveImage({ auth, scope, mediaId, eventId, alt, purpose, factory, source, observeGeometry }: ImageProps & {
  readonly factory: (id: string) => MediaPreviewPort; readonly source: unknown; readonly observeGeometry: ObserveGeometry
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
  useLayoutEffect(() => { if (failed) observeGeometry(null) }, [failed, observeGeometry])
  const retry = () => setAttempt(value => value + 1)
  return failed ? <ImageFailure purpose={purpose} retry={retry} />
    : mounted && mounted.source === source && mounted.attempt === attempt
      ? <ImageContent port={mounted.port} auth={auth} scope={scope} mediaId={mediaId} eventId={eventId} alt={alt} purpose={purpose} retry={retry} observeGeometry={observeGeometry} />
      : <span className="root-media-placeholder" aria-hidden="true">Loading image…</span>
}
function ImageFailure({ purpose, retry }: { readonly purpose: RetainedMediaPurpose; readonly retry: () => void }) {
  return <div><p>Image unavailable.{purpose === 'POST' ? ' Post text remains available.' : ''}</p><button type="button" aria-label="Retry image read" onClick={retry}>Retry</button></div>
}
function ImageContent({ port, auth, scope, mediaId, eventId, alt, purpose, retry, observeGeometry }: Omit<ImageProps, 'createPreview'> & {
  readonly port: MediaPreviewPort; readonly retry: () => void; readonly observeGeometry: ObserveGeometry
}) {
  const state = useSyncExternalStore(port.subscribe, port.read, port.read)
  const valid = auth.privacy.isCurrent(scope) && state.scope && state.scope.generation === scope.generation
    && state.scope.userId === scope.userId && state.scope.eventId === scope.eventId && scope.eventId === eventId
  const ready = state.phase === 'ready' && valid && state.media?.media_id === mediaId && state.media.purpose === purpose
    && state.media.read_path === `/api/concert/events/${eventId}/media/${mediaId}/content` && state.previewUrl?.startsWith('blob:')
  const width = state.media?.width, height = state.media?.height
  useLayoutEffect(() => {
    if (ready) observeGeometry(width && height ? { width, height } : null)
    else if (state.phase === 'failed' || state.phase === 'cancelled' || state.phase === 'ready' && !ready) observeGeometry(null)
  }, [ready, width, height, state.phase, observeGeometry])
  if (ready && state.media) {
    return <img className={purpose === 'POST' ? 'feed-image root-media-image' : 'root-media-image'} src={state.previewUrl ?? undefined} alt={alt}
      width={state.media.width} height={state.media.height} decoding="async" />
  }
  if (state.phase === 'failed' || state.phase === 'ready') return <ImageFailure purpose={purpose} retry={retry} />
  return <span className="root-media-placeholder" aria-hidden="true">{state.phase === 'cancelled' ? 'Image access ended.' : 'Loading image…'}</span>
}
