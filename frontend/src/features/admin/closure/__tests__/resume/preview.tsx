import { createRoot } from 'react-dom/client'
import { EventClosure } from '../../EventClosure'
import { fixture } from './fixture'
import './preview.css'

const f = fixture()
const hooks = {
  counts: () => ({ posts: f.posts().length, admissions: f.server.admissions, effects: Object.fromEntries(f.server.effects) }),
  command: () => f.controller.read().command,
  mode(value: string) { f.server.mode = value },
  pending: () => f.settle(),
  fail: () => f.failAgain(),
  complete() { f.complete(); f.syncContext() },
  conflict() { f.failAgain() },
  revoke() { f.server.operator = false; f.update(null) },
  logout() { f.session.replace(f.session.read().generation, null) },
  dispose: () => f.cleanup(),
}
declare global { interface Window { __CLOSURE_RESUME__: typeof hooks } }
window.__CLOSURE_RESUME__ = hooks
await f.open()
createRoot(document.getElementById('root')!).render(<main><p className="fixture-caption">FE39 / deterministic authenticated HTTP fixture / no live cleanup acceptance</p><EventClosure controller={f.controller} /></main>)
