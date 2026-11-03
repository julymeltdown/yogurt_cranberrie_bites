import type { SessionPort, SessionScope } from '../api/http'
import { decodeMedia, mediaBase, uuid } from './contract'
import { inspectImage, MAX_IMAGE_BYTES } from './image-policy'
import { browserPreviews, type PreviewPort } from './preview'
import type { MediaRequest } from './transport'
import type { MediaPurpose, ReadyMedia } from './types'
export interface MediaPreviewState {
  readonly phase: 'idle' | 'loading' | 'ready' | 'failed' | 'cancelled'
  readonly scope: SessionScope | null; readonly previewUrl: string | null; readonly media: ReadyMedia | null; readonly error: string | null
}
const empty = (phase: MediaPreviewState['phase']): MediaPreviewState => Object.freeze({ phase, scope: null, previewUrl: null, media: null, error: null })
/** Authorized metadata + byte reads; never assign the protected API read_path directly to an img. */
export function createMediaPreviewController(options: {
  readonly session: SessionPort; readonly request: MediaRequest; readonly mediaId: string; readonly purpose: MediaPurpose; readonly previews?: PreviewPort
}) {
  const { session, request, mediaId, purpose } = options, previews = options.previews ?? browserPreviews
  if (!uuid(mediaId)) throw new Error('Invalid media ID')
  const listeners = new Set<() => void>()
  let state = empty('idle'), generation = 0, disposed = false, busy = false
  let active: AbortController | null = null, identitySignal: AbortSignal | null = null
  const emit = (v: MediaPreviewState) => { state = Object.freeze(v); for (const listener of listeners) listener() }
  function cancel() {
    generation++; busy = false
    const attempt = active; active = null
    identitySignal?.removeEventListener('abort', cancel); identitySignal = null
    const url = state.previewUrl; state = empty('cancelled')
    attempt?.abort(); if (url) previews.revoke(url)
    for (const listener of listeners) listener()
  }
  return {
    read: () => state,
    subscribe(listener: () => void) { listeners.add(listener); return () => listeners.delete(listener) },
    async load() {
      if (disposed || busy || state.phase === 'ready') return
      cancel(); const g = generation, snapshot = session.read()
      if (!uuid(snapshot.userId) || !uuid(snapshot.eventId) || snapshot.signal.aborted) { emit({ ...empty('failed'), error: 'authentication-required' }); return }
      const scope = Object.freeze({ generation: snapshot.generation, userId: snapshot.userId, eventId: snapshot.eventId })
      identitySignal = snapshot.signal; identitySignal.addEventListener('abort', cancel, { once: true })
      active = new AbortController(); const signal = active.signal; busy = true
      const current = () => !disposed && g === generation && session.isCurrent(scope) && !signal.aborted
      const context = (c: { scope: SessionScope; path: string; status: number }, path: string) => {
        if (c.status !== 200 || c.path !== path || c.scope.generation !== scope.generation || c.scope.userId !== scope.userId || c.scope.eventId !== scope.eventId) throw new Error('Invalid media context')
      }
      try {
        emit({ ...empty('loading'), scope }); if (!current()) return
        const path = `${mediaBase(scope)}/${mediaId}`
        const metadata = (await request({ path, method: 'GET', access: 'protected', errorFamily: 'domain', domainOperation: 'getConcertMedia', signal,
          decode: (v, c) => { context(c, path); return decodeMedia(v, scope, mediaId, purpose) },
        })).data
        if (!current()) return
        const content = (await request({ path: metadata.read_path, method: 'GET', access: 'protected', errorFamily: 'domain', domainOperation: 'readConcertMedia', signal,
          binaryResponse: { maxBytes: MAX_IMAGE_BYTES, contentTypes: [metadata.content_type] },
          decode: (v, c) => { context(c, metadata.read_path); if (!(v instanceof Blob)) throw new Error('Invalid media bytes'); return v },
        })).data
        if (!current()) return
        const info = await inspectImage(content, signal)
        if (!current()) return
        if (info.type !== metadata.content_type || info.width !== metadata.width || info.height !== metadata.height) throw new Error('Media metadata mismatch')
        const url = previews.create(content)
        if (!current()) { previews.revoke(url); return }
        emit({ phase: 'ready', scope, media: metadata, previewUrl: url, error: null })
      } catch { if (current()) emit({ ...empty('failed'), scope, error: 'media-unavailable' }) }
      finally { if (g === generation) { busy = false; active = null } }
    },
    cancel,
    dispose() { if (disposed) return; disposed = true; cancel(); listeners.clear() },
  }
}
