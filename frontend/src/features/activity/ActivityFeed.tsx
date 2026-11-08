import { Link } from 'react-router-dom'
import { activityLabel, activityTarget, type ActivityItem } from './contract'
import { sharedInterestLine } from '../intros/contract'
import './activity.css'

/**
 * WP-05 T-415: the activity feed.
 *
 * Four kinds of entry, each one tap from its target. There is **no approval
 * affordance anywhere**: no accept button, no decline button, no "pending"
 * badge, no unanswered-request counter and no acceptance deadline. A heart and a
 * follow are statements, not requests, so there is nothing to answer.
 *
 * A like entry opens the liker's profile directly.
 */
export interface ActivityFeedProps {
  readonly items: readonly ActivityItem[]
  readonly nextCursor: string | null
  readonly loading: boolean
  readonly onMore: () => void
}

export function ActivityFeed({ items, nextCursor, loading, onMore }: ActivityFeedProps) {
  return (
    <section className="activity-feed" aria-labelledby="activity-title">
      <header>
        <p>ACTIVITY</p>
        <h1 id="activity-title">Recent activity</h1>
      </header>
      {items.length === 0 ? (
        <p role="status">Nothing yet.</p>
      ) : (
        <ul>
          {items.map((item) => {
            const shared = sharedInterestLine(item.actor.shared_with_author)
            return (
              <li key={item.activity_id} className={item.read ? 'activity-item' : 'activity-item activity-item-unread'}>
                <Link to={activityTarget(item)} className="activity-link">
                  <span className="activity-avatar" aria-hidden="true">
                    {item.actor.nickname.slice(0, 1).toUpperCase()}
                  </span>
                  <span className="activity-copy">
                    <span>{activityLabel(item)}</span>
                    {shared !== null && <span className="activity-shared">{shared}</span>}
                    {item.actor.interaction_style !== null && item.actor.interaction_style !== '' && (
                      <span className="activity-style">{item.actor.interaction_style}</span>
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
          More activity
        </button>
      )}
    </section>
  )
}
