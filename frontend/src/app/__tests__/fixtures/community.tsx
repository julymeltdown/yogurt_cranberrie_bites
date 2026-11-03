import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { createBrowserRouter, RouterProvider } from 'react-router-dom'
import { createAppRoutes } from '../../router'
import { bootstrapFor, createFixturePorts } from './ports'
import { fixture } from '../../../features/gift-discovery/__tests__/fixtures/data'
import '../../../shared/styles/base.css'
import '../../shell.css'

// Real application route composition + FE01 transport, with deterministic HTTP
// data and an explicitly instrumented world lease. No server/multiplayer claim.
const f = fixture(), world = createFixturePorts()
let giftsAvailable = true, held = false
const waiting: (() => void)[] = []
let bootstrapReads = 0
const ports = { ...world.ports, auth: f.auth, session: f.auth.session, bootstrap: {
  async load(scope: Parameters<typeof bootstrapFor>[0], signal: AbortSignal) {
    bootstrapReads++
    if (held) await new Promise<void>(resolve => waiting.push(resolve))
    if (signal.aborted) throw new Error('Aborted fixture bootstrap')
    const value = bootstrapFor(scope)
    return { ...value, event: { ...value.event, title: 'Community integration · synthetic event' },
      capabilities: { ...value.capabilities, gifts: giftsAvailable ? value.capabilities.gifts
        : { available: false as const, reason: 'FEATURE_DISABLED' as const } } }
  },
} }
history.replaceState(null, '', '/gifts')
const router = createBrowserRouter(createAppRoutes(ports))
const root = createRoot(document.getElementById('root')!)
root.render(<StrictMode><RouterProvider router={router} /></StrictMode>)
window.__COMMUNITY__ = {
  state: () => ({ ...world.stats, signals: world.stats.signals.map(signal => signal.aborted),
    requests: f.requests.map(request => request.url), bootstrapReads }),
  revoke() { held = true; f.auth.privacy.changePolicy(f.auth.privacy.capture(), 'block'); return world.stats.games },
  releaseDisabled() { giftsAvailable = false; held = false; waiting.splice(0).forEach(resolve => resolve()) },
  dispose() { root.unmount(); router.dispose(); f.dispose() },
}
declare global { interface Window { __COMMUNITY__: {
  state(): { games: number; connections: number; mounts: number; disposals: number; peakGames: number; peakConnections: number;
    requests: string[]; bootstrapReads: number; signals: boolean[] }
  revoke(): number; releaseDisabled(): void; dispose(): void
} } }
