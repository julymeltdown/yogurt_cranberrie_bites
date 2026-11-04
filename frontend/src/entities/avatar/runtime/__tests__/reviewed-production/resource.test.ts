import { createHash } from 'node:crypto'
import { readFile } from 'node:fs/promises'
import { URL as FileURL } from 'node:url'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import pack from '../../../character-source-pack.json'
import { setup } from '../../../../../features/avatar/__tests__/reviewed-production/fixtures'
import { createMotionRuntime } from '../../motion-adoption/runtime'
import { MOTION_ART } from '../../motion-adoption/art'

const cleanup: (() => void)[] = []
beforeEach(() => vi.stubGlobal('location', { origin: 'https://mcr.test' }))
afterEach(() => { cleanup.splice(0).reverse().forEach(fn => fn()); vi.unstubAllGlobals() })
const publicFile = (path: string) => new FileURL(`../../../../../../public/${path}`, import.meta.url)
it('revokes a public runtime after an actual resource hash mismatch, exposing no catalog or texture', async () => {
  const f = await setup(); cleanup.push(f.dispose)
  const authority = await f.create(), bytes = new Uint8Array(await readFile(publicFile(MOTION_ART.descriptor.path)))
  bytes[0] = bytes[0]! ^ 1
  const runtime = createMotionRuntime({ authority, scope: f.scope, privacy: f.auth.privacy, origin: 'https://mcr.test',
    fetch: async input => {
      const response = new Response(bytes, { headers: { 'Content-Type': 'application/json', 'Content-Length': String(bytes.byteLength) } })
      Object.defineProperty(response, 'url', { value: String(input) })
      return response
    },
  }); cleanup.push(runtime.dispose)
  await expect(runtime.catalog(f.signal.signal)).rejects.toThrow()
  expect(runtime.diagnostics()).toMatchObject({ disposed: true, decodedAtlases: 0, leases: 0, composition: null })
  expect(authority.isCurrent()).toBe(false)
})
it('preserves all selected originals and frozen derivative bytes without asserting cross-family pixel identity', async () => {
  for (const file of pack.files) {
    const bytes = await readFile(publicFile(`world/avatars/supplied-v1/${file.file}`))
    expect(bytes.byteLength).toBe(file.byte_length)
    expect(createHash('sha256').update(bytes).digest('hex')).toBe(file.sha256)
  }
  for (const resource of [MOTION_ART.descriptor, MOTION_ART.atlas]) {
    const bytes = await readFile(publicFile(resource.path))
    expect(bytes.byteLength).toBe(resource.bytes)
    expect(`sha256:${createHash('sha256').update(bytes).digest('hex')}`).toBe(resource.sha256)
  }
  const staticAtlas = await readFile(publicFile('world/avatars/derived-v1/photo01-front-idle-64-e124c88815b22220.png'))
  expect(createHash('sha256').update(staticAtlas).digest('hex')).toBe('e124c88815b22220df2f2dc59e2f26b3ceb845e57b77674fd8e55383116d7efe')
})
