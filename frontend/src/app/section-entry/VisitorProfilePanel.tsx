/**
 * WP-09 T-903: the visitor profile screen.
 *
 * A visitor sees the introduction, the shared interests, the photos and artwork,
 * the room entry and a follow action. The presentation is
 * `features/profile/PublicProfilePage`; this panel supplies the profile id from
 * the URL and, when the shell has one, the follow action.
 *
 * Two rules are absolute and are asserted by `__tests__/PublicProfilePage.test.tsx`:
 *
 * 1. **No numeric age, ever.** The view model has no age field and the decoder
 *    refuses a payload that carries one.
 * 2. **Shared interests are omitted when empty.** No empty heading and no filler.
 *
 * The page shows no owner-only control: no edit, no delete, no settings link.
 */
import { useId } from 'react'
import { useParams } from 'react-router-dom'
import { ShellLink } from '../navigation'
import { profilePath } from '../../features/discovery'
import { PublicProfilePage, type VisitorProfileView } from '../../features/profile/PublicProfilePage'
import './section-entry.css'

export interface VisitorProfilePanelProps {
  /**
   * The profile to show. The shell supplies it from the server read; a test
   * supplies it directly. `null` is the unavailable state.
   */
  readonly profile?: VisitorProfileView | null
  /** Follow or unfollow. Omitted means the action is unavailable. */
  readonly onFollow?: (() => void) | undefined
}

/**
 * `/discover/people/:profileId` — a visitor profile.
 *
 * The route parameter is the profile id. The page itself renders from the view
 * model, so this panel never invents profile data: without a loaded profile it
 * renders the unavailable state, which is also what a visitor sees for a profile
 * they may not view.
 */
export function VisitorProfilePanel({ profile = null, onFollow }: VisitorProfilePanelProps = {}) {
  const params = useParams()
  const id = useId()
  const profileId = profile?.profileId ?? params.profileId ?? ''
  // A profile whose id does not match the URL is not this route's profile: it is
  // a stale or forged value, and rendering it would show the wrong person.
  const matches = profile !== null && profile.profileId === profileId
  return <div className="section-page">
    <PublicProfilePage view={matches ? profile : null} {...(onFollow ? { onFollow } : {})} />
    {!matches && profileId !== '' && <ShellLink to={profilePath(profileId)}>Open this profile</ShellLink>}
    <span hidden id={`${id}-route`}>{profileId}</span>
  </div>
}

/** The visitor profile path, so callers keep one spelling. */
export { profilePath }
