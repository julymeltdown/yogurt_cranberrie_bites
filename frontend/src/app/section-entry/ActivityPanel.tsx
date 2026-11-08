/**
 * WP-09 T-901: the Activity section.
 *
 * Activity is the second primary section: likes received, comments, replies and
 * follow notifications (product-plan.md §7). The notification list itself is
 * owned by `features/inbox`; this panel binds the shell's session to it and
 * gives the section its heading and its empty state.
 *
 * What it must not do: generate a notification for a view. Merely opening this
 * section notifies nobody (product-plan.md §2 hard rule 6).
 */
import { useId } from 'react'
import { ShellLink } from '../navigation'
import './section-entry.css'

/** The props of the activity panel. */
export interface ActivityPanelProps {
  /** The notification list, supplied by the shell or a test. */
  readonly children?: React.ReactNode
}

/**
 * `/activity` — the section home.
 *
 * The inbox is a separate screen (`/notifications`); this section is the summary
 * entry point plus the same list, so a user who taps Activity sees their
 * notifications without a second tap.
 */
export function ActivityPanel({ children }: ActivityPanelProps = {}) {
  const id = useId()
  return <section className="section-page" aria-labelledby={`${id}-title`}>
    <header className="section-header">
      <h1 id={`${id}-title`}>Activity</h1>
      <p>Likes, comments, replies and follows.</p>
    </header>
    {children ?? <section className="section-block" aria-labelledby={`${id}-inbox`}>
      <h2 id={`${id}-inbox`}>Notifications</h2>
      <p role="status">Open your inbox to see likes, comments, replies and follows.</p>
      <ShellLink to="/notifications">Open inbox</ShellLink>
    </section>}
  </section>
}
