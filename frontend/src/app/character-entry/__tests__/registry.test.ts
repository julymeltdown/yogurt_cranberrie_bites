import { afterEach, expect, it, vi } from 'vitest'
import { QueryClient } from '@tanstack/react-query'
import { createMemorySession } from '../../../shared/api/http'
import { createQueryPrivacy } from '../../../shared/query/privacy'
import { createCharacterRuntimeRegistry, type CharacterRuntimeInput } from '../registry'
import { bootstrapFor } from '../../__tests__/fixtures/ports'
import manifest from '../../../../../contracts/fixtures/assets/selected-static/avatar-manifest.json'

const cleanup: (() => void)[] = []
afterEach(() => cleanup.splice(0).reverse().forEach(f => f()))
function fixture() {
  const session = createMemorySession(), client = new QueryClient()
  session.replace(0, { userId: '11111111-1111-4111-8111-111111111111', eventId: '22222222-2222-4222-8222-222222222222', accessToken: 'fixture' })
  const privacy = createQueryPrivacy({ session, client }), scope = privacy.capture()
  const bootstrap = { ...bootstrapFor(scope as Parameters<typeof bootstrapFor>[0]), document: { avatar_manifest: manifest } }
  cleanup.push(() => { privacy.dispose(); client.clear() })
  const create = vi.fn((_input: CharacterRuntimeInput) => ({ isCurrent: () => true, dispose: vi.fn() }))
  const registry = createCharacterRuntimeRegistry({ auth: { session, privacy }, create })
  cleanup.push(registry.dispose)
  return { session, privacy, scope, bootstrap, create, registry }
}
it('lends one runtime across views without changing the canonical privacy owner', () => {
  const f = fixture(), first = f.registry.get(f.scope, f.bootstrap)
  expect(first).not.toBeNull()
  expect(f.registry.get(f.scope, structuredClone(f.bootstrap))).toBe(first)
  expect(f.create).toHaveBeenCalledTimes(1)
  expect(f.create.mock.calls[0]?.[0]).toEqual({ scope: f.scope, manifest, pin: { manifest_id: manifest.manifest_id, manifest_version: 1, content_hash: manifest.content_hash } })
})
it('policy withdrawal destroys the shared runtime synchronously and rejects old scope reuse', () => {
  const f = fixture(), runtime = f.registry.get(f.scope, f.bootstrap)!
  f.privacy.changePolicy(f.scope, 'block')
  expect(runtime.dispose).toHaveBeenCalledTimes(1)
  expect(f.registry.get(f.scope, f.bootstrap)).toBeNull()
  expect(f.registry.get(f.privacy.capture(), f.bootstrap)).not.toBe(runtime)
})
it('rejects cross-user bootstrap data before allocating a runtime', () => {
  const f = fixture()
  expect(f.registry.get(f.scope, { ...f.bootstrap, participant: { ...f.bootstrap.participant, userId: '33333333-3333-4333-8333-333333333333' } })).toBeNull()
  expect(f.create).not.toHaveBeenCalled()
})
it('rejects a runtime created during synchronous identity replacement and releases it', () => {
  const f = fixture(), runtime = { isCurrent: () => true, dispose: vi.fn() }
  const registry = createCharacterRuntimeRegistry({ auth: f, create: () => { f.session.replace(f.scope.generation, null); return runtime } })
  cleanup.push(registry.dispose)
  expect(registry.get(f.scope, f.bootstrap)).toBeNull()
  expect(runtime.dispose).toHaveBeenCalledTimes(1)
})
