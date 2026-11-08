import { useEffect, useId, useSyncExternalStore } from 'react'
import { Button } from '../../shared/ui'
import type { IdentityController } from './controller'
import './identity.css'

/**
 * The start step: one button that asks the server for a provider authorization
 * URL and then hands the browser to NAVER.
 *
 * Two properties matter here:
 *
 * * the URL is never built by this component. It comes from the server, which
 *   also decides the state and the redirect URI, and the controller validates it
 *   before the browser is sent anywhere;
 * * nothing is submitted on the user's behalf. The return path is a constant
 *   from the closed allowlist, so the user lands on a screen they can read, not
 *   on a resumed draft.
 */
export function VerificationStart({
  controller,
  returnPath = '/me/identity',
}: {
  controller: IdentityController
  returnPath?: '/me/identity' | '/me/settings' | '/me/profile'
}) {
  const state = useSyncExternalStore(controller.subscribe, controller.read, controller.read)
  const id = useId()

  useEffect(() => {
    controller.start()
  }, [controller])

  return (
    <section className="identity-surface" aria-labelledby={`${id}-title`}>
      <h1 id={`${id}-title`}>Connect NAVER</h1>
      <p>
        NAVER will ask you to sign in and to share your age and gender. This product never asks
        you to type either value, and it never sees your NAVER password.
      </p>
      <p>
        You will come back here afterwards. Verification is optional and is not required for the
        rest of the product.
      </p>
      {state.message && (
        <p role="status" aria-live="polite">
          {state.message}
        </p>
      )}
      <div className="identity-actions">
        <Button
          disabled={!controller.canVerify()}
          busy={state.busy}
          onClick={() => void controller.startVerification(returnPath)}
        >
          Continue to NAVER
        </Button>
      </div>
    </section>
  )
}
