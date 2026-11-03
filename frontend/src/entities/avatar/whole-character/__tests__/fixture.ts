import { readFile } from 'node:fs/promises'
import { vi } from 'vitest'
import manifest from '../../../../../../contracts/fixtures/assets/selected-static/avatar-manifest.json'
import { createSessionController } from '../../../../features/auth/model/session'
import { createWholeCharacterRuntime, type WholeCharacterRuntimeOptions } from '../runtime'
import { REVIEWED } from '../contract'
export { manifest }
export const config = manifest.presets[0]!.config
export const pin = { manifest_id: manifest.manifest_id, manifest_version: manifest.manifest_version, content_hash: manifest.content_hash }
export const publicRoot = new URL('../../../../../public/', import.meta.url)
export async function resource(url: string) {
  const path = new URL(url).pathname.slice(1), data = await readFile(new URL(path, publicRoot))
  const response = new Response(data, { headers: { 'content-type': path.endsWith('.json') ? 'application/json' : 'image/png', 'content-length': String(data.byteLength) } })
  Object.defineProperty(response, 'url', { value: url, configurable: true }); return response
}
export function fixture(patch: Partial<WholeCharacterRuntimeOptions> = {}) {
  const eventId = '00000000-0000-4000-8000-000000000001', userId = '00000000-0000-4000-8000-000000000002'
  const auth = createSessionController({ eventId, cookiePolicy: { available: true, onInvalidate: () => () => {}, run: (_kind, work) => work(), dispose() {} } })
  auth.session.replace(0, { eventId, userId, accessToken: 'fixture-memory-token' })
  const fetcher = vi.fn<typeof fetch>(async input => resource(String(input))), disposed: ReturnType<typeof vi.fn>[] = []
  const decode: WholeCharacterRuntimeOptions['decode'] = vi.fn(async bytes => {
    const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength), dispose = vi.fn(); disposed.push(dispose)
    return { image: { naturalWidth: view.getUint32(16), naturalHeight: view.getUint32(20) } as HTMLImageElement, dispose }
  })
  const runtime = createWholeCharacterRuntime({ privacy: auth.privacy, manifest, pin, origin: 'https://fixture.invalid', fetch: fetcher, decode, ...patch })
  return { auth, runtime, fetcher, decode, disposed, signal: () => new AbortController().signal, dispose() { runtime.dispose(); auth.dispose() } }
}
export const deferred = <T>() => { let resolve!: (v: T) => void; const promise = new Promise<T>(done => { resolve = done }); return { promise, resolve } }
export const atlas64 = REVIEWED.texture
