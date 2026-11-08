/**
 * WP-06 T-507: the profile media gallery.
 *
 * A grid of the profile's ordinary posts. Three things it must get right:
 *
 * 1. **Aspect ratio is preserved.** Each tile reserves the aspect ratio the
 *    server reported (`width`/`height`), so the grid does not jump while images
 *    load. The image itself is `object-fit: contain` inside that box, never
 *    cropped to a square.
 * 2. **Lazy loading with retry.** `loading="lazy"` and `decoding="async"` keep a
 *    long gallery cheap. A failed image shows a retry control rather than a
 *    broken-image icon; the retry re-requests the same authenticated path.
 * 3. **An empty state that does not imply restriction.** "No posts yet" is a
 *    fact about the profile. It is never rendered as "not available to you",
 *    which would be a visibility claim the gallery cannot make: a private post
 *    the viewer may not see is simply absent from the page the server returned,
 *    and the gallery must not editorialize about it.
 *
 * A restricted intro attachment never appears here. That is a server guarantee
 * (an `INTRO_POST` asset cannot be attached to a profile post, and the gallery
 * only renders `media` of a profile post), and the gallery adds a client-side
 * check so a malformed payload cannot smuggle one in.
 */
import { useCallback, useEffect, useId, useMemo, useRef, useState, useSyncExternalStore } from 'react'
import type { HttpClient } from '../../shared/api/http'
import type { PrivacyScope } from '../../shared/query/privacy'
import { decodeProfilePostPage, PATHS, type ProfilePost, type ProfileMedia } from './contract'
import './profile-media.css'

/** A gallery read: one page of a profile's posts. */
export type GalleryReader = Readonly<{
  list: (options: { readonly signal: AbortSignal; readonly cursor: string | null }) => Promise<{
    readonly items: readonly ProfilePost[]
    readonly next_cursor: string | null
  }>
}>

export type GalleryDeps = Readonly<{
  auth: Readonly<{
    privacy: { run<T>(scope: PrivacyScope, work: (context: { readonly signal: AbortSignal }) => T | Promise<T>, signal?: AbortSignal): Promise<T> }
    request: HttpClient['request']
  }>
  scope: PrivacyScope
  profileId: string
}>

/** Build the gallery reader over the shared HTTP client. */
export function createGalleryReader(deps: GalleryDeps): GalleryReader {
  const { auth, scope, profileId } = deps
  return Object.freeze({
    list: ({ signal, cursor }) => auth.privacy.run(scope, ({ signal: active }) => auth.request({
      path: PATHS.listPosts(profileId),
      method: 'GET',
      access: 'protected',
      errorFamily: 'domain',
      domainOperation: 'listProfilePosts',
      signal: active,
      query: cursor === null ? { limit: 20 } : { limit: 20, cursor },
      decode: decodeProfilePostPage,
    }), signal).then(result => ({
      items: result.data.items,
      next_cursor: result.data.next_cursor,
    })),
  })
}

/** The observable gallery state. */
export type GalleryState = Readonly<{
  posts: readonly ProfilePost[]
  cursor: string | null
  phase: 'idle' | 'loading' | 'ready' | 'failed'
  /** A 404 from the server: the profile is not available to this viewer. */
  unavailable: boolean
  notice: string | null
}>

/**
 * The gallery store.
 *
 * It is a plain external store so the component can be tested without a query
 * client and so an account switch aborts the in-flight page through the privacy
 * coordinator.
 */
export function createGalleryStore(reader: GalleryReader) {
  let state: GalleryState = Object.freeze({
    posts: Object.freeze([]),
    cursor: null,
    phase: 'idle',
    unavailable: false,
    notice: null,
  })
  const listeners = new Set<() => void>()
  let lifetime = new AbortController()
  const publish = (next: Partial<GalleryState>) => {
    state = Object.freeze({ ...state, ...next })
    for (const listener of Array.from(listeners)) listener()
  }
  async function load(cursor: string | null = null) {
    publish({ phase: 'loading', notice: null, unavailable: false })
    try {
      const page = await reader.list({ signal: lifetime.signal, cursor })
      publish({
        posts: Object.freeze(cursor === null ? page.items : [...state.posts, ...page.items]),
        cursor: page.next_cursor,
        phase: 'ready',
      })
    } catch (error) {
      if (lifetime.signal.aborted) return
      const status = error instanceof Error && 'status' in error ? (error as { status: number | null }).status : null
      publish({
        phase: 'failed',
        unavailable: status === 404,
        notice: status === 404
          ? 'This profile is not available.'
          : 'The posts could not be loaded.',
      })
    }
  }
  return Object.freeze({
    read: () => state,
    subscribe(listener: () => void) {
      listeners.add(listener)
      return () => { listeners.delete(listener) }
    },
    load,
    loadMore: () => (state.cursor === null ? Promise.resolve() : load(state.cursor)),
    dispose() {
      lifetime.abort()
      lifetime = new AbortController()
      listeners.clear()
    },
  })
}

export type GalleryStore = ReturnType<typeof createGalleryStore>

