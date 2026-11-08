import { Link } from 'react-router-dom'
import { sharedInterestLine, type LikerProfile } from './contract'

/**
 * WP-05 T-412: the liker list panel.
 *
 * Author-only, paginated, and every row is one tap to the profile. A row shows
 * the profile image or the emo character, the nickname, the shared interests and
 * the one-line interaction style.
 *
 * **No acceptance screen and no room screen in the path.** The row links straight
 * to the profile; there is no intermediate step, no approve button and no
 * "accept" affordance anywhere. A heart is not a request, so there is nothing to
 * accept.
 *
 * The shared-interest line is omitted when there is no overlap, exactly as on a
 * discovery card.
 */
export interface IntroLikersPanelProps {
  readonly likers: readonly LikerProfile[]
  readonly nextCursor: string | null
  readonly loading: boolean
  readonly onMore: () => void
}

export function IntroLikersPanel({ likers, nextCursor, loading, onMore }: IntroLikersPanelProps) {
  return (
    <section className="intro-likers" aria-label="People who left a heart">
      <h2>People who left a heart</h2>
      {likers.length === 0 ? (
        <p>No hearts yet.</p>
      ) : (
        <ul>
          {likers.map((liker) => {
            const shared = sharedInterestLine(liker.shared_with_author)
            return (
              <li key={liker.profile_id}>
                <Link to={`/profiles/${liker.profile_id}`} className="intro-liker-row">
                  <span className="intro-card-avatar" aria-hidden="true">
                    {liker.nickname.slice(0, 1).toUpperCase()}
                  </span>
                  <span className="intro-liker-copy">
                    <strong>{liker.nickname}</strong>
                    {shared !== null && <span className="intro-liker-shared">{shared}</span>}
                    {liker.interaction_style !== null && liker.interaction_style !== '' && (
                      <span className="intro-liker-style">{liker.interaction_style}</span>
                    )}
                  </span>
                </Link>
              </li>
            )
          })}
        </ul>
      )}
      {nextCursor !== null && (
        <button type="button" disabled={loading} onClick={onMore}>
          More people
        </button>
      )}
    </section>
  )
}
