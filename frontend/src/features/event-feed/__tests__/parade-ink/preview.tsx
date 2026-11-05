import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { createMemoryRouter, RouterProvider } from 'react-router-dom'
import { createAppRoutes } from '../../../../app/router'
import { createProductionPorts } from '../../../../app/ports'
import { createFeedFixture, fixtureRoute, inputRevision, mediaInputs, selectedPin, shapeProvenance, type FixtureMode } from './fixture'
import '../../../../shared/styles/base.css'
import '../../../../app/shell.css'

declare const __PI_CANDIDATE__: string
declare const __PI_FOUNDATION_PINS__: readonly { path: string; sha256: string }[]
const mode = new URLSearchParams(location.search).get('mode') ?? 'loaded'
if (!['loaded', 'no-readiness', 'denied-media', 'thread', 'notice', 'long', 'empty', 'error', 'read-only', 'pending'].includes(mode)) throw Error('Unknown PI02/03 synthetic state')
const fixture = await createFeedFixture(mode as FixtureMode)
const ports = createProductionPorts(fixture.auth, {}, fixture.social, fixture.eventId)
const router = createMemoryRouter(createAppRoutes(ports), { initialEntries: [mode === 'notice' ? '/feed' : fixtureRoute(mode as FixtureMode)] })
const root = createRoot(document.getElementById('root')!)
const urls = new Set<string>(), create = URL.createObjectURL.bind(URL), revoke = URL.revokeObjectURL.bind(URL)
URL.createObjectURL = blob => { const url = create(blob); urls.add(url); return url }
URL.revokeObjectURL = url => { urls.delete(url); revoke(url) }
let disposed = false
const dispose = () => {
  if (disposed) return
  disposed = true
  try { root.unmount(); router.dispose() } finally {
    try { ports.dispose?.() } finally { fixture.dispose(); URL.createObjectURL = create; URL.revokeObjectURL = revoke }
  }
}
window.__PARADE_FEED__ = {
  qualification: { synthetic: true, actualDefaultApp: true, candidate: __PI_CANDIDATE__, mode, inputRevision, mediaInputs,
    shapeProvenance, foundationPins: __PI_FOUNDATION_PINS__, liveIN03: 'NOT_RUN', device: 'NOT_RUN', selectedStatic: '1.1.0' },
  calls: fixture.calls, resources: () => urls.size, setReady: fixture.setReady, invalidate: fixture.invalidate,
  navigate: path => router.navigate(path), dispose,
  characterDiagnostics: () => ports.characters?.read(fixture.auth.privacy.capture(), selectedPin)?.diagnostics() ?? null,
}
root.render(<StrictMode><RouterProvider router={router} /></StrictMode>)
window.addEventListener('pagehide', dispose, { once: true })
declare global { interface Window { __PARADE_FEED__: {
  qualification: unknown; calls: { method: string; path: string; status: number }[]
  resources(): number; setReady(value: boolean): void; invalidate(): void; navigate(path: string): Promise<void>; dispose(): void
  characterDiagnostics(): Readonly<{ disposed: boolean; leases: number; pending: number; cleanupFailures: number; decodedAtlases: number }> | null
} } }
