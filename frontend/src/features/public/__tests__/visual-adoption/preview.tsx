import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { RouterProvider } from 'react-router-dom'
import { createAppRouter } from '../../../../app/router'
import { createFixturePorts } from '../../../../app/__tests__/fixtures/ports'
import type { SessionReadiness } from '../../../../app/ports'
import { PublicDocumentsProvider } from '../../model/document-provider'
import type { DocumentProvider } from '../../model/documents'
import { syntheticDocument } from '../fixtures/documents'
import '../../../../shared/styles/base.css'
import '../../../../app/shell.css'

const options = new URLSearchParams(location.search)
const readiness = options.get('entry')
const accepted: readonly SessionReadiness[] = ['anonymous', 'checking', 'unknown', 'unavailable', 'error']
const fixture = createFixturePorts(accepted.includes(readiness as SessionReadiness) ? readiness as SessionReadiness : 'anonymous')
fixture.ports.session.replace(fixture.ports.session.read().generation, null)
const mode = options.get('document') ?? 'published'
const provider: DocumentProvider = { syntheticPreview: true, async read(key, signal) {
  await new Promise(resolve => setTimeout(resolve, 500))
  if (signal.aborted) return { status: 'unavailable' }
  if (mode === 'error') throw new Error('PRIVATE_SYNTHETIC_DOCUMENT_FAILURE')
  return syntheticDocument(key)
} }
const route = options.get('route')
history.replaceState(null, '', route && /^\/legal\/(terms|privacy|safety|contact)$/.test(route) ? route : '/')
const router = createAppRouter(fixture.ports)
createRoot(document.getElementById('root')!).render(<StrictMode><PublicDocumentsProvider provider={provider}>
  <RouterProvider router={router} />
</PublicDocumentsProvider></StrictMode>)
