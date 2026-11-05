import '../../features/social-sharing/ui/parade-ink-private/private.css'
import { useEffect, useSyncExternalStore } from 'react'
import { createSocialSharingController, HandleSettings, SharingOffer, SharingRequest, SharingRequests } from '../../features/social-sharing'
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
    let view: Readonly<{ path: '/me/social' | '/me/requests'; detail: string | null; offer: { readonly introId: string; readonly recipientId: string; readonly revision: number } | null; locked: boolean; version: number }> = Object.freeze({ path: '/me/requests', detail: null, offer: null, locked: false, version: 0 })
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
          else if (current.offer) { void controller.loadHandles(); void controller.loadOffer(current.offer.introId) }
          else if (current.detail) void controller.loadRequest(current.detail)
          else void controller.loadRequests()
        }
        return controller.hideSecret
      }, [current.path, current.detail, current.offer, state.available])
      return <div className="social-sharing-entry pi-private-social">
        <nav aria-label="Private sharing sections">
          <ShellLink id="sharing-handles" to="/me/social" aria-current={current.path === '/me/social' ? 'page' : undefined}>Social links</ShellLink>
          <ShellLink id="sharing-requests" to="/me/requests" aria-current={current.path === '/me/requests' ? 'page' : undefined}>Requests</ShellLink>
        </nav>
        {current.path === '/me/social' ? <HandleSettings controller={controller} /> : current.offer ? <>
          <button type="button" disabled={current.locked} onClick={() => { if (!view.locked) { controller.hideSecret(); publish({ offer: null }) } }}>Back to requests</button>
          {state.offer.phase !== 'ready' || state.offer.data?.intro_id === current.offer.introId && state.offer.data.recipient_id === current.offer.recipientId && state.offer.data.intro_revision === current.offer.revision
            ? <SharingOffer controller={controller} retry={() => { void controller.loadOffer(current.offer!.introId) }} />
            : <p role="status">This introduction changed. Return to the current profile before requesting sharing.</p>}
        </> : current.detail ? <>
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
      enter(path) { if (!disposed && !view.locked && view.path !== path) { controller.hideSecret(); publish({ path, detail: null, offer: null }) } },
      offer(target) {
        if (!disposed && !view.locked && canonicalId(target.introId) && canonicalId(target.recipientId) && target.recipientId !== scope.userId && Number.isSafeInteger(target.revision) && target.revision > 0) {
          controller.hideSecret(); publish({ path: '/me/requests', detail: null, offer: Object.freeze({ ...target }) })
        }
      },
      dispose() { if (disposed) return; disposed = true; stop(); listeners.clear(); controller.dispose() },
    }
    return module
  },
}
