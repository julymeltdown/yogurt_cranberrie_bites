import { useLayoutEffect, useRef } from 'react'
import { Link, useNavigate } from 'react-router-dom'
import { useAccess, usePorts } from '../../../app/access'
import { PublicFrame } from './PublicFrame'
import { ConcertAction } from '../../../shared/ui/concert-window'
import { SelectedStaticGallery } from '../../../entities/avatar/selected-static'

export function Landing() {
  const ports = usePorts(), { readiness, scope, session } = useAccess(), navigate = useNavigate()
  const key = scope ? `${scope.generation}:${scope.userId}:${scope.eventId}` : null
  const redirected = useRef<string | null>(null)
  useLayoutEffect(() => {
    // A rendered ready identity may be revoked before layout setup. Recheck the
    // canonical source before dispatch, never a cached person/event projection.
    if (!scope || key === redirected.current || session.signal.aborted || !ports.session.isCurrent(scope)
      || ports.readiness.read() !== 'ready') return
    redirected.current = key
    // Dispatch after the current navigation commit; a nested navigate during a just-committed
    // route change is ignored by the router on a warm module graph.
    queueMicrotask(() => {
      if (redirected.current !== key || session.signal.aborted || !ports.session.isCurrent(scope)
        || ports.readiness.read() !== 'ready') return
      void navigate('/world', { replace: true })
    })
  }, [ports, key, scope, session.signal, navigate])
  const checking = readiness === 'unknown' || readiness === 'checking'
  const anonymous = readiness === 'anonymous'
  const opening = scope !== null
  const status = opening ? 'Opening the world…' : checking ? 'Checking your session…'
    : readiness === 'unavailable' ? 'World entry is not open yet.'
      : anonymous ? 'Sign in to check entry to the world.' : 'We could not confirm entry. You can check sign-in again.'
  return <PublicFrame appearance="poster">
    <div className="public-hero">
      <div className="landing-poster-frame">
      <div className="public-poster" aria-labelledby="landing-title">
      <h1 className="landing-accessible-title" id="landing-title">One world. Every era.</h1>
      <SelectedStaticGallery count={4} className="landing-crowd" />
      <div className="landing-poster-copy">
        <p className="public-poster-motto">The MCR<br />fan social world.</p>
        <p className="public-poster-invitation">Meet <span>·</span> Trade <span>·</span> Hang out</p>
        <p className="public-poster-show">At the show.</p>
      </div>
      <div className="public-entry-action">
        {anonymous ? <Link className="public-primary" to="/login?returnTo=%2Fworld" aria-label="Sign in to enter" aria-describedby="public-entry-status"><span className="landing-control-label">Enter the world</span><span aria-hidden="true">→</span></Link>
          : <ConcertAction className="public-primary" type="button" disabled aria-describedby="public-entry-status">{opening ? 'Opening the world…' : checking ? 'Checking entry…' : 'Enter the world'}<span aria-hidden="true">→</span></ConcertAction>}
      </div>
      </div>
      </div>
      <div className="landing-entry-feedback">
        <p id="public-entry-status" role="status" aria-live="polite">{status}</p>
        {ports.readiness.retry && !checking && !opening && !anonymous && <button className="public-text-link" type="button" onClick={() => ports.readiness.retry?.()}>Check entry again</button>}
      </div>
    </div>
    <details className="public-about-disclosure"><summary>About this space</summary>
    <section className="public-about" id="public-about" tabIndex={-1} aria-labelledby="public-about-title">
      <h2 id="public-about-title">A place for the crowd.</h2>
      <p>A shared world, event conversation and pixel wall. Community features are still being prepared.</p>
      <dl className="public-features">
        <div><dt>Meet</dt><dd>Find fellow fans.</dd></div>
        <div><dt>Share</dt><dd>Stories and plans.</dd></div>
        <div><dt>Create</dt><dd>One pixel at a time.</dd></div>
      </dl>
      <Link className="public-text-link" to="/legal/safety">Read the safety information</Link>
    </section>
    </details>
  </PublicFrame>
}
