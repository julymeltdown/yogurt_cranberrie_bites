import type { SessionPort, SessionSnapshot, SessionScope } from '../../../shared/api/http'
import { TransportError } from '../../../shared/api/errors'
import { decodeIntent, decodeMedia, decodeReceipt, decodeUploaded, uuid, type CommandBinding, type MediaCommandResult, type MediaOperation } from '../../../shared/media/contract'
import { ImageError, inspectImage } from '../../../shared/media/image-policy'
import { browserPreviews, type PreviewPort } from '../../../shared/media/preview'
import { prepareImage, type PrepareImage } from '../../../shared/media/prepare-image'
import type { MediaUploadTransport } from '../../../shared/media/transport'
import type { ImageUploadPort, MediaPurpose, PreparedImage, UploadState } from '../../../shared/media/types'
export type { ImageUploadPort, MediaPurpose, ReadyMedia, UploadState } from '../../../shared/media/types'
export { createMediaUploadTransport, type MediaUploadTransport } from '../../../shared/media/transport'
export interface ImageUploadOptions {
  readonly session: SessionPort; readonly transport: MediaUploadTransport; readonly purpose: MediaPurpose
  readonly prepare?: PrepareImage; readonly previews?: PreviewPort
  /** Deterministic fixtures only; production uses canonical random UUID and browser clock. */
  readonly key?: () => string; readonly now?: () => number
}
function abortable<T>(promise: Promise<T>, signal: AbortSignal): Promise<T> {
  return new Promise((resolve, reject) => {
    const abort = () => reject(new ImageError('aborted'))
    signal.addEventListener('abort', abort, { once: true })
    promise.then(v => { signal.removeEventListener('abort', abort); resolve(v) }, e => { signal.removeEventListener('abort', abort); reject(e) })
    if (signal.aborted) abort()
  })
}
const empty = (phase: UploadState['phase'] = 'idle'): UploadState => Object.freeze({ phase, scope: null, previewUrl: null, progress: null, attachment: null, error: null })
export function createImageUploadController(options: ImageUploadOptions): ImageUploadPort {
  const { session, transport, purpose } = options
  const previews = options.previews ?? browserPreviews, prepare = options.prepare ?? prepareImage, now = options.now ?? Date.now
  const key = options.key ?? (() => crypto.randomUUID())
  let state = empty(), generation = 0, disposed = false, busy = false
  let scope: SessionScope | null = null, sessionSignal: AbortSignal | null = null, attempt: AbortController | null = null
  let file: File | null = null, image: PreparedImage | null = null, binding: CommandBinding | null = null, mutated = false
  const listeners = new Set<() => void>()
  const emit = (patch: Partial<UploadState>) => { state = Object.freeze({ ...state, ...patch }); for (const listener of listeners) listener() }
  function release(phase: UploadState['phase']) {
    generation++; busy = false
    const c = attempt; attempt = null
    sessionSignal?.removeEventListener('abort', invalidate); sessionSignal = null
    const url = state.previewUrl
    scope = null; file = null; image = null; binding = null; mutated = false
    state = empty(phase)
    c?.abort(); if (url) previews.revoke(url)
    for (const listener of listeners) listener()
  }
  const invalidate = () => release('cancelled')
  const current = (g: number) => !disposed && generation === g && !!scope && session.isCurrent(scope) && !attempt?.signal.aborted
  function capture() {
    const snapshot: SessionSnapshot = session.read()
    if (!uuid(snapshot.userId) || !uuid(snapshot.eventId) || snapshot.signal.aborted) throw new ImageError('aborted')
    scope = Object.freeze({ userId: snapshot.userId, eventId: snapshot.eventId, generation: snapshot.generation })
    sessionSignal = snapshot.signal; sessionSignal.addEventListener('abort', invalidate, { once: true })
    emit({ scope })
  }
  function newBinding(operation: MediaOperation, mediaId: string | null): CommandBinding {
    const id = key(); if (!uuid(id) || !scope) throw new Error('Invalid command key')
    return Object.freeze({ scope, operation, key: id, mediaId })
  }
  async function prepared(g: number, input: File) {
    attempt = new AbortController(); const signal = attempt.signal
    busy = true; emit({ phase: 'preparing', progress: 0, error: null, attachment: null })
    try {
      // Independent input/output gates also constrain injected processors; never trust declared MIME alone.
      await inspectImage(input, signal)
      if (!current(g)) return
      const result = await abortable(prepare(input, { signal, onProgress: progress => {
        if (current(g) && Number.isFinite(progress)) emit({ progress: Math.max(0, Math.min(100, progress)) })
      } }), signal)
      if (!current(g)) return
      const info = await inspectImage(result.blob, signal)
      if (!current(g)) return
      if (result.width !== info.width || result.height !== info.height || !/^[0-9a-f]{64}$/.test(result.sha256)) throw new ImageError('decode-failed')
      image = Object.freeze(result)
      const previewUrl = previews.create(result.blob)
      if (!current(g)) { previews.revoke(previewUrl); return }
      emit({ phase: 'prepared', previewUrl, progress: 100 })
    } catch (error) {
      if (current(g)) emit({ phase: 'failed', error: error instanceof ImageError ? error.code : 'preparation-failed', progress: null })
    } finally { if (generation === g) { busy = false; attempt = null } }
  }
  function take<T>(result: MediaCommandResult<T>): T {
    if (result.kind === 'pending') {
      decodeReceipt(result.receipt, binding!)
      emit({ phase: 'uncertain', progress: null, error: 'command-pending', attachment: null })
      throw new Error('Pending command')
    }
    return result.value
  }
  return {
    read: () => state,
    subscribe(listener) { listeners.add(listener); return () => listeners.delete(listener) },
    async select(input) {
      if (disposed || busy || state.phase === 'uncertain') return
      release('idle')
      try { capture() } catch { emit({ phase: 'failed', error: 'authentication-required' }); return }
      if (!scope) return
      file = input
      await prepared(generation, input)
    },
    async upload() {
      if (disposed || busy || state.phase !== 'prepared' || !image || !scope || !session.isCurrent(scope)) return
      const g = generation, preparedImage = image
      busy = true; attempt = new AbortController(); const signal = attempt.signal
      try {
        binding = newBinding('initConcertUpload', null)
        emit({ phase: 'initializing', progress: null }); if (!current(g)) return
        mutated = true
        const initialization = await abortable(transport.initialize(binding, purpose, preparedImage, signal), signal)
        if (!current(g)) return
        const intent = decodeIntent(take(initialization), scope!, now())
        binding = newBinding('putConcertUploadBytes', intent.media_id)
        emit({ phase: 'uploading', progress: null }); if (!current(g)) return
        const transfer = await abortable(transport.upload(binding, intent, preparedImage, signal, progress => {
          if (current(g)) emit({ progress: progress === null || !Number.isFinite(progress) ? null : Math.max(0, Math.min(100, progress)) })
        }), signal)
        if (!current(g)) return
        const uploaded = decodeUploaded(take(transfer), intent.media_id)
        if (uploaded.revision <= intent.revision) throw new Error('Invalid media revision')
        binding = newBinding('finalizeConcertUpload', intent.media_id)
        emit({ phase: 'finalizing', progress: null }); if (!current(g)) return
        const finalization = await abortable(transport.finalize(binding, purpose, uploaded.revision, signal), signal)
        if (!current(g)) return
        const media = decodeMedia(take(finalization), scope!, intent.media_id, purpose)
        if (media.revision <= uploaded.revision) throw new Error('Invalid media revision')
        image = null; file = null; binding = null
        emit({ phase: 'ready', progress: 100, attachment: media, error: null })
      } catch (error) {
        if (current(g) && (state as UploadState).phase !== 'uncertain') {
          const definitive = !mutated || error instanceof TransportError && error.kind === 'http' && !error.uncertain
          emit({ phase: definitive ? 'failed' : 'uncertain', error: definitive ? (error instanceof TransportError ? error.server?.reason : null) ?? 'upload-failed' : 'outcome-unknown', attachment: null, progress: null })
        }
      } finally { if (generation === g) { busy = false; attempt = null } }
    },
    async recover() {
      if (disposed || busy || !scope || !session.isCurrent(scope) || !binding || !['uncertain', 'failed'].includes(state.phase)) return
      const b = binding, g = generation
      attempt = new AbortController(); const signal = attempt.signal; busy = true
      try {
        const r = decodeReceipt(await abortable(transport.status(b, signal), signal), b)
        if (!current(g)) return
        if (Date.parse(r.expires_at) <= now() || r.state === 'RETIRED') { emit({ phase: 'uncertain', error: 'receipt-retired' }); return }
        if (r.state === 'FAILED') { emit({ phase: 'failed', error: r.failure_reason, attachment: null }); return }
        if (r.state !== 'SUCCEEDED') { emit({ phase: 'uncertain', error: 'command-pending' }); return }
        // A successful receipt is metadata only. Current authorized READY resource must be reread.
        binding = Object.freeze({ ...b, mediaId: r.resource_id! })
        const media = decodeMedia(await abortable(transport.read(binding, purpose, signal), signal), scope!, r.resource_id!, purpose)
        if (!current(g)) return
        if (media.revision < r.resource_revision!) throw new Error('Stale media')
        image = null; file = null; binding = null
        emit({ phase: 'ready', attachment: media, progress: 100, error: null })
      } catch { if (current(g)) emit({ phase: 'uncertain', error: 'recovery-unavailable', attachment: null }) }
      finally { if (generation === g) { busy = false; attempt = null } }
    },
    async retry() {
      if (disposed || busy || mutated || !file || state.phase !== 'failed' || !scope || !session.isCurrent(scope)) return
      await prepared(generation, file)
    },
    cancel: () => release('cancelled'),
    dispose() { if (disposed) return; disposed = true; release('cancelled'); listeners.clear() },
  }
}
