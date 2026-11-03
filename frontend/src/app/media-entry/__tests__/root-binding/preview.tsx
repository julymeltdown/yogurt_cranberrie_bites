import { createRoot } from 'react-dom/client'
import { createMemoryRouter, RouterProvider } from 'react-router-dom'
import { createAppRoutes } from '../../../router'
import { RootMediaProvider } from '../../provider'
import { authorizedPost, rowFixture } from './rows-fixture'
import '../../../../shared/styles/base.css'
import '../../../shell.css'
const created: string[] = [], revoked: string[] = []
const create = URL.createObjectURL.bind(URL), revoke = URL.revokeObjectURL.bind(URL)
URL.createObjectURL = blob => { const url = create(blob); created.push(url); return url }
URL.revokeObjectURL = url => { revoked.push(url); revoke(url) }
// Qualification fixture supplies known authorized rows and current identity; it
// does not enable a production service or change actual route/feature rendering.
const f = rowFixture(request => fetch(request), true, true)
const router = createMemoryRouter(createAppRoutes(f.ports), { initialEntries: ['/feed'] })
const root = createRoot(document.getElementById('root')!)
root.render(<RootMediaProvider auth={f.auth} scope={f.scope}><RouterProvider router={router} /></RootMediaProvider>)
const api = {
  navigate: (path: string) => router.navigate(path),
  thread: () => router.navigate(`/feed/${authorizedPost.post_id}`),
  state: () => ({ requests: f.requests.map(r => new URL(r.url).pathname), created: [...created], revoked: [...revoked], world: { mounts: f.stats.mounts, games: f.stats.games, connections: f.stats.connections, peakGames: f.stats.peakGames } }),
  revoke: () => f.auth.privacy.changePolicy(f.scope, 'block'),
  dispose() { root.unmount(); router.dispose(); f.dispose(); URL.createObjectURL = create; URL.revokeObjectURL = revoke },
}
declare global { interface Window { __ROOT_MEDIA_BINDING__: typeof api } }
window.__ROOT_MEDIA_BINDING__ = api
