import type { WorldLease, WorldLifecyclePort } from '../../app/ports'
import type { SessionController } from '../../features/auth/model/session'
import type { PrivacyScope } from '../../shared/query/privacy'
import type { WorldCamera, Point } from '../camera/port'
import { attachMovement, type MovementAttachment } from '../input/controller'
import { abortable, loadMap, type LoadedMap } from '../map/resources'
import { createWorldRuntime, type EngineFactory, type OverviewFactory, type RuntimeState, type WorldRuntime } from '../runtime'
import { createWorldReplication, type ReplicationView, type WorldReplication } from '../network/replication'
import { WebSocketWorldTransport } from '../network/websocket'
import { immutable } from '../entities/replicated'
import { createTicketAcquirer } from './ticket'
import { prepareRelease, samePrivacy, type PreparedWorldRelease, type WorldReleaseResolver } from './release'
import { createThemeController, type ThemeController } from '../../features/theme/model/controller'
import { createLocalThemePreferences, type ThemePreferences } from '../../features/theme/model/preferences'
import { MapError } from '../map/json'
import { createThemeLoader } from '../theme/loader'
import type { PlaceMapRelease } from '../../features/places/model/controller'
import type { AvatarScenePort } from '../avatar/scene'
import type { SceneryOverviewPort } from '../illustrated/overview'
import { attachMuralEntrySource, type MuralEntryReleaseSource, type MuralQualificationFactory } from '../mural-entry-lifecycle/source'

export type { WorldReleaseResolver, ResolvedWorldRelease, WorldReleaseRequest } from './release'
export type WorldRendererStatus = 'loading' | 'ready' | 'unavailable'
export interface WorldRendererLease {
  replace(view: ReplicationView): void
  dispose(): void
  readStatus?(): WorldRendererStatus
  subscribeStatus?(listener: () => void): () => void
}
/** FE12 owns art. No fallback character, synthetic sprite or second Phaser game lives here.
 * The renderer calls sample from its existing frame loop; composition adds no render RAF.
 */
export interface WorldRendererPort {
  mount(context: { host: HTMLElement; map: LoadedMap; camera: WorldCamera; release: PreparedWorldRelease;
    signal: AbortSignal; avatarScene?: AvatarScenePort; sample(): ReplicationView | null }): WorldRendererLease
}
export interface WorldCompositionState {
  readonly phase: 'idle' | 'loading' | 'ready' | 'unavailable' | 'disposed'
  readonly runtime: RuntimeState | null
  readonly renderer: WorldRendererStatus
  readonly problem: 'release' | 'runtime' | 'renderer' | null
}
/** Stable FE18 release; null withdraws all prior membership/presence immediately. */
export interface WorldReplicationRelease {
  read(): ReplicationView | null
  subscribe(listener: () => void): () => void
  switchRoom(roomId: string): Promise<boolean>
  reconnect(): Promise<boolean>
}
/** Read-only capabilities of this exact live engine/release, withdrawn before any resource cleanup. */
export interface WorldPresentationRelease {
  /** Explicit qualified mural source on this incumbent world; absent stays unavailable. */
  readonly mural?: MuralEntryReleaseSource
  readonly overview?: SceneryOverviewPort
  readonly request: import('./release').WorldReleaseRequest
  readonly map: PlaceMapRelease
  readonly camera: Pick<MovementAttachment['camera'], 'lookAt' | 'recenter' | 'setZoom' | 'read'>
  readonly theme: Pick<ThemeController, 'read' | 'subscribe' | 'select'>
}
export interface WorldPresentationPort { read(): WorldPresentationRelease | null; subscribe(listener: () => void): () => void }
export interface WorldCompositionOptions {
  readonly mural?: MuralQualificationFactory
  readonly auth: SessionController
  readonly resolveRelease: WorldReleaseResolver
  readonly renderer?: WorldRendererPort
  readonly themePreferences?: ThemePreferences
  /** Existing FE14 engine seam. Omit in production to dynamically load actual Phaser. */
  readonly importEngine?: () => Promise<EngineFactory>
  readonly timeoutMs?: number
  /** Runtime-owned projection factory; the default remains the incumbent overview. */
  readonly createOverview?: OverviewFactory
}
export type WorldMountContext = Parameters<WorldLifecyclePort['mount']>[1] & { readonly privacyScope?: PrivacyScope | null }
const sessionOwners = new WeakMap<SessionController, { owner: object | null; drain: Promise<void> }>()

