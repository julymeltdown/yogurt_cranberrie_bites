import { useEffect, useId, useSyncExternalStore } from 'react'
import { Link } from 'react-router-dom'
import type { IntroController } from './controller'
import '../people/people.css'
export function IntroEditor({ controller }: { controller: IntroController }) {
  const state = useSyncExternalStore(controller.subscribe, controller.read, controller.read), id = useId(), draft = state.draft
  useEffect(() => { controller.start() }, [controller])
  const locked = !state.available || state.command.locked || state.load.phase === 'loading'
  function bound(field: 'minimum_age' | 'maximum_age_exclusive', value: string) {
    controller.edit({ ...draft, age_bounds: { ...draft.age_bounds, [field]: value === '' ? null : Number(value) } as typeof draft.age_bounds })
  }
  return <section className="intro-surface" aria-labelledby={`${id}-title`}><Link to="/meet/people">Back to people</Link><h1 id={`${id}-title`}>YOUR INTRO</h1><p>Introduce yourself. Your audience conditions stay private and are enforced by the server.</p>
    {state.readOnly && <p role="status">This event is read-only. You can keep editing a local draft; publishing and saving are unavailable.</p>}
    <p role="status" aria-live="polite">{state.load.phase === 'loading' ? 'Loading your introduction…' : state.command.message || state.message || (state.saved ? `Saved state: ${state.saved.state}` : 'Your introduction has not been verified by the server.')}</p>
    {['pending', 'uncertain', 'reconciling', 'failed'].includes(state.command.phase) && state.command.locked && <button onClick={() => void controller.checkStatus()}>Check original command</button>}
    <form onSubmit={event => { event.preventDefault(); void controller.save() }}>
      <label htmlFor={`${id}-text`}>Introduction</label><textarea id={`${id}-text`} value={draft.text} maxLength={1000} disabled={locked} onChange={event => controller.edit({ ...draft, text: event.target.value })} />
      <fieldset disabled={locked}><legend>Private audience conditions</legend><p>Leave both bounds empty for no age condition. Otherwise use a minimum inclusive age and a maximum exclusive age. These are service sharing conditions, not concert admission rules.</p>
        <label htmlFor={`${id}-min`}>Minimum age · inclusive</label><input id={`${id}-min`} type="number" inputMode="numeric" min={14} max={100} value={draft.age_bounds.minimum_age ?? ''} onChange={event => bound('minimum_age', event.target.value)} />
        <label htmlFor={`${id}-max`}>Maximum age · exclusive</label><input id={`${id}-max`} type="number" inputMode="numeric" min={15} max={101} value={draft.age_bounds.maximum_age_exclusive ?? ''} onChange={event => bound('maximum_age_exclusive', event.target.value)} />
        <p>Unknown, expired or unavailable verification cannot satisfy a restricted audience.</p>
      </fieldset>
      <label htmlFor={`${id}-expiry`}>Expiry · UTC (optional)</label><input id={`${id}-expiry`} type="text" placeholder="2026-11-08T00:00:00Z" value={draft.expires_at ?? ''} disabled={locked} onChange={event => controller.edit({ ...draft, expires_at: event.target.value || null })} />
      <div className="intro-actions"><button type="submit" disabled={locked || state.readOnly || state.needsReview}>Save introduction</button><button type="button" disabled={locked} onClick={() => controller.cancel()}>Cancel changes</button></div>
    </form>
    <div className="intro-actions"><button disabled={locked || state.readOnly || state.needsReview || state.saved?.state !== 'DRAFT'} onClick={() => void controller.transition('publish')}>Publish introduction</button>
      <button disabled={locked || state.needsReview || !state.saved || state.saved.state === 'CLOSED'} onClick={() => void controller.transition('close')}>Close introduction</button>
      <button disabled={locked} onClick={() => void controller.load()}>Load latest introduction</button></div>
    {state.latest && <section aria-label="Introduction reconciliation"><h2>Latest revision {state.latest.revision}</h2><p className="intro-user-text">{state.latest.text}</p><div className="intro-actions"><button onClick={() => controller.reconcile('keep-draft')}>Keep my draft</button><button onClick={() => controller.reconcile('use-server')}>Use saved introduction</button></div></section>}
    <p>Closing removes future discovery and sharing access. It cannot erase information someone already saw.</p><Link to="/me/age">Age &amp; privacy</Link>
  </section>
}
