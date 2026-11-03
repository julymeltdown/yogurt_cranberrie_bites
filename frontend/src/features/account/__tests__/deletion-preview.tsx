import { createRoot } from 'react-dom/client'
import { createSessionController } from '../../auth/model/session'
import { isolatedCookiePolicy } from '../../auth/__tests__/fixtures/data'
import { createAccountController, type AccountDeletionPort } from '../controller'
import { AccountPanel } from '../panel'
import { deletionAccepted, deletionIds as ids, deletionNow, deletionSaga } from './deletion-fixtures'

type Mode = 'pending' | 'retry' | 'failure' | 'complete' | 'legacy' | 'duplicate' | 'unknown' | 'slow' | 'uncertain'
let mode: Mode = (new URLSearchParams(location.search).get('mode') as Mode) ?? 'pending'
let release: (() => void) | undefined
let requests = 0, reads = 0, completed = 0, restricted = 0
const auth = createSessionController({ eventId: ids.event, cookiePolicy: isolatedCookiePolicy() })
auth.session.replace(0, { userId: ids.actor, eventId: ids.event, accessToken: 'SYNTHETIC_ONLY' })
auth.queryClient.setQueryData(['private', 'fixture'], 'PRIVATE old view')
function response() {
  const saga = deletionSaga(mode === 'complete' || mode === 'legacy' || mode === 'duplicate' || mode === 'unknown')
  if (mode === 'legacy') saga.steps = saga.steps.slice(0, 4)
  if (mode === 'duplicate') saga.steps[7]!.target = 'PROFILE'
  if (mode === 'unknown') saga.steps[7]!.target = 'UNAPPROVED_TARGET'
  if (mode === 'retry' || mode === 'failure') {
    saga.steps[7]!.status = mode === 'retry' ? 'RETRY' : 'DEAD_LETTERED'
    saga.steps[7]!.lastError = 'PRIVATE provider failure diagnostic'
  }
  return saga
}
const deletion: AccountDeletionPort = {
  async request(scope) {
    requests++
    if (mode === 'slow') await new Promise<void>(yes => { release = yes }) // Intentionally ignores abort to test final identity fencing.
    if (mode === 'uncertain') throw new Error('PRIVATE uncertain diagnostic')
    return { scope, status: 202, data: deletionAccepted(response()) }
  },
  async status(scope) { reads++; return { scope, status: 200, data: response() } },
}
const account = createAccountController({
  auth, now: () => deletionNow,
  ...(new URLSearchParams(location.search).has('unavailable') ? {} : { deletion }),
  onDeletionComplete() { completed++ }, onDeletionRestricted() { restricted++ },
})
declare global {
  interface Window {
    __DELETION_TARGETS__: {
      mode(value: Mode): void; release(): void; changeIdentity(): void
      snapshot(): { phase: string; progress: unknown; requests: number; reads: number; completed: number; restricted: number; userId: string | null; gate: string; privateCleared: boolean }
    }
  }
}
window.__DELETION_TARGETS__ = {
  mode(value) { mode = value }, release() { release?.() },
  changeIdentity() { auth.session.replace(auth.session.read().generation, { userId: ids.other, eventId: ids.event, accessToken: 'NEXT_SYNTHETIC' }) },
  snapshot() { return { phase: account.read().phase, progress: account.read().progress, requests, reads, completed, restricted,
    userId: auth.session.read().userId, gate: auth.privacy.capture().gate, privateCleared: auth.queryClient.getQueryData(['private', 'fixture']) === undefined } },
}
createRoot(document.getElementById('root')!).render(<><p style={{ padding: '0 1.25rem' }}>Synthetic eight-target deletion qualification. Live Auth and cleanup owners are not connected.</p><AccountPanel auth={auth} controller={account} onBack={() => undefined}/></>)
