import auth from '../../../../../../contracts/fixtures/core/auth-success.json'
import disabled from '../../../../../../contracts/fixtures/core/auth-capabilities-disabled.json'
import bootstrap from '../../../../../../contracts/fixtures/core/bootstrap-open.json'
import type { CookiePolicy } from '../../model/cookie-policy'
export const fixture = () => structuredClone(bootstrap)
export const authResponse = () => structuredClone(auth)
export const eventId = bootstrap.event.event_id
export const userId = auth.userId
export const capabilities = () => ({ ...structuredClone(disabled), providers: disabled.providers.map(p => p.provider === 'EMAIL'
  ? { ...p, enabled: true, disabled_reason: null } : { ...p }) })
export function isolatedCookiePolicy(): CookiePolicy & { invalidate(): void } {
  const listeners = new Set<() => void>()
  return { available: true, onInvalidate(listener) { listeners.add(listener); return () => { listeners.delete(listener) } },
    run: async (_kind, work) => work(), dispose: () => listeners.clear(), invalidate: () => { for (const listener of listeners) listener() } }
}
export function deferred<T>() {
  let resolve!: (value: T) => void
  const promise = new Promise<T>(yes => { resolve = yes })
  return { promise, resolve }
}
