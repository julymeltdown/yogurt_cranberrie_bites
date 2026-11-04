// QA07 ONLY. Published UI/controller with explicit synthetic authority and HTTP.
import { useRef, useState } from 'react'
import { createRoot } from 'react-dom/client'
import { Dialog, Button } from '../../../src/shared/ui'
import { SharingRequest } from '../../../src/features/social-sharing'
import { setup, request } from '../../../src/features/social-sharing/__tests__/fixtures/data'
import '../../../src/shared/styles/base.css'

if (!import.meta.env.DEV) throw Error('QA07 fixture requires development mode')
function Sheet() {
  const [open, setOpen] = useState(false)
  const opener = useRef<HTMLButtonElement>(null), initial = useRef<HTMLInputElement>(null)
  return <main><p>QA07 synthetic sheet; actual shared Dialog.</p>
    <Button ref={opener} onClick={() => setOpen(true)}>Open pickup sheet</Button>
    <a href="#outside">Outside navigation</a>
    <Dialog open={open} title="Pickup sheet fixture" presentation="sheet" onClose={() => setOpen(false)} initialFocusRef={initial} returnFocusRef={opener}>
      <label>Pickup note<input ref={initial} /></label><Button onClick={() => setOpen(false)}>Done</Button>
    </Dialog>
  </main>
}
let row = structuredClone(request)
const social = new URLSearchParams(location.search).has('social')
if (social) {
  const f = setup(async input => {
    const r = input as Request, path = new URL(r.url).pathname
    if (path.endsWith('/reveal')) return Response.json({ request_id: row.request_id, platform: 'INSTAGRAM', owner_id: row.recipient_id, handle_version: row.consents.find(c => c.platform === 'INSTAGRAM')!.recipient_handle_version, handle: 'qa07_synthetic_private', grant_revision: row.revision, expires_at: row.expires_at })
    if (path.endsWith('/revoke')) row = { ...row, state: 'REVOKED', revision: row.revision + 1 }
    return Response.json(row)
  })
  void f.controller.loadRequest(request.request_id)
  createRoot(document.getElementById('root')!).render(<><p>QA07 synthetic approved consent fixture; no live reciprocal consent.</p><SharingRequest controller={f.controller} retry={() => { void f.controller.loadRequest(request.request_id) }} /></>)
  window.addEventListener('pagehide', f.dispose, { once: true })
} else createRoot(document.getElementById('root')!).render(<Sheet />)
