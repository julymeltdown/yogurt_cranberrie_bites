// MOCK_READY: real routes/providers/controllers/artwork; deterministic identity,
// authorized HTTP rows and instrumented world host. No live/multiuser claim.
import { createRoot } from 'react-dom/client'
import { createMemoryRouter, RouterProvider } from 'react-router-dom'
import selected from '../../../../../../contracts/fixtures/assets/selected-static/avatar-manifest.json'
import { createAppRoutes } from '../../../router'
import { createRootCharacterRegistry } from '../../../character-entry/runtime'
import { fixture, post, response, publicIntro, user, event, id } from '../../../social-composition/__tests__/fixtures'
import { party } from '../../../../features/party-discovery/__tests__/fixtures/data'
import { sha256 } from '../../../../world/map/json'
import '../../../../shared/styles/base.css'
import '../../../shell.css'
const mode = new URLSearchParams(location.search).get('mode') ?? 'whole'
const config = selected.presets[3]!.config, hostConfig = selected.presets[1]!.config
const original = publicIntro().author
const profile = { ...original, display_name: 'Authorized portrait subject', avatar: mode === 'layered' ? original.avatar : { revision: 7, config } }
const ownPosts = [0, 1, 2].map(n => ({ ...post, author: { ...post.author, avatar: profile.avatar, display_name: 'Authorized feed author' }, post_id: id(500 + n), text: `Authorized subject post ${n + 1}`, created_at: new Date(Date.parse(post.created_at) - n * 60_000).toISOString(), media_ids: [] }))
const otherPost = { ...post, post_id: id(509), author: { ...post.author, user_id: id(999) }, text: 'Other profile only', created_at: new Date(Date.parse(post.created_at) - 4 * 60_000).toISOString(), media_ids: [] }
const f = fixture(request => {
  const path = new URL(request.url).pathname
  if (path.includes('/people/')) return Response.json({ ...profile, user_id: path.split('/').at(-1)! })
  if (path.endsWith('/feed')) return Response.json({ items: [...ownPosts, otherPost], next_cursor: null })
  if (path.includes('/posts/')) return Response.json({ post: ownPosts.find(p => path.endsWith(`/${p.post_id}`)) ?? post, comments: [], next_cursor: null })
  if (path.endsWith('/parties')) return Response.json({ items: [
    party({ event_id: event, owner: { ...profile, display_name: 'Authorized party host', avatar: { revision: 2, config: hostConfig } } }),
    party({ party_id: id(700), event_id: event, owner: { ...original, display_name: 'Layered party host' } }),
  ], next_cursor: null })
  return response(request)
})
const bootstrap = { ...f.bootstrap, ...(mode === 'no-runtime' ? {} : { document: { avatar_manifest: selected } }) }
const characters = createRootCharacterRegistry(f.auth, location.origin)
const ports = { ...f.ports, characters, bootstrap: { load: async () => bootstrap }, social: {
  availability(context: { scope: typeof f.scope; bootstrap: typeof bootstrap }) {
    const value = { scope: context.scope, bootstrap: context.bootstrap, services: { feed: true, people: true } }
    return { read: () => value, subscribe: () => () => undefined }
  },
} }
const router = createMemoryRouter(createAppRoutes(ports), { initialEntries: [`/people/${user}`] })
const root = createRoot(document.getElementById('root')!)
root.render(<RouterProvider router={router} />)
const pin = { manifest_id: selected.manifest_id, manifest_version: selected.manifest_version, content_hash: selected.content_hash }
let retained: ReturnType<typeof characters.read> = null
function runtime() { const current = characters.read(f.scope, pin); if (current && !retained) retained = current; return current }
async function pixels(selector: string) {
  const canvas = document.querySelector<HTMLCanvasElement>(selector)
  if (!canvas) return null
  return sha256(new Uint8Array(canvas.getContext('2d')!.getImageData(0, 0, 64, 64).data.buffer))
}
async function expected(which: 'profile' | 'host') {
  const lease = await runtime()!.acquire(which === 'profile' ? config : hostConfig, { signal: f.auth.session.read().signal })
  const render = lease.read()!, canvas = document.createElement('canvas'); canvas.width = canvas.height = 64
  const ctx = canvas.getContext('2d')!; ctx.imageSmoothingEnabled = false
  ctx.drawImage(render.image, render.source.x, render.source.y, render.source.width, render.source.height, 0, 0, 64, 64)
  const hash = await sha256(new Uint8Array(ctx.getImageData(0, 0, 64, 64).data.buffer)); lease.dispose(); canvas.width = canvas.height = 0; return hash
}
const api = { user, other: id(999), mode, config, hostConfig, pixels, expected, navigate: (path: string) => router.navigate(path),
  state: () => ({ path: router.state.location.pathname, requests: f.requests.map(r => ({ path: new URL(r.url).pathname, method: r.method })), current: !!runtime(), sameRuntime: runtime() === retained, diagnostics: retained?.diagnostics(), world: { ...f.stats, signals: undefined } }),
  revoke: () => f.auth.privacy.changePolicy(f.scope, 'suspended'),
  dispose() { root.unmount(); router.dispose(); characters.dispose(); f.dispose() },
}
Object.assign(window, { __PROFILE_COMPOSITION__: api })
declare global { interface Window { __PROFILE_COMPOSITION__: typeof api } }