/** Whether a media item may be rendered by this gallery. */
export function admitGalleryMedia(media: ProfileMedia): boolean {
  // Only a PROFILE_POST asset belongs in the profile gallery. An intro
  // attachment is an INTRO_POST asset and is refused here even if a malformed
  // payload associated one.
  return media.purpose === 'PROFILE_POST' && media.state === 'READY'
}

/** The props of the gallery. */
export type ProfileMediaGalleryProps = Readonly<{
  store: GalleryStore
  /** Whether the viewer is the author, which is the only case with a delete. */
  isAuthor: boolean
  /** Delete one post. The gallery reloads on success. */
  onDelete?: (post: ProfilePost) => Promise<void>
  /** The media source for a tile. Injected so tests do not fetch bytes. */
  imageSrc?: (media: ProfileMedia) => string
}>

export function ProfileMediaGallery({ store, isAuthor, onDelete, imageSrc }: ProfileMediaGalleryProps) {
  const state = useSyncExternalStore(store.subscribe, store.read, store.read)
  const id = useId()
  const [deleting, setDeleting] = useState<string | null>(null)
  useEffect(() => { void store.load() }, [store])
  const src = useCallback((media: ProfileMedia) => imageSrc ? imageSrc(media) : media.read_path, [imageSrc])

  const visible = useMemo(
    () => state.posts.map(post => Object.freeze({
      ...post,
      media: post.media.filter(admitGalleryMedia),
    })),
    [state.posts],
  )

  if (state.phase === 'loading' && state.posts.length === 0) {
    return <p role="status">Loading posts…</p>
  }
  if (state.unavailable) {
    // A 404 is the server's answer for a profile this viewer may not see. It is
    // a different statement from "this profile has no posts".
    return <p role="status" className="profile-media-unavailable">
      This profile is not available to you.
    </p>
  }
  if (state.phase === 'failed') {
    return <div role="alert" className="profile-media-error">
      <p>{state.notice}</p>
      <button type="button" onClick={() => { void store.load() }}>Try again</button>
    </div>
  }
  if (visible.length === 0) {
    return <p role="status" className="profile-media-empty">
      {isAuthor ? 'You have not posted any photos yet.' : 'No posts yet.'}
    </p>
  }

  return <section className="profile-media-gallery" aria-labelledby={`${id}-title`}>
    <h2 id={`${id}-title`}>Posts</h2>
    <ul className="profile-media-grid">
      {visible.map(post => <li key={post.post_id} className="profile-media-post">
        {post.media.length > 0 && <ul className="profile-media-tiles">
          {post.media.map(media => <li key={media.media_id}>
            <GalleryTile media={media} src={src(media)} alt={post.description} />
          </li>)}
        </ul>}
        {post.description !== '' && <p className="profile-media-description">{post.description}</p>}
        {post.visibility === 'PRIVATE' && <p className="profile-media-hint">Only you can see this post.</p>}
        {isAuthor && onDelete && <button
          type="button"
          disabled={deleting === post.post_id}
          onClick={() => {
            setDeleting(post.post_id)
            void onDelete(post).finally(() => setDeleting(null))
          }}
        >
          {deleting === post.post_id ? 'Deleting…' : 'Delete post'}
        </button>}
      </li>)}
    </ul>
    {state.cursor !== null && <button type="button" onClick={() => { void store.loadMore() }}>
      Show more posts
    </button>}
  </section>
}

/**
 * One image tile.
 *
 * The box reserves the server-reported aspect ratio, so the grid never reflows
 * when the image arrives. A load failure offers a retry that re-requests the
 * same authenticated path with a cache-busting fragment (the path itself is
 * `no-store`, so a retry is a real request).
 */
function GalleryTile({ media, src, alt }: { media: ProfileMedia; src: string; alt: string }) {
  const [attempt, setAttempt] = useState(0)
  const [failed, setFailed] = useState(false)
  const [loaded, setLoaded] = useState(false)
  const ref = useRef<HTMLImageElement | null>(null)
  // A server-side sanitized photograph always has a positive size; the guard
  // keeps a malformed payload from producing a zero-height box.
  const ratio = media.width > 0 && media.height > 0 ? media.width / media.height : 1
  return <figure className="profile-media-tile" style={{ aspectRatio: `${ratio}` }}>
    {!failed && <img
      ref={ref}
      src={attempt === 0 ? src : `${src}#retry-${attempt}`}
      alt={alt === '' ? 'A post image' : alt}
      loading="lazy"
      decoding="async"
      width={media.width}
      height={media.height}
      onLoad={() => setLoaded(true)}
      onError={() => setFailed(true)}
    />}
    {loaded && alt === '' && <figcaption className="profile-media-hint">No alt text was provided.</figcaption>}
    {failed && <div className="profile-media-tile-error">
      <p role="alert">This image could not be loaded.</p>
      <button type="button" onClick={() => { setFailed(false); setLoaded(false); setAttempt(value => value + 1) }}>
        Retry image
      </button>
    </div>}
  </figure>
}