/** One adapter per canonical shell. A second live mount is rejected, never silently taken over. */
export function createWorldComposition(options: WorldCompositionOptions) {
  const { auth } = options, listeners = new Set<() => void>()
  const authority = sessionOwners.get(auth) ?? { owner: null, drain: Promise.resolve() }
  sessionOwners.set(auth, authority)
  let owner: { dispose(): void; replication: WorldReplication | null; rooms: ReadonlySet<string> } | null = null
  let state: WorldCompositionState = Object.freeze({ phase: 'idle', runtime: null, renderer: 'unavailable', problem: null })
  let presented: WorldPresentationRelease | null = null
  const presentation: WorldPresentationPort = { read: () => presented && !presented.request.signal.aborted && auth.privacy.isCurrent(presented.request.privacyScope) && auth.session.isCurrent(presented.request.scope) ? presented : null, subscribe: listener => release.subscribe(listener) }
  let closed = false, drain = Promise.resolve()
  const notify = () => { for (const listener of Array.from(listeners)) { try { listener() } catch { /* Isolated FE18 observers. */ } } }
  const release: WorldReplicationRelease = {
    read: () => owner?.replication?.read() ?? null,
    subscribe(listener) { if (closed) return () => {}; listeners.add(listener); return () => { listeners.delete(listener) } },
    switchRoom: roomId => owner?.rooms.has(roomId) ? owner.replication?.switchRoom(roomId) ?? Promise.resolve(false) : Promise.resolve(false),
    reconnect: () => owner?.replication?.reconnect() ?? Promise.resolve(false),
  }
  function mount(host: HTMLElement, context: WorldMountContext): WorldLease {
    const suppliedScope = context.privacyScope
    if (closed || owner || authority.owner || !suppliedScope || !auth.privacy.isCurrent(suppliedScope)
      || !auth.session.isCurrent(context.scope) || !samePrivacy(suppliedScope, { ...suppliedScope, ...context.scope })
      || context.signal.aborted || !context.bootstrap.worldAvailable
      || !context.bootstrap.capabilities.world.available || ['DRAFT', 'ARCHIVED'].includes(context.bootstrap.event.phase)) throw new Error('World authority unavailable')
    const privacyScope: PrivacyScope = Object.freeze({ ...suppliedScope })
    context = { ...context, scope: Object.freeze({ ...context.scope }), bootstrap: immutable(structuredClone(context.bootstrap)) }
    let disposed = false, failed = false, blocked = true, visible = true
    let runtime: WorldRuntime | null = null
    let renderer: WorldRendererLease | null = null, camera: WorldCamera | null = null, foot: Point | null = null, zoom = 1, inputPaused: boolean | null = null
    let themeController: ThemeController | null = null
    let muralEntry: ReturnType<typeof attachMuralEntrySource> | null = null
    let movement: MovementAttachment | null = null, presentationAbort: AbortController | null = null
    let removeRendererStatus = () => {}
    let controller = new AbortController(), bootRevision = 0
    const waiting = Promise.all([drain, authority.drain]).then(() => {})
    const root = document.createElement('div'), stage = document.createElement('div'), inputs = document.createElement('div'), status = document.createElement('p'), retry = document.createElement('button')
    root.className = 'world-composition'; root.style.cssText = 'height:100%;min-height:0;display:flex;flex-direction:column;overflow-y:auto;overscroll-behavior:contain'
    root.setAttribute('role', 'region'); root.setAttribute('aria-label', 'World scene and controls'); root.tabIndex = 0
    stage.style.cssText = 'flex:1 0 120px;min-height:120px;overflow:hidden'; inputs.style.cssText = 'flex:none'; status.setAttribute('role', 'status')
    const authorityNotice = document.createElement('p'), summary = document.createElement('div')
    summary.className = 'world-composition-summary'
    status.className = 'world-composition-status'; authorityNotice.className = 'world-composition-notice'
    authorityNotice.textContent = 'Virtual position is not physical arrival.'
    retry.type = 'button'; retry.hidden = true; retry.textContent = 'Reconnect world'
    status.textContent = 'Opening the world…'; summary.append(status, retry, authorityNotice); root.append(stage, inputs, summary); host.append(root)
    const cleanup: (() => void)[] = []
    const current = () => !disposed && !failed && owner === owned && !controller.signal.aborted
      && auth.privacy.isCurrent(privacyScope) && auth.session.isCurrent(context.scope)
    const publish = (patch: Partial<WorldCompositionState>) => {
      if (!current()) return
      state = Object.freeze({ ...state, ...patch }); notify()
    }
    const refreshControls = () => {
      const paused = blocked || document.hidden || !visible || owned.replication?.read().phase !== 'live'
      if (runtime && paused !== inputPaused) { inputPaused = paused; runtime.setInputBlocked(paused) }
    }
    function renderStatus() {
      if (!current()) return
      const rendererState = renderer ? renderer.readStatus?.() ?? 'ready' : 'unavailable'
      state = Object.freeze({ ...state, renderer: rendererState })
      const view = owned.replication?.read()
      if (!view) return
      status.textContent = view.phase === 'live' ? rendererState === 'ready' ? 'World connected.'
        : rendererState === 'loading' ? 'Connected. Loading avatar artwork…' : 'Connected. Avatar artwork is unavailable.'
        : view.phase === 'suspended' ? 'World paused. Return to continue.'
          : view.phase === 'closed' ? 'World disconnected. Select Reconnect world to continue.' : 'Connecting to the world…'
      retry.hidden = view.phase !== 'closed'; retry.textContent = 'Reconnect world'
    }
    function replace(view: ReplicationView) {
      if (!current()) return
      if (view.phase === 'unavailable' && (view.problem === 'authority' || view.problem === 'sink')) { dispose(); return }
      if (view.problem === 'map-version') { fail('release'); return }
      const self = view.entities.find(entity => entity.self)
      foot = self?.player.position ?? null
      if (foot && movement?.camera.read().following !== false) camera?.center(foot, zoom)
      renderer?.replace(view)
      if (!current()) return
      refreshControls()
      renderStatus()
      notify()
    }
    function stopResources() {
      ++bootRevision
      muralEntry?.dispose(); muralEntry = null
      presented = null
      presentationAbort?.abort(); presentationAbort = null
      themeController?.dispose(); themeController = null
      notify() // Synchronous withdrawal precedes replication, camera and engine destruction.
      movement = null
      const removeStatus = removeRendererStatus; removeRendererStatus = () => {}
      try { removeStatus() } catch { /* A broken observer must not strand privacy teardown. */ }
      // Withdraw socket authority and roster before releasing visual ownership.
      owned.replication?.dispose()
      try { if (owned.replication) renderer?.replace(owned.replication.read()) } catch { /* Disposal still clears a broken sink. */ }
      owned.replication = null
      try { renderer?.dispose() } catch { /* Continue releasing every owner. */ }
      renderer = null; camera = null; foot = null
      controller.abort()
      runtime?.dispose()
      if (runtime) { drain = Promise.all([drain, runtime.settled()]).then(() => {}); authority.drain = drain }
      runtime = null
    }
    function dispose() {
      if (disposed) return
      disposed = true; stopResources(); root.remove()
      for (const remove of cleanup.splice(0)) remove()
      if (authority.owner === owned) authority.owner = null
      if (owner === owned) { owner = null; state = Object.freeze({ phase: 'disposed', runtime: null, renderer: 'unavailable', problem: null }); notify() }
    }
    const owned = { dispose, replication: null as WorldReplication | null, rooms: new Set<string>() }
    owner = owned
    authority.owner = owned
    const fail = (problem: NonNullable<WorldCompositionState['problem']>) => {
      if (!current()) return
      failed = true; stopResources(); inputs.replaceChildren()
      status.textContent = 'The world is unavailable. You can try opening it again.'
      retry.hidden = false; retry.textContent = 'Try opening world again'
      state = Object.freeze({ phase: 'unavailable', runtime: null, renderer: 'unavailable', problem }); notify()
    }
    const setVisible = () => {
      if (!current()) return
      const show = visible && !document.hidden
      // A panel covers the engine; only document backgrounding withdraws presence.
      owned.replication?.setVisible(!document.hidden)
      if (!current()) return
      runtime?.setVisible(show); refreshControls()
    }
    const listen = (target: EventTarget, event: string, listener: EventListener) => {
      target.addEventListener(event, listener); cleanup.push(() => target.removeEventListener(event, listener))
    }
    try {
      cleanup.push(auth.privacy.registerCleanup(dispose))
      listen(auth.session.read().signal, 'abort', dispose)
      listen(context.signal, 'abort', dispose)
      listen(document, 'visibilitychange', setVisible)
      listen(retry, 'click', () => {
        if (disposed || context.signal.aborted || !auth.session.isCurrent(context.scope) || !auth.privacy.isCurrent(privacyScope)) return
        if (!failed) { if (current()) void owned.replication?.reconnect(); return }
        failed = false; controller = new AbortController(); retry.hidden = true; status.textContent = 'Opening the world…'
        state = Object.freeze({ phase: 'loading', runtime: null, renderer: 'unavailable', problem: null }); notify()
        if (current()) void boot()
      })
      if (!current()) { dispose(); return { dispose, setInputBlocked() {}, setVisible() {} } }
      state = Object.freeze({ phase: 'loading', runtime: null, renderer: 'unavailable', problem: null }); notify()
      void boot()
    } catch { dispose(); throw new Error('World authority unavailable') }
    async function boot() {
      const attempt = controller, revision = ++bootRevision
      const isBootCurrent = () => current() && attempt === controller && revision === bootRevision
      let step: 'release' | 'runtime' | 'renderer' = 'release'
      try {
        await abortable(Promise.all([waiting, drain, authority.drain]), attempt.signal, options.timeoutMs)
        if (!isBootCurrent()) return
        const request = { scope: context.scope, privacyScope, bootstrap: context.bootstrap }
        const prepared = await auth.privacy.run(privacyScope, async ({ signal }) => {
          const resolved = await options.resolveRelease({ ...request, signal })
          return prepareRelease(resolved, request)
        }, attempt.signal)
        if (!isBootCurrent()) return
        zoom = prepared.zoom
        owned.rooms = new Set(prepared.rooms.items.map(room => room.room_id))
        const transport = new WebSocketWorldTransport({ session: auth.session, origin: location.origin,
          acquireTicket: createTicketAcquirer(auth, privacyScope) })
        owned.replication = createWorldReplication({ session: auth.session, transport, resources: prepared.resources, sink: { replace } })
        step = 'runtime'
        runtime = createWorldRuntime({ host: stage, load: signal => loadMap(prepared.mapInput, signal),
          ...(options.createOverview ? { createOverview: options.createOverview } : {}),
          ...(options.importEngine ? { importEngine: options.importEngine } : {}), ...(options.timeoutMs ? { timeoutMs: options.timeoutMs } : {}),
          attachControls: control => {
            if (!isBootCurrent()) throw new Error('World authority unavailable')
            camera = control.camera
            // FE15 may simulate a local intent route. Only C05 self position or explicit look-at may move the real camera.
            const authoritativeCamera: WorldCamera = { ...control.camera,
              center: (predicted, nextZoom) => { zoom = nextZoom; const target = movement?.camera.read().following === false ? predicted : foot; if (target && isBootCurrent()) camera?.center(target, zoom) } }
            const attachment = attachMovement({ ...control, camera: authoritativeCamera }, { controls: inputs,
              policy: prepared.movementPolicy, zoom: prepared.zoom,
              consume: intent => { if (isBootCurrent() && !blocked && visible && !document.hidden) owned.replication?.setIntent(intent) } })
            movement = attachment
            try {
              if (options.renderer) {
                step = 'renderer'
                const returned = options.renderer.mount({ host: stage, map: control.map, camera: control.camera, release: prepared,
                  signal: attempt.signal, ...(control.avatarScene ? { avatarScene: control.avatarScene } : {}),
                  sample: () => isBootCurrent() ? owned.replication?.sample() ?? null : null })
                if (!isBootCurrent()) returned.dispose()
                else {
                  renderer = returned
                  const remove = returned.subscribeStatus?.(() => {
                    if (!isBootCurrent()) return
                    try { renderStatus(); notify() } catch { fail('renderer') }
                  }) ?? (() => {})
                  if (!isBootCurrent()) remove(); else removeRendererStatus = remove
                }
              }
              if (options.mural && control.muralScene) muralEntry = attachMuralEntrySource({ privacy: auth.privacy,
                context: { scope: privacyScope, signal: control.signal, scene: control.muralScene, world: release, release: prepared }, factory: options.mural })
              return attachment
            } catch (error) { attachment.dispose(); throw error }
          } })
        inputPaused = true; runtime.setInputBlocked(true); runtime.setVisible(visible && !document.hidden)
        const mounted = await runtime.start(attempt.signal)
        if (!isBootCurrent()) return
        if (mounted.phase !== 'ready') { fail(step); return }
        if (!movement || !runtime) { fail('runtime'); return }
        const preferences = options.themePreferences ?? {
          read: (scope: Parameters<ThemePreferences['read']>[0]) => createLocalThemePreferences(localStorage).read(scope),
          write: (scope: Parameters<ThemePreferences['write']>[0], theme: Parameters<ThemePreferences['write']>[1]) => createLocalThemePreferences(localStorage).write(scope, theme),
        }
        const loadTheme = createThemeLoader(prepared.themeInputs)
        const theme = createThemeController({ scope: { userId: context.scope.userId, eventId: context.scope.eventId }, preferences, runtime, initial: prepared.themeId, load: (id, signal) => prepared.availableThemeIds.includes(id) ? loadTheme(id, signal) : Promise.reject(new MapError('unavailable')) })
        themeController = theme
        const attachment = movement as MovementAttachment
        presentationAbort = new AbortController()
        const presentationSignal = presentationAbort.signal
        const livePresentation = () => isBootCurrent() && !presentationSignal.aborted
        const publicCamera = Object.freeze({
          lookAt: (point: Point) => { if (livePresentation()) attachment.camera.lookAt(point) },
          recenter: () => { if (livePresentation()) attachment.camera.recenter() },
          setZoom: (value: number) => livePresentation() && attachment.camera.setZoom(value),
          read: () => livePresentation() ? attachment.camera.read() : { following: true, camera: null },
        })
        presented = Object.freeze({ request: Object.freeze({ ...request, signal: presentationAbort.signal }),
          ...(muralEntry ? { mural: Object.freeze({ read: muralEntry.read, subscribe: muralEntry.subscribe }) } : {}),
          map: Object.freeze({ scope: context.scope, manifest: prepared.map,
            expected: Object.freeze({ mapId: prepared.resources.mapId, revision: prepared.resources.mapRevision, contentHash: prepared.mapInput.pin.mapHash, geometryHash: prepared.map.geometry_hash }) }),
          camera: publicCamera, theme: Object.freeze({ read: theme.read, subscribe: theme.subscribe, select: theme.select }),
          overview: Object.freeze({ read: () => livePresentation() ? runtime?.overview.read() ?? null : null,
            subscribe: runtime.overview.subscribe }) })
        void theme.restore()
        publish({ phase: 'ready', runtime: mounted, renderer: renderer ? renderer.readStatus?.() ?? 'ready' : 'unavailable' })
        if (!isBootCurrent()) return
        owned.replication.setVisible(visible && !document.hidden)
        await owned.replication.start()
      } catch { if (isBootCurrent()) fail(step) }
    }
    return { dispose,
      setInputBlocked(value) {
        if (disposed) return
        blocked = value
        if (!current()) return
        if (value) owned.replication?.setIntent({ axisX: 0, axisY: 0, buttons: [] })
        refreshControls()
      },
      setVisible(value) { if (disposed) return; visible = value; setVisible() },
    }
  }
  return { mount, replication: release, presentation, read: () => state, subscribe: release.subscribe,
    dispose() { if (closed) return; closed = true; owner?.dispose(); listeners.clear() },
    /** Admission of a replacement game waits for the previous FE14 engine's closed promise. */
    settled: () => drain,
  }
}
export type WorldComposition = Omit<ReturnType<typeof createWorldComposition>, 'presentation'> & { readonly presentation?: WorldPresentationPort }
