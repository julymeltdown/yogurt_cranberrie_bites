import { useLayoutEffect, useRef } from 'react'
import { Link, useNavigate } from 'react-router-dom'
import { useAccess, usePorts } from '../../../app/access'
import { PublicFrame } from './PublicFrame'

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
  return <PublicFrame>
    <section className="public-hero" aria-labelledby="public-title">
      <p className="public-eyebrow">An online world for My Chemical Romance fans</p>
      <h1 id="public-title">One world.<br />{' '}Every era.</h1>
      <p className="public-lead">The music brought us here. A shared 2D world is being built for the crowd around it.</p>
      <div className="public-entry-action">
        {anonymous ? <Link className="public-primary" to="/login?returnTo=%2Fworld" aria-describedby="public-entry-status">Sign in to enter</Link>
          : <button className="public-primary" type="button" disabled aria-describedby="public-entry-status">{opening ? 'Opening the world…' : checking ? 'Checking entry…' : 'Enter the world'}</button>}
        <p id="public-entry-status" role="status" aria-live="polite">{status}</p>
        {ports.readiness.retry && !checking && !opening && !anonymous && <button className="public-text-link" type="button" onClick={() => ports.readiness.retry?.()}>Check entry again</button>}
      </div>
      <a className="public-text-link" href="#public-about">About this space</a>
    </section>
    <section className="public-about" id="public-about" tabIndex={-1} aria-labelledby="public-about-title">
      <p className="public-eyebrow">Built around a shared space</p>
      <h2 id="public-about-title">A place for the crowd.</h2>
      <p>Meet other fans in the world, follow the event conversation, and leave a mark together. These community features are still being prepared.</p>
      <dl className="public-features">
        <div><dt>Find your people</dt><dd>A world and community for meeting fellow fans.</dd></div>
        <div><dt>Share the night</dt><dd>Event stories, plans, and fan giveaways.</dd></div>
        <div><dt>Make something together</dt><dd>A shared pixel wall, one mark at a time.</dd></div>
      </dl>
      <Link className="public-text-link" to="/legal/safety">Read the safety information</Link>
    </section>
  </PublicFrame>
}
