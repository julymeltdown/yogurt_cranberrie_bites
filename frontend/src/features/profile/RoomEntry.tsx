/**
 * WP-07 T-610: the profile's Room entry.
 *
 * The room is a **subordinate screen**: the profile shows one ordinary entry that
 * opens it, and nothing else. The room is never embedded in the profile header and
 * never embedded in an intro post — there is no room preview, thumbnail or inline
 * scene here, only a link.
 *
 * The entry is the only way into the room. A profile that reports
 * `room_available: false` (the room is private, or the profile has none) shows the
 * entry as unavailable rather than a link that would 404.
 */
import { Link } from 'react-router-dom'

export interface RoomEntryProps {
  /** The profile whose room the entry opens. */
  readonly profileId: string
  /** The profile's `room_available`, as the server reported it. */
  readonly available: boolean
  /** Whether the viewer is the profile owner. */
  readonly isOwner?: boolean | undefined
}

/** The route of a profile's room. A subordinate screen, not a panel of the profile. */
export const roomPath = (profileId: string) => `/people/${profileId}/room`

export function RoomEntry({ profileId, available, isOwner = false }: RoomEntryProps) {
  return <section className="room-entry" aria-label="Personal room">
    <div>
      <h2>Room</h2>
      <p>{available
        ? (isOwner ? 'Your personal room. Open it to arrange furniture and posters.' : 'Open this person\'s personal room.')
        : 'This room is not available.'}</p>
    </div>
    {available
      ? <Link to={roomPath(profileId)}>Open room</Link>
      : <span role="status">Unavailable</span>}
  </section>
}
