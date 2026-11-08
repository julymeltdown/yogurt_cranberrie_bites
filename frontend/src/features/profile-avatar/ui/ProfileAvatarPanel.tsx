import { useEffect, useMemo, useState } from 'react'
import type { PrivacyScope } from '../../../shared/query/privacy'
import type { ProfileAvatarEditorOwner } from '../model/owner'
import { ProfileAvatarEditor } from './ProfileAvatarEditor'

/**
 * WP-08 T-704/T-705: the profile screen's avatar panel.
 *
 * This replaces the concert `AccountPanel` avatar branch, which selected
 * `MotionEditor` only when an event-scoped `motionAvatar` owner existed and
 * otherwise fell back to a different editor. There is no fallback editor and no
 * event gate here:
 *
 * * with an owner for the *current* account, the profile editor renders;
 * * without one, an explicit unavailable state renders — never another
 *   account's character and never a substitute character.
 *
 * The panel re-checks the owner against the scope on every render, so an account
 * switch that has not yet re-mounted the panel cannot display the previous
 * account's editor.
 */
export function ProfileAvatarPanel({ owner, scope }: {
  readonly owner: ProfileAvatarEditorOwner | null
  readonly scope: PrivacyScope | null
}) {
  const [version, setVersion] = useState(0)
  useEffect(() => {
    // A scope change is an account change: force a re-evaluation of the owner.
    setVersion(value => value + 1)
  }, [scope?.userId, scope?.generation, scope?.policyGeneration])
  const usable = useMemo(() => {
    if (!owner || !scope) return false
    if (!owner.isCurrent()) return false
    return owner.scope.userId === scope.userId && owner.scope.generation === scope.generation
      && owner.scope.policyGeneration === scope.policyGeneration
    // `version` deliberately participates: it is the account-change trigger.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [owner, scope, version])
  if (!usable || !owner) {
    return <section className="mcr-profile-avatar" aria-label="Character editor">
      <header className="mcr-profile-avatar__header">
        <h1>Your character</h1>
        <p>This is the character shown on your profile.</p>
      </header>
      <p role="status">Your character is unavailable until your account is verified.</p>
    </section>
  }
  return <ProfileAvatarEditor controller={owner.controller} />
}
