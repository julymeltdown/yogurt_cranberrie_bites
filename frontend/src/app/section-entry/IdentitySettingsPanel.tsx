/**
 * WP-09 T-906: the identity settings screen.
 *
 * `/me/identity` is where the owner connects, inspects and disconnects their
 * NAVER verification (charter decision D-09). It composes WP-04b's
 * `IdentityStatus`, which already owns the read, the start, the disconnect and
 * the confirmation dialog; this panel binds the shell's session and transport to
 * it and places it inside My Profile.
 *
 * Two sentences the screen must state, and does:
 *
 * * **Purpose** — what verification is for: seeing and being matched by
 *   introductions that carry an age or gender restriction.
 * * **Non-requirement** — that everything else works without it, so a user who
 *   never verifies is not left wondering whether they are locked out.
 *
 * This is also the only screen in the product that shows the owner's derived age,
 * and it is owner-only: the value comes from `getOwnIdentityVerification`, which
 * the server answers only for the owner.
 */
import { useId, useMemo } from 'react'
import { usePorts } from '../access'
import { ShellLink } from '../navigation'
import { IdentitySettings, createIdentityController } from '../../features/identity'
import './section-entry.css'

/** `/me/identity` — identity verification settings. */
export function IdentitySettingsPanel() {
  const ports = usePorts()
  const auth = ports.auth
  const id = useId()
  const controller = useMemo(() => {
    if (!auth) return null
    return createIdentityController({
      request: auth.request,
      scope: () => auth.session.read(),
      privacy: auth.privacy,
      // A start navigates the browser to the provider. In a test the navigation
      // is injected; here it is the real location change.
      navigate: url => { window.location.assign(url) },
    })
  }, [auth])

  if (!controller) {
    return <section className="section-page" aria-labelledby={`${id}-title`}>
      <header className="section-header"><h1 id={`${id}-title`}>Identity verification</h1></header>
      <p role="status">Verification settings are unavailable until your session is verified.</p>
      <ShellLink to="/me">Back to your profile</ShellLink>
    </section>
  }
  return <div className="section-page">
    <IdentitySettings controller={controller} />
    <ShellLink to="/me">Back to your profile</ShellLink>
  </div>
}
