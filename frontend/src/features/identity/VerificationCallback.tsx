import { useEffect, useId, useSyncExternalStore } from 'react'
import { Link } from 'react-router-dom'
import { Button } from '../../shared/ui'
import { isSuccess, resultMessage, type ResultMarker } from './api/identity'
import type { IdentityController } from './controller'
import './identity.css'

/**
 * The callback landing page.
 *
 * The provider redirects the browser to the server, the server answers `303`
 * with a `Location` of `{return path}?identity={marker}`, and the browser lands
 * here. This component therefore does exactly three things:
 *
 * 1. reports the marker in one sentence, or a provider-independent failure
 *    sentence when there is no marker;
 * 2. re-reads the owner's verification so the page is truthful;
 * 3. offers a link back to where the user was.
 *
 * What it deliberately does **not** do is resume anything. The task document
 * forbids auto-submitting the work the user was doing before verification, and
 * a landing page that replayed a draft would be exactly that. There is no
 * pending action to resume, no retry of an earlier request, and no form on this
 * page.
 */
export function VerificationCallback({
  controller,
  marker,
  returnPath = '/me/identity',
}: {
  controller: IdentityController
  /** The marker the server appended. Already validated against the closed set. */
  marker: ResultMarker | null
  returnPath?: '/me/identity' | '/me/settings' | '/me/profile'
}) {
  const state = useSyncExternalStore(controller.subscribe, controller.read, controller.read)
  const id = useId()

  useEffect(() => {
    // A re-read only. Nothing is submitted.
    controller.start()
  }, [controller])

  const success = isSuccess(marker)
  const message = resultMessage(marker)

  return (
    <section className="identity-surface" aria-labelledby={`${id}-title`}>
      <h1 id={`${id}-title`}>{success ? 'Verification complete' : 'Verification'}</h1>
      <p role={success ? 'status' : 'alert'} aria-live="polite">
        {message}
      </p>
      {state.phase === 'loading' && <p role="status">Checking your verification…</p>}
      {state.phase === 'error' && (
        <p role="status">Your verification status could not be read right now.</p>
      )}
      {state.verification && (
        <p>
          Current status:{' '}
          <strong>
            {state.verification.status === 'VERIFIED' ? 'verified' : state.verification.status.toLowerCase().replaceAll('_', ' ')}
          </strong>
          .
        </p>
      )}
      <div className="identity-actions">
        <Link to={returnPath}>Back to where you were</Link>
        {!success && (
          <Button variant="secondary" onClick={() => void controller.startVerification(returnPath)}>
            Try again
          </Button>
        )}
      </div>
      <p className="identity-prompt__note">
        Nothing you were doing before verification was submitted or repeated.
      </p>
    </section>
  )
}
