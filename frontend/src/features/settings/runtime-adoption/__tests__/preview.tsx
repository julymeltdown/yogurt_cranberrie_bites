import { createRoot } from 'react-dom/client'
import { MemoryRouter } from 'react-router-dom'
import { SettingsPage } from '../../SettingsPage'
import { fixture } from './fixture'

const f = fixture()
await f.controller.load()
window.__FE34_RUNTIME__ = f
declare global { interface Window { __FE34_RUNTIME__: ReturnType<typeof fixture> } }
createRoot(document.getElementById('root')!).render(<MemoryRouter><p>Controlled FE34 fixture: actual SettingsPage, Auth/privacy, ME, ThemeController and adapter; instrumented effects/audio, no Phaser, live server or production artwork.</p><SettingsPage controller={f.controller} auth={f.auth} runtime={f.adapter} theme={f.theme} /></MemoryRouter>)
