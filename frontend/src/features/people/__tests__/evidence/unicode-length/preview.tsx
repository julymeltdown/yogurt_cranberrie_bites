// Synthetic HTTP fixture for the actual discovery, profile and search consumers. No live API.
import { createRoot } from 'react-dom/client'
import { MemoryRouter, Route, Routes, useParams } from 'react-router-dom'
import { People } from '../../../People'
import { PublicProfile } from '../../../../profile/PublicProfile'
import { harness, publicIntro } from '../../fixtures/harness'
const h = harness(), row = publicIntro()
row.text = '😀'.repeat(1000); row.author.display_name = '😀'.repeat(40); (row.author as { bio: string | null }).bio = '😀'.repeat(160)
h.respond(async request => Response.json(new URL(request.url).pathname.endsWith('/intros') ? { items: [row], next_cursor: null } : row.author))
h.controller.subscribe(() => {
  document.getElementById('native-state')!.textContent = JSON.stringify({
    phase: h.controller.read().page.phase, requests: h.requests.length,
    query: h.controller.read().query, input: h.controller.read().input,
  })
})
function Profile() { const { userId = '' } = useParams(); return <PublicProfile controller={h.controller} userId={userId} /> }
createRoot(document.getElementById('root')!).render(<MemoryRouter initialEntries={['/meet/people']}><Routes>
  <Route path="/meet/people" element={<People controller={h.controller} />} />
  <Route path="/people/:userId" element={<Profile />} />
</Routes></MemoryRouter>)
window.addEventListener('pagehide', h.dispose, { once: true })
