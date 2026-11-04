// TEST ONLY: default createAppRoutes/PublicBoundary with synthetic public C04 responses.
import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { createMemoryRouter, RouterProvider } from 'react-router-dom'
import { createAppRoutes } from '../../router'
import { createFixturePorts } from '../fixtures/ports'
import { eventId, id, page } from '../../../features/archive/__tests__/fixtures/data'
import '../../../shared/styles/base.css'
import '../../shell.css'

let mode = 'ready'
const fixture = createFixturePorts('anonymous')
const requests: Request[] = []
globalThis.fetch = async input => {
  const request = input as Request
  requests.push(request)
  if (request.method !== 'GET' || !new URL(request.url).pathname.endsWith(`/events/${eventId}/archive`)) throw new Error('Unexpected fixture request')
  if (mode === 'offline') throw new Error('DO_NOT_DISPLAY')
  if (mode === 'removed') return Response.json({ private_handle: 'DO_NOT_DISPLAY' }, { status: 410 })
  const data = page(new URL(request.url).searchParams.has('cursor'))
  if (mode === 'empty') { data.feed.items = []; data.feed.next_cursor = null }
  return Response.json(mode === 'private' ? { ...data, physical_beacon: 'DO_NOT_DISPLAY' } : data)
}
const router = createMemoryRouter(createAppRoutes({ ...fixture.ports, eventId }), { initialEntries: ['/archive'] })
const root = createRoot(document.getElementById('root')!)
root.render(<StrictMode><RouterProvider router={router} /></StrictMode>)
declare global {
  interface Window {
    __ARCHIVE_ROOT__: {
      mode(value: string): void
      generation(): void
      requests(): { method: string; path: string; credentials: string; cache: string; authorization: boolean; cookie: boolean; aborted: boolean }[]
      worldMounts(): number
      dispose(): void
    }
  }
}
window.__ARCHIVE_ROOT__ = {
  mode(value) { mode = value },
  generation() { fixture.replace(id(81), id(82)) },
  requests: () => requests.map(r => ({ method: r.method, path: new URL(r.url).pathname, credentials: r.credentials,
    cache: r.cache, authorization: r.headers.has('Authorization'), cookie: r.headers.has('Cookie'), aborted: r.signal.aborted })),
  worldMounts: () => fixture.stats.mounts,
  dispose() { root.unmount(); router.dispose() },
}
import.meta.hot?.dispose(() => window.__ARCHIVE_ROOT__.dispose())
