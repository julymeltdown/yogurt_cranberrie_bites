import { useEffect, useId, useRef, useState, useSyncExternalStore } from 'react'
import { Button, Dialog } from '../../shared/ui'
import {
  expiryLabel,
  genderLabel,
  statusLabel,
  type IdentityController,
} from './controller'
import './identity.css'

/**
 * The owner's own verification view.
 *
 * Everything this component shows is owner-only. The derived age and the gender
 * bucket appear here and nowhere else: a viewer-facing surface never receives
 * them, because the server sends `claims` only on this operation and only when
 * the claim is live.
 *
 * The copy says plainly what verification is for and that it is not required
 * for the rest of the product, so a user who never verifies is not left
 * wondering whether they are locked out.
 */
export function IdentityStatus({ controller }: { controller: IdentityController }) {
  const state = useSyncExternalStore(controller.subscribe, controller.read, controller.read)
  const id = useId()
  const [confirming, setConfirming] = useState(false)
  const disconnectRef = useRef<HTMLButtonElement>(null)
  const cancelRef = useRef<HTMLButtonElement>(null)

  useEffect(() => {
    controller.start()
  }, [controller])

  const verification = state.verification
  const status = verification?.status ?? 'NOT_VERIFIED'
  const claims = verification?.claims ?? null
  const expires = expiryLabel(verification?.expires_at ?? null)
  const markerMessage = controller.markerMessage()
  const titleId = `${id}-title`
  const statusId = `${id}-status`

  return (
    <section className="identity-surface" aria-labelledby={titleId}>
      <header>
        <h1 id={titleId}>NAVER verification</h1>
        <p>
          NAVER verification confirms your age and gender so you can see, and be matched by,
          introductions that carry an age or gender restriction. It is not required for the rest
          of the product: your profile, photos, room, favorites and unrestricted introductions
          all work without it.
        </p>
      </header>

      {markerMessage && (
        <div className="identity-report" role="status" aria-live="polite">
          <p>{markerMessage}</p>
          <Button variant="quiet" onClick={() => controller.dismissMarker()}>
            Dismiss
          </Button>
        </div>
      )}

      {state.message && !markerMessage && (
        <p className="identity-message" role="status" aria-live="polite">
          {state.message}
        </p>
      )}

      {state.phase === 'loading' && !verification ? (
        <p role="status">Checking your verification…</p>
      ) : state.phase === 'error' && !verification ? (
        <div role="status">
          <p>{state.message || 'Verification status is unavailable.'}</p>
          <Button onClick={() => void controller.reload()}>Try again</Button>
        </div>
      ) : (
        <>
          <p id={statusId} className="identity-status" data-status={status}>
            <strong>{statusLabel(status)}</strong>
          </p>

          {status === 'VERIFIED' && claims && (
            <dl className="identity-claims" aria-describedby={statusId}>
              <div>
                <dt>Age</dt>
                {/* The derived age, owner-only. It is never sent to a viewer. */}
                <dd>{claims.age}</dd>
              </div>
              <div>
                <dt>Gender</dt>
                <dd>{genderLabel(claims.gender_bucket)}</dd>
              </div>
              {expires && (
                <div>
                  <dt>Valid until</dt>
                  <dd>
                    <time dateTime={verification?.expires_at ?? undefined}>{expires}</time>
                  </dd>
                </div>
              )}
            </dl>
          )}

          {status !== 'VERIFIED' && expires && (
            <p>
              Last verification ended <time dateTime={verification?.expires_at ?? undefined}>{expires}</time>.
            </p>
          )}

          {status === 'EXPIRED' && (
            <p>Verification expires after 90 days. Reconnect NAVER to verify again.</p>
          )}
          {status === 'REVOKED' && (
            <p>You disconnected NAVER. Reconnect to verify your age and gender again.</p>
          )}
          {status === 'PROVIDER_UNAVAILABLE' && (
            <p>The verification service is unavailable right now. Nothing was changed.</p>
          )}

          <div className="identity-actions">
            {status === 'VERIFIED' ? (
              <>
                <Button
                  variant="secondary"
                  disabled={!controller.canVerify()}
                  onClick={() => void controller.startVerification()}
                >
                  Reconnect
                </Button>
                <Button
                  ref={disconnectRef}
                  variant="danger"
                  disabled={!controller.canDisconnect()}
                  onClick={() => setConfirming(true)}
                >
                  Disconnect
                </Button>
              </>
            ) : (
              <Button
                disabled={!controller.canVerify()}
                busy={state.busy}
                onClick={() => void controller.startVerification()}
              >
                {status === 'NOT_VERIFIED' ? 'Connect NAVER' : 'Reconnect'}
              </Button>
            )}
          </div>

          <Dialog
            open={confirming}
            onClose={() => setConfirming(false)}
            title="Disconnect NAVER?"
            description="Your age and gender will no longer be verified."
            initialFocusRef={cancelRef}
            returnFocusRef={disconnectRef}
          >
            <p>
              Your own introductions keep the audience you chose, and nothing you published is
              deleted. You will no longer be able to view introductions that require a verified
              age or gender until you verify again.
            </p>
            <div className="identity-actions">
              <Button ref={cancelRef} variant="secondary" onClick={() => setConfirming(false)}>
                Keep connected
              </Button>
              <Button
                variant="danger"
                busy={state.busy}
                onClick={() => {
                  setConfirming(false)
                  void controller.disconnect()
                }}
              >
                Disconnect NAVER
              </Button>
            </div>
          </Dialog>
        </>
      )}
    </section>
  )
}
