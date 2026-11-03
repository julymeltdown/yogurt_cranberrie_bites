import { createRoot } from 'react-dom/client'
import { createSessionController } from '../../auth/model/session'
import { createInboxController } from '../model/controller'
import { Inbox } from '../ui/Inbox'
// Isolated fixture only. Production composition must supply the canonical session and access port.
const uid = (n: number) => `00000000-0000-4000-8000-${String(n).padStart(12, '0')}`
const auth = createSessionController({ eventId: uid(2), cookiePolicy: { available: true, onInvalidate: () => () => {}, run: (_kind, work) => work(), dispose() {} } })
auth.session.replace(0, { userId: uid(1), eventId: uid(2), accessToken: 'synthetic-inbox-fixture' })
let available = true, navigated = ''
const listeners = new Set<() => void>()
const controller = createInboxController({ auth, access: { read: () => available, subscribe(f) { listeners.add(f); return () => { listeners.delete(f) } } },
  navigate(destination) { navigated = destination.path }, now: () => Date.parse('2026-11-04T00:00:00Z') })
Object.assign(window, { __FE35__: {
  read: controller.read, navigated: () => navigated,
  invalidate: () => controller.invalidate(auth.privacy.capture()),
  available(value: boolean) { available = value; listeners.forEach(f => f()) },
  logout: () => auth.session.replace(auth.session.read().generation, null),
} })
controller.start()
createRoot(document.getElementById('root')!).render(<Inbox controller={controller} />)
