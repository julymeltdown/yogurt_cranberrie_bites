import { prepareImage } from '../../../../shared/media/prepare-image'
import { inspectImage } from '../../../../shared/media/image-policy'
import { browserPreviews } from '../../../../shared/media/preview'
import { uploadPresignedImage } from '../../../../shared/media/presigned-upload'
const NativeWorker = window.Worker
let liveWorkers = 0, peakWorkers = 0, terminatedWorkers = 0
class TrackedWorker extends NativeWorker {
  private alive = true
  constructor(url: string | URL, options?: WorkerOptions) { super(url, options); liveWorkers++; peakWorkers = Math.max(peakWorkers, liveWorkers) }
  override terminate() { if (this.alive) { this.alive = false; liveWorkers--; terminatedWorkers++ }; super.terminate() }
}
window.Worker = TrackedWorker
const fixture = {
  async compression() {
    const canvas = document.createElement('canvas'); canvas.width = 3200; canvas.height = 1800
    const ctx = canvas.getContext('2d')!; ctx.fillStyle = '#235'; ctx.fillRect(0, 0, canvas.width, canvas.height)
    const jpeg = await new Promise<Blob>(resolve => canvas.toBlob(b => resolve(b!), 'image/jpeg', 0.95))
    const original = new Uint8Array(await jpeg.arrayBuffer())
    // Synthetic APP1 metadata marker. Browser decode accepts it; output must not retain it.
    const secret = new TextEncoder().encode('Exif\0\0II*\0\x08\0\0\0\0\0\0\0\0\0FE04_SYNTHETIC_PRIVATE_METADATA')
    const tag = new Uint8Array(secret.length + 4); tag.set([0xff, 0xe1, (secret.length + 2) >> 8, (secret.length + 2) & 255]); tag.set(secret, 4)
    const file = new File([original.slice(0, 2), tag, original.slice(2)], 'private-camera.jpg', { type: 'image/jpeg' })
    const progress: number[] = []
    const result = await prepareImage(file, { signal: new AbortController().signal, onProgress: p => progress.push(p) })
    const info = await inspectImage(result.blob, new AbortController().signal)
    const output = new Uint8Array(await result.blob.arrayBuffer())
    const digest = await crypto.subtle.digest('SHA-256', output)
    const actualHash = Array.from(new Uint8Array(digest), n => n.toString(16).padStart(2, '0')).join('')
    const url = browserPreviews.create(result.blob), preview = new Image(); preview.src = url; await preview.decode()
    browserPreviews.revoke(url)
    let revoked = false; try { await fetch(url) } catch { revoked = true }
    canvas.width = canvas.height = 0; preview.src = ''
    return { ...info, bytes: output.length, originalBytes: file.size, sha256Matches: actualHash === result.sha256,
      metadataAbsent: !new TextDecoder().decode(output).includes('FE04_SYNTHETIC_PRIVATE_METADATA'),
      progressCompleted: progress.at(-1) === 100, revoked, liveWorkers, peakWorkers, terminatedWorkers }
  },
  async abort() {
    const canvas = document.createElement('canvas'); canvas.width = canvas.height = 200
    const blob = await new Promise<Blob>(resolve => canvas.toBlob(b => resolve(b!), 'image/png'))
    const file = new File([blob], 'test.png', { type: 'image/png' }), c = new AbortController()
    const promise = prepareImage(file, { signal: c.signal, onProgress() {} }).then(() => 'unexpected-success', e => e.code)
    // Wait for the own worker to exist, then abort before it returns.
    const start = performance.now()
    while (liveWorkers === 0 && performance.now() - start < 1000) await new Promise(resolve => setTimeout(resolve, 0))
    c.abort(); const result = await promise; canvas.width = canvas.height = 0
    return { result, liveWorkers, terminatedWorkers }
  },
  async presigned() {
    await uploadPresignedImage({ url: 'https://storage.mcr-fixture.invalid/upload?signature=fixture', allowedOrigins: ['https://storage.mcr-fixture.invalid'],
      blob: new Blob(['fixture'], { type: 'image/png' }), signal: new AbortController().signal })
  },
}
Object.assign(window, { mediaFixture: fixture })
