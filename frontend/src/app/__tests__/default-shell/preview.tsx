import { createRoot } from 'react-dom/client'
import { createMemoryRouter, RouterProvider } from 'react-router-dom'
import { createAppRoutes } from '../../router'
import { createFixturePorts } from '../fixtures/ports'
import '../../../shared/styles/base.css'
import '../../shell.css'
const fixture = createFixturePorts()
const router = createMemoryRouter(createAppRoutes(fixture.ports), { initialEntries: [new URLSearchParams(location.search).get('path') ?? '/world'] })
const root = createRoot(document.getElementById('root')!)
root.render(<RouterProvider router={router} />)
window.__DEFAULT_CHROME__ = { state: () => ({ ...fixture.stats, path: router.state.location.pathname }), dispose() { root.unmount(); router.dispose() } }
declare global { interface Window { __DEFAULT_CHROME__: { state(): typeof fixture.stats & { path: string }; dispose(): void } } }
