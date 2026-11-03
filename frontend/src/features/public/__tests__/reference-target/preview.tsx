// TEST ONLY public presentation and canonical MemorySession/readiness fixtures.
// No live provider, person, event or artwork approval is synthesized here.
import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { createMemoryRouter, RouterProvider } from 'react-router-dom'
import { createAppRoutes } from '../../../../app/router'
import { createFixturePorts } from '../../../../app/__tests__/fixtures/ports'
import { PublicDocumentsProvider } from '../../model/document-provider'
import { syntheticDocument } from '../fixtures/documents'
import type { SessionReadiness } from '../../../../app/ports'
import '../../../../shared/styles/base.css'
import '../../../../app/shell.css'

const options = new URLSearchParams(location.search)
const accepted: readonly SessionReadiness[] = ['anonymous', 'unknown', 'checking', 'unavailable', 'error']
const input = options.get('entry') as SessionReadiness
const fixture = createFixturePorts(accepted.includes(input) ? input : 'anonymous')
fixture.ports.session.replace(fixture.ports.session.read().generation, null)
let retries = 0
const ports = { ...fixture.ports, readiness: { ...fixture.ports.readiness,
  retry() { retries++; fixture.setReadiness('checking') },
} }
const router = createMemoryRouter(createAppRoutes(ports), { initialEntries: [options.get('route') ?? '/'] })
const root = createRoot(document.getElementById('root')!)
root.render(<StrictMode><PublicDocumentsProvider provider={{ syntheticPreview: true, read: async key => syntheticDocument(key) }}><RouterProvider router={router} /></PublicDocumentsProvider></StrictMode>)
const controller = {
  state: () => ({ path: router.state.location.pathname, search: router.state.location.search, action: router.state.historyAction, retries, mounts: fixture.stats.mounts, games: fixture.stats.games }),
  readiness: fixture.setReadiness,
  ready() { fixture.replace(); fixture.setReadiness('ready') },
  revoke() { fixture.ports.session.replace(fixture.ports.session.read().generation, null); fixture.setReadiness('anonymous') },
  navigate: (path: string) => router.navigate(path),
  back: () => router.navigate(-1),
  dispose() { root.unmount(); router.dispose() },
}
Object.assign(window, { __REFERENCE_PUBLIC__: controller })
declare global { interface Window { __REFERENCE_PUBLIC__: typeof controller } }
