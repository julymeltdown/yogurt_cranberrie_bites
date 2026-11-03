import type { SessionController } from '../../features/auth/model/session'
import type { PrivacyScope } from '../../shared/query/privacy'
import type { ShellBootstrap } from '../ports'

export interface CharacterRuntimeLifetime { isCurrent(): boolean; dispose(): void }
export interface CharacterRuntimeInput {
  readonly scope: PrivacyScope
  readonly manifest: Readonly<Record<string, unknown>>
  readonly pin: Readonly<{ manifest_id: string; manifest_version: number; content_hash: string }>
}
const same = (a: PrivacyScope, b: PrivacyScope) => a.generation === b.generation && a.userId === b.userId
  && a.eventId === b.eventId && a.policyGeneration === b.policyGeneration && a.gate === b.gate
const record = (value: unknown): value is Record<string, unknown> => value !== null && typeof value === 'object' && !Array.isArray(value)
const pinKey = (pin: CharacterRuntimeInput['pin']) => JSON.stringify([pin.manifest_id, pin.manifest_version, pin.content_hash])

/** The application owns this registry; views and World borrow its runtime.
 * The factory remains responsible for reviewed manifest/hash/art validation.
 * No asset, profile or mutation request is started by the registry itself.
 */
export function createCharacterRuntimeRegistry<T extends CharacterRuntimeLifetime>({ auth, create }: {
  readonly auth: Pick<SessionController, 'privacy' | 'session'>
  readonly create: (input: CharacterRuntimeInput) => T
}) {
  let closed = false
  let active: { scope: PrivacyScope; key: string; runtime: T } | null = null
  function clear() {
    const previous = active; active = null
    try { previous?.runtime.dispose() } catch { /* Withdrawal must finish even if a sink fails. */ }
  }
  const unregister = auth.privacy.registerCleanup(clear)
  const current = (scope: PrivacyScope) => !closed && auth.session.isCurrent(scope) && auth.privacy.isCurrent(scope)
    && same(scope, auth.privacy.capture())
  return {
    /** World may borrow only a runtime already admitted by the shell bootstrap. */
    read(scope: PrivacyScope, pin: CharacterRuntimeInput['pin']): T | null {
      return current(scope) && active && same(active.scope, scope) && active.key === pinKey(pin)
        && active.runtime.isCurrent() ? active.runtime : null
    },
    get(scope: PrivacyScope, bootstrap: ShellBootstrap): T | null {
      if (!current(scope) || bootstrap.event.eventId !== scope.eventId || bootstrap.participant.eventId !== scope.eventId
        || bootstrap.participant.userId !== scope.userId) return null
      const manifest = bootstrap.document?.avatar_manifest
      if (!record(manifest) || manifest.contract_version !== '1.1.0' || manifest.representation !== 'whole_character'
        || typeof manifest.manifest_id !== 'string' || !Number.isSafeInteger(manifest.manifest_version) || Number(manifest.manifest_version) < 1
        || typeof manifest.content_hash !== 'string' || !/^sha256:[a-f0-9]{64}$/.test(manifest.content_hash)) return null
      const pin = Object.freeze({ manifest_id: manifest.manifest_id, manifest_version: Number(manifest.manifest_version), content_hash: manifest.content_hash })
      const key = pinKey(pin)
      if (active && same(active.scope, scope) && active.key === key && active.runtime.isCurrent()) return active.runtime
      clear()
      let runtime: T
      try { runtime = create({ scope: Object.freeze({ ...scope }), manifest: structuredClone(manifest), pin }) } catch { return null }
      // A constructor can synchronously trigger privacy/identity replacement.
      if (!current(scope) || !runtime.isCurrent()) { runtime.dispose(); return null }
      active = { scope: Object.freeze({ ...scope }), key, runtime }
      return runtime
    },
    dispose() { if (closed) return; closed = true; unregister(); clear() },
  }
}
