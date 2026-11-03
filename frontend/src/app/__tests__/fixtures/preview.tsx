import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { createBrowserRouter, RouterProvider } from 'react-router-dom'
import { Button } from '../../../shared/ui'
import '../../../shared/styles/base.css'
import '../../shell.css'
import { createAppRoutes } from '../../router'
import { ShellLink } from '../../navigation'
import { createFixturePorts, giftPath, ids } from './ports'

if (!import.meta.env.DEV) throw new Error('FE07 fixture is development-only')
const fixture = createFixturePorts('checking')
// Only this explicitly opened fixture entry changes the URL/installs synthetic credentials.
// The real application entry does not import this file or offer a fixture query switch.
history.replaceState(null, '', giftPath)
const router = createBrowserRouter(createAppRoutes(fixture.ports, { panels: {
  '/gifts': <><p className="shell-kicker">Deterministic route fixture</p>
    <ShellLink id="fixture-gift" className="shell-link" to={giftPath}>Open test giveaway</ShellLink>
    {Array.from({ length: 24 }, (_, index) => <p key={index}>Scroll check — test content only.</p>)}</>,
  '/gifts/:giveawayId': <><p>Test destination. No giveaway, stock or recipient data is loaded.</p>
    <ShellLink id="fixture-manage" className="shell-link" to={`${giftPath}/manage`}>Manage test giveaway</ShellLink></>,
  '/gifts/:giveawayId/manage': <><label htmlFor="fixture-draft">Test draft</label>
    <input className="mcr-input" id="fixture-draft" /></>,
} }))
Object.assign(window, { __FE07_STATS__: fixture.stats })
const controls = document.getElementById('fixture-controls')!
controls.style.cssText = 'position:fixed;top:0;right:0;z-index:30;font-size:12px;max-width:300px;background:#faf6ed;border:1px solid #201d1d;padding:4px'
createRoot(controls).render(<details><summary>FE07 test controls</summary>
  <p>Fixture only. No real game or connection.</p>
  <Button onClick={() => fixture.setReadiness('ready')}>Authorize fixture</Button>
  <Button onClick={() => fixture.setReadiness('anonymous')}>Anonymous fixture</Button>
  <Button onClick={() => fixture.replace()}>Switch account</Button>
  <Button onClick={() => fixture.replace(ids.other, ids.nextEvent)}>Switch event</Button>
  <Button onClick={() => void router.navigate('/admin')}>Admin exit</Button>
  <Button onClick={() => void router.navigate('/')}>Public exit</Button>
</details>)
createRoot(document.getElementById('root')!).render(<StrictMode><RouterProvider router={router} /></StrictMode>)
