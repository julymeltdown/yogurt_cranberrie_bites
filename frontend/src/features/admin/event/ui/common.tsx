import { Fragment, useEffect, useId, useRef, useSyncExternalStore, type ReactNode, type RefObject } from 'react'
import type { OperationsController, OperationsState } from '../model/controller'
import { ConcertWindow } from '../../../../shared/ui/concert-window'
import './operations.css'
export const useOperations = (controller: OperationsController) => useSyncExternalStore(controller.subscribe, controller.read, controller.read)
export function useReviewFocus(state: OperationsState, container: RefObject<HTMLDivElement | null>) {
  const lastOpener = useRef<HTMLElement | null>(null)
  useEffect(() => {
    const track = (event: FocusEvent) => {
      if (event.target instanceof HTMLElement && !container.current?.contains(event.target)) lastOpener.current = event.target
    }
    document.addEventListener('focusin', track)
    return () => document.removeEventListener('focusin', track)
  }, [container])
  useEffect(() => {
    if (state.command?.stage !== 'review') return
    const opener = lastOpener.current
    container.current?.querySelector<HTMLElement>('h2')?.focus()
    return () => { if (opener instanceof HTMLElement && opener.isConnected && !opener.matches(':disabled')) opener.focus() }
  }, [state.command?.stage, container])
}
export function OperationsGate({ state, children }: { state: OperationsState; children: ReactNode }) {
  return state.access === 'ready' ? children : <OperationsFrame title="Operations unavailable">
    <p role="status">A current server-qualified event operator role is required. No private operations data is open.</p></OperationsFrame>
}
export function OperationsFrame({ title, children }: { title: string; children: ReactNode }) {
  return <main className="event-ops"><ConcertWindow title={title} headingLevel={1}>{children}</ConcertWindow></main>
}
export function Field({ label, children }: { label: string; children: (id: string) => ReactNode }) {
  const id = useId()
  return <div className="ops-field"><label htmlFor={id}>{label}</label>{children(id)}</div>
}
export function OperationFeedback({ state, controller }: { state: OperationsState; controller: OperationsController }) {
  const action = state.command
  return <>
    {state.loading && <p role="status">Reading current authorized information…</p>}
    {state.message && <p role="status">{state.message}</p>}
    {action && <section className="ops-confirm" aria-labelledby="operation-review-title">
      <h2 id="operation-review-title" tabIndex={-1}>{action.stage === 'review' ? 'Confirm operational change' : 'Command outcome'}</h2>
      <dl><dt>Operation</dt><dd>{{ setConcertSettings: 'Update settings', setConcertEventPhase: 'Change event phase', setConcertPlace: 'Update place', createConcertNotice: 'Create notice', updateConcertNotice: 'Update notice' }[action.operation]}</dd><dt>Target</dt><dd>{action.target ?? 'New notice in the current event'}</dd>
        <dt>Expected revision</dt><dd>{action.expectedRevision === 0 ? 'Create revision 1' : action.expectedRevision}</dd>
        <dt>Status</dt><dd>{action.stage}</dd></dl>
      <dl>{action.changes.map(([label, value]) => <Fragment key={label}><dt>{label}</dt><dd>{value}</dd></Fragment>)}</dl>
      <p role="status">{action.message}</p>
      {action.stage === 'review' && <div className="ops-actions"><button type="button" onClick={() => { void controller.confirm() }}>Confirm this change</button>
        <button type="button" onClick={controller.cancel}>Cancel confirmation</button></div>}
      {['pending', 'unknown'].includes(action.stage) && <button type="button" disabled={state.loading} onClick={() => { void controller.checkStatus() }}>Check command status</button>}
    </section>}
  </>
}
