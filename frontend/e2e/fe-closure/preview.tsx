import { StrictMode, useEffect, useState, type ReactNode } from 'react'
import { createRoot } from 'react-dom/client'
import { MemoryRouter } from 'react-router-dom'
import { createSessionController } from '../../src/features/auth/model/session'
import { projectCapabilities } from '../../src/app/capabilities'
import { GiftDetail } from '../../src/features/gift-discovery'
import { createGiftDiscoveryController, type GiftContext } from '../../src/features/gift-discovery/model/controller'
import { RootMediaProvider } from '../../src/app/media-entry/provider'
import { RootGiftImage } from '../../src/app/media-entry/RootMediaView'
import { createMeController, ProfileForm } from '../../src/features/me'
import { SettingsPage } from '../../src/features/settings'
import { createMuralHttp, createMuralSync, MuralSyncPanel } from '../../src/features/mural/sync'
import { ShellLink } from '../../src/app/navigation'

interface Runtime {
  event_id: string
  users: Record<'alice' | 'bob', string>
  tokens: Record<'alice' | 'bob', string>
  media_giveaway_id: string
  empty_giveaway_id: string
  missing_giveaway_id: string
  bootstrap: { capabilities: unknown }
}
interface Harness {
  ready: boolean
  error: string | null
  phase: string
  message: string
}
declare global { interface Window { __FE43__: Harness } }
window.__FE43__ = { ready: false, error: null, phase: 'starting', message: '' }
function fail(error: unknown) {
  window.__FE43__ = { ready: false, phase: 'error', message: '', error: error instanceof Error ? error.message : 'unknown' }
}
function authFor(runtime: Runtime, role: 'alice' | 'bob', token: string) {
  const auth = createSessionController({ eventId: runtime.event_id, fetch: (input, init) => fetch(input, init) })
  auth.session.replace(0, { userId: runtime.users[role], eventId: runtime.event_id, accessToken: token })
  return auth
}
function Frame({ children }: { children: ReactNode }) {
  return <MemoryRouter><main className="mcr-preview-frame">{children}<ShellLink id="preview-home" to="/world">World</ShellLink></main></MemoryRouter>
}
function Gifts({ runtime, role, mode }: { runtime: Runtime; role: 'alice' | 'bob'; mode: 'media' | 'empty' | 'missing' }) {
  const [value] = useState(() => {
    const auth = authFor(runtime, role, runtime.tokens[role])
    const context: GiftContext = Object.freeze({ scope: auth.privacy.capture(),
      capabilities: projectCapabilities(runtime.bootstrap.capabilities), eventPhase: 'OPEN' })
    return { auth, context, controller: createGiftDiscoveryController({ auth, context: { read: () => context, subscribe: () => () => {} } }) }
  })
  useEffect(() => {
    const detail = value.controller.read().detail
    window.__FE43__ = { ready: detail.phase === 'ready', error: null, phase: detail.phase, message: '' }
    return value.controller.subscribe(() => {
      const state = value.controller.read()
      window.__FE43__ = { ready: state.detail.phase === 'ready', error: null, phase: state.detail.phase, message: state.detail.failure ?? '' }
    })
  }, [value])
  return <Frame><RootMediaProvider auth={value.auth} scope={value.context.scope}>
    <GiftDetail controller={value.controller} giveawayId={mode === 'empty' ? runtime.empty_giveaway_id : mode === 'missing' ? runtime.missing_giveaway_id : runtime.media_giveaway_id}
      renderGiftImage={request => <RootGiftImage {...request} />} />
  </RootMediaProvider></Frame>
}
function Me({ runtime, role, conflict }: { runtime: Runtime; role: 'alice' | 'bob'; conflict: boolean }) {
  const [value] = useState(() => {
    const auth = authFor(runtime, role, runtime.tokens[role])
    return { auth, controller: createMeController(auth) }
  })
  useEffect(() => { void value.controller.load() }, [value])
  useEffect(() => value.controller.subscribe(() => {
    const state = value.controller.read()
    window.__FE43__ = { ready: state.phase === 'ready' || state.phase === 'conflict', error: null, phase: state.phase, message: state.message }
  }), [value])
  async function save() {
    if (!value.controller.read().participant) await new Promise<void>(resolve => {
      const stop = value.controller.subscribe(() => { if (value.controller.read().participant) { stop(); resolve() } })
    })
    if (conflict) {
      for (let attempt = 0; attempt < 5; attempt++) {
        const saved = value.controller.read().participant!
        try {
          await value.auth.request({ path: `/api/concert/events/${runtime.event_id}/me`, method: 'PATCH', access: 'protected', errorFamily: 'core',
            body: { json: { expected_revision: saved.revision, preferences: saved.preferences } },
            decode: value => value })
          break
        } catch {
          await value.controller.load()
        }
      }
    }
    value.controller.editPreferences({ low_power: !value.controller.read().draft.preferences.low_power })
    await value.controller.save('preferences')
  }
  return <Frame><section><SettingsPage controller={value.controller} auth={value.auth} /><button type="button" id="fe43-save" onClick={() => void save()}>Harness save</button><ProfileForm controller={value.controller} /></section></Frame>
}
function Mural({ runtime, role, invalid }: { runtime: Runtime; role: 'alice' | 'bob'; invalid: boolean }) {
  const [value] = useState(() => {
    const auth = authFor(runtime, role, invalid ? 'invalid-token' : runtime.tokens[role])
    const binding = { eventId: runtime.event_id, roomId: crypto.randomUUID(), roomEpoch: crypto.randomUUID(), streamId: crypto.randomUUID() }
    const authority = { read: () => ({ binding, writable: true }), subscribe: () => () => {} }
    const http = { request: auth.request, refreshSession: async () => { throw new Error('canonical Auth owns refresh') } }
    const adapter = createMuralHttp({ http, privacy: auth.privacy, authority })
    const sync = createMuralSync({ privacy: auth.privacy, authority, reads: adapter.reads, commands: adapter.commands,
      sink: { replace: () => {}, dispose: () => {}, subscribeFailure: () => () => {} } })
    return { auth, sync }
  })
  useEffect(() => {
    value.sync.start()
    return value.sync.subscribe(() => {
      const state = value.sync.read()
      window.__FE43__ = { ready: state.phase === 'ready', error: null, phase: state.phase, message: state.revision ? String(state.revision) : '' }
    })
  }, [value])
  return <Frame><div className="mcr-mural-preview"><MuralSyncPanel sync={value.sync} /></div></Frame>
}
const runtime = await fetch('/__fe43/runtime.json').then(async response => {
  if (!response.ok) throw new Error('runtime packet unavailable')
  return response.json() as Promise<Runtime>
})
const query = new URLSearchParams(location.search)
const role = query.get('role') === 'bob' ? 'bob' : 'alice'
const test = query.get('case') ?? 'gift'
function App() {
  if (test === 'gift') return <Gifts runtime={runtime} role={role} mode="media" />
  if (test === 'gift-empty') return <Gifts runtime={runtime} role={role} mode="empty" />
  if (test === 'gift-missing') return <Gifts runtime={runtime} role={role} mode="missing" />
  if (test === 'profile' || test === 'profile-conflict') return <Me runtime={runtime} role={role} conflict={test === 'profile-conflict'} />
  if (test === 'mural' || test === 'mural-unavailable') return <Mural runtime={runtime} role={role} invalid={test === 'mural-unavailable'} />
  throw new Error('Unknown FE43 case')
}
try {
  const root = document.getElementById('root')
  if (!root) throw new Error('Root missing')
  createRoot(root).render(<StrictMode><App /></StrictMode>)
} catch (error) { fail(error) }
