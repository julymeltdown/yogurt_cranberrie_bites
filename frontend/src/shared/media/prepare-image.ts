import { checkAbort, inspectImage, ImageError } from './image-policy'
import type { PreparedImage } from './types'
export interface ImagePreparationOptions { readonly signal: AbortSignal; readonly onProgress: (percent: number) => void }
export type PrepareImage = (file: File, options: ImagePreparationOptions) => Promise<PreparedImage>
/** No CDN worker or shared decoder: terminate the owned same-origin worker on every exit. */
export const prepareImage: PrepareImage = async (file, { signal, onProgress }) => {
  await inspectImage(file, signal); checkAbort(signal)
  const worker = new Worker(new URL('./image-compression.worker.ts', import.meta.url), { type: 'module' })
  let abort: (() => void) | undefined
  let timer: ReturnType<typeof setTimeout> | undefined
  try {
    const blob = await new Promise<Blob>((resolve, reject) => {
      abort = () => { worker.terminate(); reject(new ImageError('aborted')) }
      signal.addEventListener('abort', abort, { once: true })
      worker.onerror = worker.onmessageerror = () => reject(new ImageError('decode-failed'))
      timer = setTimeout(() => reject(new ImageError('decode-failed')), 30_000)
      worker.onmessage = (event: MessageEvent<{ progress?: number; blob?: Blob; error?: string }>) => {
        if (signal.aborted) return
        if (event.data.blob instanceof Blob) resolve(event.data.blob)
        else if (typeof event.data.progress === 'number' && Number.isFinite(event.data.progress)) onProgress(Math.max(0, Math.min(90, event.data.progress * 0.9)))
        else reject(new ImageError('decode-failed'))
      }
      worker.postMessage(file)
      if (signal.aborted) abort()
    })
    checkAbort(signal)
    const info = await inspectImage(blob, signal)
    const bytes = await blob.arrayBuffer(); checkAbort(signal)
    const hash = await crypto.subtle.digest('SHA-256', bytes); checkAbort(signal)
    const sha256 = Array.from(new Uint8Array(hash), n => n.toString(16).padStart(2, '0')).join('')
    onProgress(100)
    return Object.freeze({ blob, width: info.width, height: info.height, sha256 })
  } finally {
    clearTimeout(timer)
    if (abort) signal.removeEventListener('abort', abort)
    worker.onmessage = worker.onerror = worker.onmessageerror = null; worker.terminate()
  }
}
