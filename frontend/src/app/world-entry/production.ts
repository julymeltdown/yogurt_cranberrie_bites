import type { WorldLifecyclePort, WorldLease, WorldAccessBinding, WorldAccessBindingPort } from '../ports'
import type { SessionController } from '../../features/auth/model/session'
import type { ReplicationReader } from '../../features/world-rooms'
import type { WorldComposition, WorldRendererPort } from '../../world/composition'
import type { LocalInputConfiguration } from './resolver'
import { validateLocalInputConfiguration } from './configuration'
import type { RootCharacterRegistry } from '../character-entry/runtime'
import type { MuralProductionConfiguration } from './mural-production/configuration'
import { projectWorldLifecycle } from '../errors/fe41-lifecycle/status'

export interface ProductionWorldOptions {
  readonly auth: SessionController
  readonly localInput?: LocalInputConfiguration | null
  readonly origin?: string
  /** Explicit reviewed virtual placement; server tools reads independently qualify eligibility. */
  readonly mural?: MuralProductionConfiguration | null
  /** Borrow the application owner; a World lease never destroys it. */
  readonly characters?: RootCharacterRegistry
  /** Test seam. The default loads the actual production composition and resolver only on mount. */
  readonly load?: (options: { auth: SessionController; origin: string; localInput: LocalInputConfiguration; characters?: RootCharacterRegistry; mural?: MuralProductionConfiguration | null }) => Promise<WorldComposition>
}
async function loadProduction({ auth, origin, localInput, characters, mural }: { auth: SessionController; origin: string; localInput: LocalInputConfiguration; characters?: RootCharacterRegistry; mural?: MuralProductionConfiguration | null }) {
  const [{ createWorldComposition }, { createWorldEntryResolver }, { createWholeCharacterWorldRenderer }, { createRootSceneOverview }, { createMotionWorldRenderer }, { createProductionMuralFactory }] = await Promise.all([
    import('../../world/composition'), import('./resolver'), import('../../world/avatar/whole-character'),
    import('./reference-scene-overview'), import('../../world/avatar/motion-adoption/renderer'), import('./mural-production/factory'),
  ])
  const pinFor = (manifest: Readonly<Record<string, unknown>>) => {
    if (typeof manifest.manifest_id !== 'string' || typeof manifest.manifest_version !== 'number'
      || typeof manifest.content_hash !== 'string') throw new Error('Character release unavailable')
    return { manifest_id: manifest.manifest_id, manifest_version: manifest.manifest_version, content_hash: manifest.content_hash }
  }
  const whole = characters ? createWholeCharacterWorldRenderer({ resolveRuntime(context) {
    const runtime = characters.read(auth.privacy.capture(), pinFor(context.release.avatarManifest))
    if (!runtime) throw new Error('Character authority unavailable')
    return runtime
  } }) : null
  const reviewed = characters ? createMotionWorldRenderer({ qualification: 'REVIEWED_PRODUCTION', resolveRuntime(context) {
    const owner = characters.readReviewed(auth.privacy.capture(), pinFor(context.release.avatarManifest))
    if (!owner) throw new Error('Reviewed character authority unavailable')
    return owner.runtime
  } }) : null
  const muralFactory = createProductionMuralFactory({ auth, configuration: mural })
  return createWorldComposition({ auth, motionProfileVersion: '1.0.0', ...(muralFactory ? { mural: muralFactory } : {}), resolveRelease: createWorldEntryResolver({ auth, origin, localInput, ...(characters ? { characters } : {}) }), createOverview: createRootSceneOverview,
    ...(whole && reviewed ? { renderer: { mount(context: Parameters<WorldRendererPort['mount']>[0]) {
      const representation = context.release.avatarManifest.representation
      if (representation === 'whole_character') return whole.mount(context)
      if (representation === 'reviewed_selected_source_motion') return reviewed.mount(context)
      throw new Error('Character family unavailable')
    } } } : {}),
  })
}
/** One lazy composition per canonical application Auth; no resource defaults or renderer fallback. */
export function createProductionWorld(options: ProductionWorldOptions) {
  const { auth } = options, configuration = validateLocalInputConfiguration(options.localInput)
  const listeners = new Set<() => void>(), observers = new Set<() => void>()
  let binding: WorldAccessBinding | null = null, owner: { dispose(): void } | null = null, composition: WorldComposition | null = null
  let loading: Promise<WorldComposition> | null = null, closed = false, stopComposition = () => {}
  const notify = (set: Set<() => void>) => { for (const fn of Array.from(set)) { try { fn() } catch { /* Independent lease observers. */ } } }
  const stopForwarding = () => { const stop = stopComposition; stopComposition = () => {}; try { stop() } catch { /* Finish independent cleanup. */ } }
  const current = () => !closed && binding && !binding.release.signal.aborted && auth.session.isCurrent(binding.release.scope) && auth.privacy.isCurrent(binding.release.privacyScope)
  const access: WorldAccessBindingPort = { read: () => current() ? binding : null,
    subscribe(fn) { if (closed) return () => {}; listeners.add(fn); return () => { listeners.delete(fn) } } }
  const world: WorldLifecyclePort = { mount(host, context): WorldLease {
    const privacy = context.privacyScope
    if (closed || owner || !privacy || context.signal.aborted || !auth.session.isCurrent(context.scope) || !auth.privacy.isCurrent(privacy)
      || privacy.userId !== context.scope.userId || privacy.eventId !== context.scope.eventId || privacy.generation !== context.scope.generation) throw new Error('World authority unavailable')
    const lifetime = new AbortController()
    const release = Object.freeze({ scope: Object.freeze({ userId: context.scope.userId, eventId: context.scope.eventId, generation: context.scope.generation }), privacyScope: Object.freeze({ userId: privacy.userId, eventId: privacy.eventId, generation: privacy.generation, policyGeneration: privacy.policyGeneration, gate: privacy.gate }), bootstrap: context.bootstrap, signal: lifetime.signal })
    let disposed = false, lease: WorldLease | null = null, blocked = true, visible = true, stopPrivacy = () => {}, loadFailed = !configuration
    const notice = document.createElement('p'); notice.setAttribute('role', 'status'); notice.textContent = 'Opening the world…'; host.append(notice)
    const owned = { dispose() {
      if (disposed) return
      disposed = true; lifetime.abort();
      try { stopPrivacy() } catch { /* Finish independent cleanup. */ }
      context.signal.removeEventListener('abort', owned.dispose)
      if (owner === owned) { owner = null; binding = null; stopForwarding() }
      try { lease?.dispose() } catch { /* Always finish privacy and DOM withdrawal. */ }
      notice.remove(); notify(listeners); notify(observers); observers.clear()
    } }
    const live = () => !disposed && owner === owned && binding?.release === release && !!current()
    const replication: ReplicationReader = {
      read() {
        if (!live()) return null
        const value = composition?.replication.read() ?? null
        return live() ? value : null
      },
      subscribe(fn) { if (!live()) return () => {}; observers.add(fn); return () => { observers.delete(fn) } },
      async switchRoom(id) {
        if (!live()) return false
        const sent = await (composition?.replication.switchRoom(id) ?? Promise.resolve(false))
        return live() && sent
      },
    }
    const presentation = {
      read() { return live() ? composition?.presentation?.read() ?? null : null },
      subscribe(fn: () => void) { if (!live()) return () => {}; observers.add(fn); return () => { observers.delete(fn) } },
    }
    const socialPolicy = {
      read() { return live() ? composition?.policy?.read() ?? null : null },
      subscribe(fn: () => void) { if (!live()) return () => {}; observers.add(fn); return () => { observers.delete(fn) } },
    }
    const lifecycle = {
      read() {
        if (!live()) return null
        const status = loadFailed ? { phase: 'unavailable' as const, problem: null }
          : lease && composition ? composition.read() : { phase: 'loading' as const, problem: null }
        return live() ? projectWorldLifecycle(release, status) : null
      },
      subscribe(fn: () => void) { if (!live()) return () => {}; observers.add(fn); return () => { observers.delete(fn) } },
    }
    owner = owned; binding = Object.freeze({ release, replication, presentation, socialPolicy, lifecycle,
      async reconnect() {
        if (!live()) return false
        const recovered = await (composition?.replication.reconnect() ?? Promise.resolve(false))
        return live() && recovered
      },
    })
    context.signal.addEventListener('abort', owned.dispose, { once: true })
    try { stopPrivacy = auth.privacy.registerCleanup(owned.dispose) } catch { owned.dispose(); throw new Error('World authority unavailable') }
    notify(listeners)
    if (live() && !configuration) notice.textContent = 'The world is unavailable. You can keep browsing.'
    if (live() && configuration) {
      if (!loading) loading = Promise.resolve().then(() => (options.load ?? loadProduction)({ auth, origin: options.origin ?? location.origin, localInput: configuration,
        ...(options.characters ? { characters: options.characters } : {}), ...(options.mural ? { mural: options.mural } : {}) })).then(value => {
        if (closed) { value.dispose(); throw new Error('World owner closed') }
        composition = value; return value
      }).catch(error => { loading = null; throw error })
      void loading.then(value => {
        if (!live()) return
        stopForwarding()
        const forward = () => { if (live()) notify(observers) }
        const stops = [value.subscribe(forward), value.replication.subscribe(forward), value.presentation?.subscribe(forward),
          value.policy?.subscribe(forward)]
        stopComposition = () => stops.forEach(stop => stop?.())
        notice.remove()
        const created = value.mount(host, { ...context, signal: lifetime.signal, privacyScope: release.privacyScope })
        if (!live()) { created.dispose(); return }
        lease = created; lease.setInputBlocked(blocked); lease.setVisible(visible); notify(observers)
      }).catch(() => {
        if (!live()) return
        try { lease?.dispose() } catch { /* Safe unavailable presentation still owns this host. */ }
        lease = null; loadFailed = true; stopForwarding()
        notice.textContent = 'The world is unavailable. You can keep browsing.'
        host.replaceChildren(notice)
        notify(observers)
      })
    }
    return { dispose: owned.dispose, setInputBlocked(value) { blocked = value; if (live()) lease?.setInputBlocked(value) }, setVisible(value) { visible = value; if (live()) lease?.setVisible(value) } }
  } }
  return { world, access, dispose() {
    if (closed) return
    closed = true; owner?.dispose(); stopForwarding()
    try { composition?.dispose() } catch { /* Complete independent application cleanup. */ }
    composition = null; notify(listeners); notify(observers); listeners.clear(); observers.clear()
  } }
}
