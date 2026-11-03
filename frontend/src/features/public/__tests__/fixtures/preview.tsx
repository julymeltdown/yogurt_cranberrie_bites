import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { RouterProvider } from 'react-router-dom'
import { createAppRouter } from '../../../../app/router'
import { createFixturePorts } from '../../../../app/__tests__/fixtures/ports'
import { PublicDocumentsProvider } from '../../model/document-provider'
import type { DocumentProvider } from '../../model/documents'
import { syntheticDocument } from './documents'
import '../../../../shared/styles/base.css'
import '../../../../app/shell.css'

const options = new URLSearchParams(location.search)
const route = options.get('route') ?? '/'
const fixture = createFixturePorts(options.get('entry') === 'unavailable' ? 'unavailable' : 'anonymous')
fixture.ports.session.replace(fixture.ports.session.read().generation, null)
let mode = options.get('mode') ?? 'published'
const provider: DocumentProvider = { syntheticPreview: true, async read(key) {
  await new Promise(resolve => setTimeout(resolve, 150))
  if (mode === 'error') throw new Error('SYNTHETIC_FE08_RAW_ERROR_MUST_NOT_RENDER')
  if (mode === 'unavailable') return { status: 'unavailable' }
  const document = syntheticDocument(key)
  if (mode === 'hostile') document.document.sections[0]!.links[0]!.to = 'javascript:alert(1)'
  return document
} }
history.replaceState(null, '', route === '/' || /^\/legal\/(terms|privacy|safety|contact)$/.test(route) ? route : '/')
const router = createAppRouter(fixture.ports)
declare global { interface Window { __FE08_TEST__: {
  ready(): void; revoke(): void; mode(value: string): void
  go(path: string): Promise<void>; stats: typeof fixture.stats
} } }
window.__FE08_TEST__ = { ready() { fixture.replace(); fixture.setReadiness('ready') },
  revoke() { fixture.ports.session.replace(fixture.ports.session.read().generation, null); fixture.setReadiness('anonymous') },
  mode(value) { mode = value }, go: async path => { await router.navigate(path) }, stats: fixture.stats }
createRoot(document.getElementById('root')!).render(<StrictMode><PublicDocumentsProvider provider={provider}><RouterProvider router={router} /></PublicDocumentsProvider></StrictMode>)
