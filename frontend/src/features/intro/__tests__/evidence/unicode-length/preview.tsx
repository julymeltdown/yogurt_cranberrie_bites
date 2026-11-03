// Synthetic native-browser fixture for the actual editor and unchanged controller. No live API.
import { createRoot } from 'react-dom/client'
import { MemoryRouter } from 'react-router-dom'
import { IntroEditor } from '../../../IntroEditor'
import { createIntroController } from '../../../controller'
import { eventId, harness, introId, userId } from '../../../../people/__tests__/fixtures/harness'
const h = harness(), controller = createIntroController({ ...h.options, now: () => Date.parse('2026-11-07T12:00:00Z') })
let row = { intro_id: introId, event_id: eventId, owner_id: userId, text: 'Welcome to the concert.', age_bounds: { minimum_age: null, maximum_age_exclusive: null }, expires_at: null, created_at: '2026-11-07T08:00:00Z', state: 'DRAFT', revision: 1 }
h.respond(async request => {
  if (request.method === 'PUT') { const { expected_revision: _, ...draft } = await request.json(); row = { ...row, ...draft, revision: row.revision + 1 } }
  return Response.json(row)
})
controller.subscribe(() => {
  document.getElementById('native-state')!.textContent = JSON.stringify({
    phase: controller.read().load.phase, writes: h.requests.filter(r => r.method === 'PUT').length,
    savedLength: [...(controller.read().saved?.text ?? '')].length, draftLength: [...controller.read().draft.text].length,
  })
})
createRoot(document.getElementById('root')!).render(<MemoryRouter><IntroEditor controller={controller} /></MemoryRouter>)
window.addEventListener('pagehide', () => { controller.dispose(); h.dispose() }, { once: true })
