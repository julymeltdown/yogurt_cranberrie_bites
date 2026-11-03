// Isolated synthetic browser harness. No selected-art approval or real World renderer claim.
import { useState } from 'react'
import { createRoot } from 'react-dom/client'
import { createAccountOwner } from '../../owner'
import { AvatarThumbnail } from '../../../../features/me/ui/AvatarThumbnail'
import { AvatarEditor } from '../../../../features/avatar'
import { ReviewedProfileAvatar } from '../../../../features/profile/Avatar'
import { setup, participant, userId, eventId } from '../../../../features/me/__tests__/fixtures'
import { bootstrapFor } from '../../../__tests__/fixtures/ports'
import { fixture } from './fixture'
import { createReviewedAvatarCompositor } from '../../../../entities/avatar/runtime'
import '../../../../shared/styles/base.css'

const counters = { reads: 0, decodes: 0, createdUrls: 0, revokedUrls: 0, meReads: 0 }
const originalDecode = HTMLImageElement.prototype.decode, createUrl = URL.createObjectURL, revokeUrl = URL.revokeObjectURL
HTMLImageElement.prototype.decode = function () { counters.decodes++; return originalDecode.call(this) }
URL.createObjectURL = blob => { counters.createdUrls++; return createUrl.call(URL, blob) }
URL.revokeObjectURL = url => { counters.revokedUrls++; return revokeUrl.call(URL, url) }
const bytes = new Uint8Array(await (await fetch('/synthetic-lease-atlas.png')).arrayBuffer()), input = await fixture(bytes), p = participant()
const f = setup(async () => { counters.meReads++; return Response.json(p) }), scope = f.auth.privacy.capture()
const owner = createAccountOwner({ auth: f.auth, scope,
  bootstrap: { ...bootstrapFor({ userId, eventId, generation: scope.generation }), document: { avatar_manifest: input.document } },
  assets: { binding: input.binding, resources: { async read() { counters.reads++; return bytes } } },
})
type Route = 'world' | 'me' | 'customize' | 'profile'
const retainedCanvases = new Set<HTMLCanvasElement>()
let navigate: (route: Route) => void = () => {}
function App() {
  const [route, setRoute] = useState<Route>('world'); navigate = setRoute
  return <main><p>Synthetic atlas lease harness. World renderer and selected artwork are outside this gate.</p>
    <header data-testid="identity"><AvatarThumbnail auth={f.auth} config={p.profile.avatar.config} source={owner.source!} /></header>
    <h1>{route}</h1>
    {route === 'world' && <p>Account owner remains mounted at the World route boundary.</p>}
    {route === 'me' && <section data-testid="me"><AvatarThumbnail auth={f.auth} config={p.profile.avatar.config} source={owner.source!} /></section>}
    {route === 'customize' && <section data-testid="editor"><AvatarEditor controller={owner.avatar} /></section>}
    {route === 'profile' && <ReviewedProfileAvatar config={p.profile.avatar.config} label="Synthetic profile" options={{ auth: f.auth, scope,
      resolve: async () => { throw new Error('Borrowed profile must not resolve again') }, source: owner.source! }} />}
  </main>
}
const root = createRoot(document.getElementById('root')!); root.render(<App />)
window.__ASSET_LEASE__ = {
  navigate(route) { for (const canvas of document.querySelectorAll('canvas')) retainedCanvases.add(canvas); navigate(route) },
  draft() { return owner.avatar.selectItem('accessory', null) },
  async baseline() {
    const options = { ...owner.source!.options }; delete options.leases
    const compositor = await createReviewedAvatarCompositor({ ...options, signal: new AbortController().signal })
    try {
      const thumbnail = compositor.thumbnail(p.profile.avatar.config, { direction: 'down', animation: 'idle', frame: 0 })
      return Array.from(thumbnail.canvas.getContext('2d')!.getImageData(0, 0, 64, 64).data)
    } finally { compositor.dispose() }
  },
  state() { return { ...counters, disposed: owner.isDisposed(), draft: owner.avatar.read().draft,
    phase: owner.avatar.read().phase, preview: owner.avatar.read().preview.phase, leases: owner.source!.options.leases!.read() } },
  revoke() { for (const canvas of document.querySelectorAll('canvas')) retainedCanvases.add(canvas); f.auth.privacy.changePolicy(scope, 'consent-revoked') },
  erased() { return [...retainedCanvases].every(canvas => canvas.width === 0 && canvas.height === 0) },
  dispose() { root.unmount(); owner.dispose(); f.dispose() },
}
declare global { interface Window { __ASSET_LEASE__: {
  navigate(route: Route): void; draft(): boolean; baseline(): Promise<number[]>; state(): unknown; revoke(): void; erased(): boolean; dispose(): void
} } }
