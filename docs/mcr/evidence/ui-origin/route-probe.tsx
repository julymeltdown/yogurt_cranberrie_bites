// Evidence-only: current production route composition, deterministic readiness,
// instrumented world host, no invented backend or privileged feature admission.
import { createRoot } from 'react-dom/client'
import { createMemoryRouter, RouterProvider, MemoryRouter } from 'react-router-dom'
import { createAppRoutes } from '../../../../frontend/src/app/router'
import { panelRoutes, publicPaths, adminPaths } from '../../../../frontend/src/app/routes'
import { createFixturePorts } from '../../../../frontend/src/app/__tests__/fixtures/ports'
import '../../../../frontend/src/shared/styles/base.css'
import '../../../../frontend/src/app/shell.css'
import { ConnectionStatusNotice, type ConnectionKind } from '../../../../frontend/src/features/connection-status'
const params = new URLSearchParams(location.search)
const fixture = createFixturePorts(params.get('anonymous') === '1' ? 'anonymous' : 'ready')
if (params.get('anonymous') === '1') fixture.ports.session.replace(fixture.ports.session.read().generation, null)
const router = createMemoryRouter(createAppRoutes(fixture.ports), { initialEntries: [params.get('route') ?? '/world'] })
const notice = params.get('notice') as ConnectionKind
const kinds = ['offline', 'reconnecting', 'resync', 'suspended', 'unavailable', 'maintenance', 'version']
createRoot(document.getElementById('root')!).render(kinds.includes(notice)
  ? <MemoryRouter><ConnectionStatusNotice view={{ kind: notice, stale: true, pending: false, canRetry: notice === 'offline' }} retry={() => {}} reload={() => {}} /></MemoryRouter>
  : <RouterProvider router={router} />)
Object.assign(window, { __QA02__: { paths: [...publicPaths, ...panelRoutes.map(route => route.path), ...adminPaths], navigate: (path: string) => router.navigate(path), stats: fixture.stats } })
