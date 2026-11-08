/**
 * WP-07 T-606: the client side of the conversion worker.
 *
 * The importer never runs the conversion on the UI thread. It asks this client,
 * which owns one worker for the lifetime of the screen, transfers the source
 * bytes to it and receives the encoded poster. A worker that fails to start is
 * reported as unavailable; the conversion is never silently run on the main
 * thread as a fallback, because that is exactly the freeze this exists to avoid.
 */
import { convertToPoster, type Crop, type PosterPreset } from '../pixelize'
import { handleConvert, type ConvertReply, type ConvertRequest } from '../worker/convert'

/** The conversion port: what the importer needs and nothing more. */
export interface PosterConverter {
  convert(source: Blob, crop: Crop, preset: PosterPreset): Promise<{ readonly blob: Blob; readonly side: number }>
  dispose(): void
}

export class ConverterUnavailableError extends Error {
  constructor() {
    super('The poster converter is unavailable.')
    this.name = 'ConverterUnavailableError'
  }
}

/** A real worker, created lazily and terminated on dispose. */
export function createWorkerConverter(): PosterConverter {
  let worker: Worker | null = null
  let next = 1
  const pending = new Map<number, { resolve: (reply: ConvertReply) => void; reject: (error: unknown) => void }>()
  let disposed = false

  function start(): Worker {
    if (worker) return worker
    if (disposed || typeof Worker === 'undefined') throw new ConverterUnavailableError()
    worker = new Worker(new URL('../worker/convert.worker.ts', import.meta.url), { type: 'module', name: 'mcr-poster-conversion' })
    worker.addEventListener('message', (event: MessageEvent<ConvertReply>) => {
      const reply = event.data
      const waiting = reply && pending.get(reply.id)
      if (!waiting) return
      pending.delete(reply.id)
      waiting.resolve(reply)
    })
    worker.addEventListener('error', () => {
      const waiting = Array.from(pending.values())
      pending.clear()
      for (const entry of waiting) entry.reject(new ConverterUnavailableError())
    })
    return worker
  }

  return {
    async convert(source, crop, preset) {
      const active = start()
      const id = next++
      const bytes = await source.arrayBuffer()
      const reply = await new Promise<ConvertReply>((resolve, reject) => {
        pending.set(id, { resolve, reject })
        const request: ConvertRequest = { id, type: 'convert', png: bytes, crop, preset }
        active.postMessage(request, [bytes])
      })
      if (reply.type === 'failed') throw new ConverterUnavailableError()
      return { blob: new Blob([reply.png], { type: 'image/png' }), side: reply.width }
    },
    dispose() {
      if (disposed) return
      disposed = true
      for (const entry of pending.values()) entry.reject(new ConverterUnavailableError())
      pending.clear()
      worker?.terminate()
      worker = null
    },
  }
}

/**
 * A direct converter that runs the same conversion on the calling thread.
 *
 * It exists for two callers only: the test suite, which must assert determinism
 * without a browser worker, and a host that has no Worker constructor at all
 * (older embedded webviews). It is never chosen as a *fallback* for a worker
 * that failed: an unavailable worker is reported, not silently downgraded.
 */
export function createDirectConverter(): PosterConverter {
  return {
    async convert(source, crop, preset) {
      const { blob, pixels } = await convertToPoster(source, crop, preset)
      return { blob, side: pixels.side }
    },
    dispose() {},
  }
}

/** The production converter: a worker, or the direct path where no worker exists. */
export function createPosterConverter(): PosterConverter {
  if (typeof Worker === 'undefined') return createDirectConverter()
  return createWorkerConverter()
}

export { handleConvert }
