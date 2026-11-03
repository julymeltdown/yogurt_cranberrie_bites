import { useLayoutEffect, useRef, useState } from 'react'
import { Link, useLocation, useNavigate } from 'react-router-dom'
import { readCallbackLocation, type CallbackPresentation } from '../model/callback'
import './recovery.css'

const copy = {
  denied: { title: 'Sign-in cancelled', text: 'Provider sign-in was not completed. You can return safely and choose another sign-in option.' },
  'invalid-state': { title: 'Sign-in could not be verified', text: 'This callback cannot complete sign-in. Return to sign in and start again when provider sign-in is available.' },
  unavailable: { title: 'Provider sign-in is unavailable', text: 'Provider sign-in is not available yet. You can use an available sign-in option instead.' },
} as const
export function Callback() {
  const location = useLocation(), navigate = useNavigate(), heading = useRef<HTMLHeadingElement>(null)
  const [projection, setProjection] = useState<CallbackPresentation>({ kind: 'unavailable', target: '/world' })
  const routeKey = useRef(location.key), scrubbing = useRef(false)
  const dirty = Boolean(location.search || location.hash || location.state !== null && location.state !== undefined)
  useLayoutEffect(() => {
    const moved = routeKey.current !== location.key; routeKey.current = location.key
    if (dirty) {
      scrubbing.current = true; setProjection(readCallbackLocation(location.search, location.hash))
      void navigate('/auth/callback', { replace: true, state: null })
    } else {
      if (scrubbing.current) scrubbing.current = false
      else if (moved) setProjection({ kind: 'unavailable', target: '/world' })
      heading.current?.focus({ preventScroll: true })
    }
  }, [dirty, location.key, location.search, location.hash, navigate])
  const state = copy[projection.kind]
  return <main className="auth-recovery" id="main" tabIndex={-1}>
    <Link className="recovery-home" to="/">MCR Concert World</Link>
    <p className="recovery-eyebrow">BACK TO THE CROWD</p>
    {!dirty && <>
      <h1 ref={heading} tabIndex={-1}>{state.title}</h1>
      <p role="status">{state.text}</p>
      <Link className="recovery-submit" to={`/login?returnTo=${encodeURIComponent(projection.target)}`}>Back to sign in</Link>
      <Link className="recovery-link" to={projection.target}>Back to your destination</Link>
    </>}
    <Link className="recovery-link" to="/">Back to home</Link>
  </main>
}
