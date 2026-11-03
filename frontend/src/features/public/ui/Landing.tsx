import { useLayoutEffect, useRef } from 'react'
import { Link, useNavigate } from 'react-router-dom'
import { useAccess, usePorts } from '../../../app/access'
import { PublicFrame } from './PublicFrame'
import { ConcertAction, ConcertIcon, ConcertWindow } from '../../../shared/ui/concert-window'

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
    void navigate('/world', { replace: true })
  }, [ports, key, scope, session.signal, navigate])
  const checking = readiness === 'unknown' || readiness === 'checking'
  const anonymous = readiness === 'anonymous'
  const opening = scope !== null
  const status = opening ? 'Opening the world…' : checking ? 'Checking your session…'
    : readiness === 'unavailable' ? 'World entry is not open yet.'
      : anonymous ? 'Sign in to check entry to the world.' : 'We could not confirm entry. You can check sign-in again.'
  return <PublicFrame appearance="poster">
    <div className="public-hero">
      <ConcertWindow title="One world. Every era." headingLevel={1} icon="star">
      <div className="public-poster">
      <p className="public-eyebrow">An online world for My Chemical Romance fans</p>
      <p className="public-poster-wordmark" aria-hidden="true"><span>My Chemical</span><strong>Romance</strong></p>
      <div className="public-poster-mark" aria-hidden="true"><span>REVENGE</span><ConcertIcon name="star" /><span>PARADE</span><ConcertIcon name="star" /><span>DANGER</span></div>
      <p className="public-poster-motto">The fan<br /><em>social world.</em></p>
      <p className="public-lead">Meet the crowd. Share the night. Leave a mark.</p>
      <div className="public-entry-action">
        {anonymous ? <Link className="public-primary" to="/login?returnTo=%2Fworld" aria-describedby="public-entry-status">Sign in to enter</Link>
          : <ConcertAction className="public-primary" type="button" disabled aria-describedby="public-entry-status">{opening ? 'Opening the world…' : checking ? 'Checking entry…' : 'Enter the world'}</ConcertAction>}
        <p id="public-entry-status" role="status" aria-live="polite">{status}</p>
        {ports.readiness.retry && !checking && !opening && !anonymous && <button className="public-text-link" type="button" onClick={() => ports.readiness.retry?.()}>Check entry again</button>}
      </div>
      <a className="public-text-link" href="#public-about">About this space</a>
      </div>
      </ConcertWindow>
    </div>
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
  </PublicFrame>
}
