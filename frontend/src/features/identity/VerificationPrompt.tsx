import { useId } from 'react'
import { Link } from 'react-router-dom'
import { Button } from '../../shared/ui'
import './identity.css'

/**
 * The gated-post prompt.
 *
 * A restricted introduction the viewer cannot see answers `404
 * RESOURCE_UNAVAILABLE`, exactly like a deleted one. The server does not
 * distinguish the two, and this component must not either: it shows the same
 * neutral panel for both, and it never guesses. What it *can* say is what the
 * user can do about it, which is verify their age and gender.
 *
 * The panel renders nothing about the post: no author, no body, no attachments,
 * no audience. There is no prop for any of them, so a future caller cannot
 * accidentally pass one in.
 */
export function VerificationPrompt({
  /** Whether the viewer already holds a live claim. */
  verified,
  /** Where the verification flow should return to. */
  returnPath = '/me/identity',
  /** The path to try again, when the caller knows it. */
  retryHref,
}: {
  verified: boolean
  returnPath?: '/me/identity' | '/me/settings' | '/me/profile'
  retryHref?: string
}) {
  const id = useId()
  return (
    <section className="identity-surface identity-prompt" aria-labelledby={`${id}-title`}>
      <h1 id={`${id}-title`}>This introduction is not available to you</h1>
      <p>
        The author shared it with a specific audience. It may be restricted by age or gender, or
        it may no longer exist.
      </p>
      {verified ? (
        <p>
          Your age and gender are verified, so this introduction is not one your verification
          covers. Nothing else about it can be shown.
        </p>
      ) : (
        <>
          <p>
            If you have not verified your age and gender with NAVER, you can do so. Verification
            is optional and is not required for the rest of the product.
          </p>
          <div className="identity-actions">
            <Link to={returnPath}>Verify with NAVER</Link>
          </div>
        </>
      )}
      {retryHref && (
        <div className="identity-actions">
          <Button variant="secondary" onClick={() => window.location.assign(retryHref)}>
            Try again
          </Button>
        </div>
      )}
      <p className="identity-prompt__note">
        Nothing about this introduction is shown, because the server does not disclose whether it
        was restricted or removed.
      </p>
    </section>
  )
}
