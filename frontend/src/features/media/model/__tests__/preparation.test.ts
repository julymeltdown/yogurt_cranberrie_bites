import { afterEach, expect, it, vi } from 'vitest'
import { prepareImage } from '../../../../shared/media/prepare-image'
import { file } from './fixtures'
class FixtureWorker {
  onmessage: ((event: unknown) => void) | null = null
  onerror: (() => void) | null = null
  onmessageerror: (() => void) | null = null
  terminate = vi.fn()
  postMessage = vi.fn()
  static instances: FixtureWorker[] = []
  constructor(readonly url: URL, readonly options: WorkerOptions) { FixtureWorker.instances.push(this) }
}
afterEach(() => { vi.unstubAllGlobals(); vi.useRealTimers(); FixtureWorker.instances = [] })
function start() {
  vi.stubGlobal('Worker', FixtureWorker)
  const c = new AbortController(), progress = vi.fn()
  const result = prepareImage(file(), { signal: c.signal, onProgress: progress }).then(value => value, error => error)
  return { c, progress, result }
}
it('uses only an owned module worker and releases handlers/resources on decode failure', async () => {
  const h = start(); await vi.waitFor(() => expect(FixtureWorker.instances).toHaveLength(1))
  const w = FixtureWorker.instances[0]!
  expect(w.options).toEqual({ type: 'module' }); expect(w.url.pathname).toMatch(/image-compression.worker.ts$/)
  w.onerror!(); expect(await h.result).toMatchObject({ code: 'decode-failed' }); expect(w.terminate).toHaveBeenCalled()
  expect(w.onmessage).toBeNull(); expect(w.onerror).toBeNull(); expect(w.onmessageerror).toBeNull()
})
it('terminates owned decoding and settles immediately on abort', async () => {
  const h = start(); await vi.waitFor(() => expect(FixtureWorker.instances).toHaveLength(1))
  const w = FixtureWorker.instances[0]!, late = w.onmessage
  h.c.abort(); expect(await h.result).toMatchObject({ code:'aborted' }); expect(w.terminate).toHaveBeenCalled()
  late?.({data:{progress:99}}); expect(h.progress).not.toHaveBeenCalled()
})
it('bounds silent decoder stalls and clears its timer', async () => {
  vi.useFakeTimers()
  const h = start()
  for (let n = 0; n < 20 && !FixtureWorker.instances.length; n++) await Promise.resolve()
  expect(FixtureWorker.instances).toHaveLength(1)
  const w = FixtureWorker.instances[0]!
  await vi.advanceTimersByTimeAsync(30_000)
  expect(await h.result).toMatchObject({ code: 'decode-failed' }); expect(w.terminate).toHaveBeenCalled()
  expect(vi.getTimerCount()).toBe(0)
})
