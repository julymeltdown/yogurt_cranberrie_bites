import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { createMemoryRouter, RouterProvider } from 'react-router-dom'
import { createAppRoutes } from '../../router'
import { createProductionPorts } from '../../ports'
import { createFeedFixture, inputRevision, mediaInputs, selectedPin, type FixtureMode } from './fixture'
import '../../../shared/styles/base.css'
import '../../shell.css'

const modeInput = new URLSearchParams(location.search).get('mode') ?? 'loaded'
if (!['loaded', 'no-readiness', 'denied-media'].includes(modeInput)) throw new Error('Unknown synthetic Feed mode')
const fixture = await createFeedFixture(modeInput as FixtureMode)
// Exactly the default route composition and Root registry/media providers. No panel override.
const ports = createProductionPorts(fixture.auth, {}, fixture.social, fixture.eventId)
const router = createMemoryRouter(createAppRoutes(ports), { initialEntries: ['/feed'] })
const root = createRoot(document.getElementById('root')!)
const objectUrls = new Set<string>(), create = URL.createObjectURL.bind(URL), revoke = URL.revokeObjectURL.bind(URL)
URL.createObjectURL = blob => { const url = create(blob); objectUrls.add(url); return url }
URL.revokeObjectURL = url => { objectUrls.delete(url); revoke(url) }
let disposed = false
const dispose = () => {
  if (disposed) return
  disposed = true
  try { root.unmount(); router.dispose() } finally {
    try { ports.dispose?.() } finally {
      fixture.dispose()
      URL.createObjectURL = create; URL.revokeObjectURL = revoke
    }
  }
}
window.__FEED_SHELL__ = {
  qualification: { synthetic: true, mode: modeInput, inputRevision, mediaInputs, liveIN03: 'NOT_RUN', world: 'DISABLED', selectedStatic: '1.1.0' },
  calls: fixture.calls, resources: () => objectUrls.size,
  characterDiagnostics: () => ports.characters?.read(fixture.auth.privacy.capture(), selectedPin)?.diagnostics() ?? null,
  setReady: fixture.setReady, invalidate: fixture.invalidate,
  navigate: path => router.navigate(path), dispose,
}
root.render(<StrictMode><RouterProvider router={router} /></StrictMode>)
window.addEventListener('pagehide', dispose, { once: true })
declare global { interface Window { __FEED_SHELL__: {
  qualification: unknown; calls: { method: string; path: string; status: number }[]
  resources(): number; setReady(value: boolean): void; invalidate(): void; navigate(path: string): Promise<void>; dispose(): void
  characterDiagnostics(): Readonly<{ disposed: boolean; leases: number; pending: number; cleanupFailures: number; decodedAtlases: number }> | null
} } }
