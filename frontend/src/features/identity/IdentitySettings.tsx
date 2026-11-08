/**
 * WP-09 T-906: the identity settings screen.
 *
 * `/me/identity` is where the owner connects, inspects and disconnects their
 * NAVER verification (charter decision D-09). It composes WP-04b's
 * `IdentityStatus`, which already owns the read, the start, the disconnect and
 * the confirmation dialog; this component places it inside My Profile and states
 * the two facts a user needs before deciding:
 *
 * * **Purpose** — what verification is for: seeing and being matched by
 *   introductions that carry an age or gender restriction.
 * * **Non-requirement** — that everything else works without it, so a user who
 *   never verifies is not left wondering whether they are locked out.
 *
 * This is the only screen in the product that shows the owner's derived age, and
 * it is owner-only: the value comes from `getOwnIdentityVerification`, which the
 * server answers only for the owner.
 */
import { useEffect, useId } from 'react'
import { IdentityStatus, type IdentityController } from './index'
import './identity.css'

export interface IdentitySettingsProps {
  readonly controller: IdentityController
}

/** The identity settings screen. */
export function IdentitySettings({ controller }: IdentitySettingsProps) {
  const id = useId()
  useEffect(() => () => { controller.dispose() }, [controller])
  return <section className="identity-settings" aria-labelledby={`${id}-title`}>
    <header className="identity-settings__header">
      <h1 id={`${id}-title`}>Identity verification</h1>
      <p>
        NAVER verification confirms your age and gender so you can see, and be matched by,
        introductions that carry an age or gender restriction.
      </p>
      <p>
        It is not required for the rest of the product: your profile, photos, room, favorites and
        unrestricted introductions all work without it.
      </p>
    </header>
    <IdentityStatus controller={controller} />
  </section>
}
