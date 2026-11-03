import { useEffect, useSyncExternalStore } from 'react'
import { createSocialSharingController, HandleSettings, SharingRequest, SharingRequests } from '../../features/social-sharing'
import { ShellLink } from '../navigation'
import { canonicalId } from '../routes'
import type { RequestsModule, SocialOptions } from './ports'

/** Adapts the qualified FE31 controller to the existing shell owner, not a new session/cache. */
export const canonicalSharingRequests: NonNullable<SocialOptions['requests']> = {
  create({ auth, scope, access }) {
    if (!auth.privacy.isCurrent(scope)) throw new Error('Sharing lease unavailable')
    const controller = createSocialSharingController({ auth, access,
      ...(typeof auth.requestSensitive === 'function' ? { sensitiveRead: auth.requestSensitive } : {}),
    })
    let disposed = false, started = false
    let view: Readonly<{ path: '/me/social' | '/me/requests'; detail: string | null; locked: boolean; version: number }> = Object.freeze({ path: '/me/requests', detail: null, locked: false, version: 0 })
    const listeners = new Set<() => void>()
    const publish = (patch: Partial<typeof view> = {}) => {
      if (disposed) return
      view = Object.freeze({ ...view, ...patch, version: view.version + 1 })
      for (const listener of Array.from(listeners)) { try { listener() } catch { /* Isolate shell observers. */ } }
    }
    const stop = controller.subscribe(() => publish({ locked: ['sending', 'pending', 'uncertain', 'verifying', 'blocked'].includes(controller.read().mutation.phase) }))
    const read = () => view
    const subscribe = (listener: () => void) => { if (disposed) return () => {}; listeners.add(listener); return () => { listeners.delete(listener) } }
    function Panel() {
      const current = useSyncExternalStore(subscribe, read, read)
      const state = useSyncExternalStore(controller.subscribe, controller.read, controller.read)
      useEffect(() => {
        if (state.available) {
          if (current.path === '/me/social') { if (controller.read().handles.phase === 'idle') void controller.loadHandles() }
          else if (current.detail) void controller.loadRequest(current.detail)
          else void controller.loadRequests()
        }
        return controller.hideSecret
      }, [current.path, current.detail, state.available])
      return <div className="social-sharing-entry">
        <nav aria-label="Private sharing sections">
          <ShellLink id="sharing-handles" to="/me/social" aria-current={current.path === '/me/social' ? 'page' : undefined}>Social links</ShellLink>
          <ShellLink id="sharing-requests" to="/me/requests" aria-current={current.path === '/me/requests' ? 'page' : undefined}>Requests</ShellLink>
        </nav>
        {current.path === '/me/social' ? <HandleSettings controller={controller} /> : current.detail ? <>
          <button type="button" disabled={current.locked} onClick={() => { if (!view.locked && !disposed) { controller.hideSecret(); publish({ detail: null }) } }}>Back to requests</button>
          <SharingRequest controller={controller} retry={() => { void controller.loadRequest(current.detail) }} />
        </> : <SharingRequests controller={controller} onRequest={id => {
          if (!disposed && !view.locked && canonicalId(id) && controller.read().requests.data?.items.some(item => item.request_id === id)) publish({ detail: id })
        }} />}
      </div>
    }
    const module: RequestsModule = {
      read, subscribe, Panel,
      start() { if (!disposed && !started) { started = true; controller.start() } },
      enter(path) { if (!disposed && !view.locked && view.path !== path) { controller.hideSecret(); publish({ path, detail: null }) } },
      dispose() { if (disposed) return; disposed = true; stop(); listeners.clear(); controller.dispose() },
    }
    return module
  },
}
