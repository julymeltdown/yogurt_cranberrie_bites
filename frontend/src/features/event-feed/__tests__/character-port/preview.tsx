import { useState } from 'react'
import { createRoot } from 'react-dom/client'
import { EventFeed, type RenderPostAuthor } from '../../index'
import { WholeCharacterView, createWholeCharacterRuntime } from '../../../../entities/avatar/whole-character'
import { ConcertWindow } from '../../../../shared/ui/concert-window'
import { setup } from '../fixtures'
import { rows, source } from './data'
import '../../../../shared/styles/base.css'
import './preview.css'
const mode = new URLSearchParams(location.search).get('mode') ?? 'loaded'
let changed = false
const reads: { path: string; method: string }[] = [], assets: string[] = [], actions: string[] = [], authors = new Set<string>()
const f = setup(async input => {
  const r = input as Request, url = new URL(r.url); reads.push({ path: url.pathname, method: r.method })
  if (url.pathname.endsWith('/notices')) return Response.json({ items: [], next_cursor: null })
  if (mode === 'denied') return Response.json({ code: 'RESOURCE_UNAVAILABLE', message: 'TEST private error', details: { reason: 'RESOURCE_UNAVAILABLE' } }, { status: 403 })
  return Response.json({ items: rows.map((r, i) => changed && i === 0 ? { ...r, revision: 2, author: { ...r.author, avatar: rows[5]!.author.avatar } } : r), next_cursor: null })
})
const runtime = createWholeCharacterRuntime({ privacy: f.auth.privacy, manifest: source, pin: source, origin: location.origin,
  fetch: (url, init) => { assets.push(String(url)); return fetch(url, init) } })
const portrait: RenderPostAuthor = author => {
  authors.add(JSON.stringify([author.user_id, author.display_name, author.avatar]))
  return <WholeCharacterView runtime={runtime} config={author.avatar.config} label={`${author.display_name}: selected character`} />
}
const root = createRoot(document.getElementById('root')!)
function App() {
  const [visible, setVisible] = useState(true), [callback, setCallback] = useState(mode !== 'fallback')
  return <><main><ConcertWindow title="FEED" onClose={() => actions.push('close')}>
    {visible && <EventFeed controller={f.controller} renderAuthor={callback ? mode === 'null' ? () => null : portrait : undefined}
      onPost={id => actions.push(`thread:${id}`)} onCompose={() => actions.push('compose')} onLink={link => actions.push(`link:${link.target_type}:${link.target_id}`)} />}
  </ConcertWindow></main><aside aria-label="Qualification controls"><p>Fixture-only loaded canonical authors, simulated HTTP, real reviewed art. Production Feed readiness is unchanged.</p>
    <button onClick={() => setVisible(v => !v)}>{visible ? 'Unmount feed' : 'Remount feed'}</button>
    <button onClick={() => setCallback(v => !v)}>{callback ? 'Omit portrait callback' : 'Install portrait callback'}</button>
    <button onClick={() => { changed = true; void f.controller.loadFeed() }}>Reload changed author</button>
    <button onClick={() => f.setAccess({ available: true, readOnly: true })}>Make read-only</button>
    <button onClick={() => f.auth.privacy.changePolicy(f.auth.privacy.capture(), 'suspended')}>Revoke privacy</button>
  </aside></>
}
root.render(<App />); f.controller.start()
window.feedCharacterPort = {
  state: () => ({ reads, assets, actions, authors: [...authors], runtime: runtime.diagnostics(), current: runtime.isCurrent(), data: f.controller.read().feed.data }),
  dispose() { root.unmount(); f.dispose(); runtime.dispose() },
}
declare global { interface Window { feedCharacterPort: { state(): unknown; dispose(): void } } }
