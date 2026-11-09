/**
 * WP-09 T-903: the visitor profile page.
 *
 * A visitor sees the introduction, the shared interests, the photos and artwork,
 * the room entry and a follow action. Two rules are absolute:
 *
 * 1. **No numeric age, ever.** The view model has no age field, `decodeVisitorProfile`
 *    refuses a payload that carries one, and `admitGenderBucket` drops anything
 *    that is not one of the three closed buckets. A numeric age therefore has no
 *    path from a response to the DOM.
 * 2. **Shared interests are omitted when empty.** No heading, no filler, no
 *    "you might get along" claim the page cannot support.
 *
 * The page is presentational: it renders an already-authorized view model and
 * never fetches or decides visibility.
 */
import { useId } from 'react'
import { Link } from 'react-router-dom'
import { z } from 'zod'
import { RoomEntry } from './RoomEntry'
import { profilePath } from '../discovery/contract'
import './profile.css'

/** The closed gender buckets a profile may show. */
export const GENDER_BUCKETS = ['female', 'male', 'unknown'] as const
export type GenderBucket = typeof GENDER_BUCKETS[number]

/**
 * What a visitor profile may show.
 *
 * Note what is absent: there is no `age` field. The type makes "render the age"
 * impossible rather than merely discouraged.
 */
export interface VisitorProfileView {
  readonly profileId: string
  readonly nickname: string
  readonly bio: string | null
  /** `null` when the owner did not opt in to showing it (product decision P-10). */
  readonly genderBucket: GenderBucket | null
  /** Empty when there is no overlap. Never a placeholder. */
  readonly sharedInterests: readonly string[]
  readonly following: boolean
  readonly roomAvailable: boolean
}

/**
 * The wire shape of a visitor profile, as a closed schema.
 *
 * `additionalProperties: false` is the point: a payload that carries an `age`
 * field is refused, so a server bug or a stale client cannot smuggle one in.
 */
const visitorProfileSchema = z.strictObject({
  profile_id: z.string().regex(/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/),
  nickname: z.string().min(1).max(40),
  bio: z.string().max(160).nullable(),
  gender_bucket: z.enum(GENDER_BUCKETS).nullable(),
  shared_items: z.array(z.object({ label: z.string().min(1).max(80) })).max(15),
  room_available: z.boolean(),
})

/**
 * Decode a visitor profile response.
 *
 * A payload with an unexpected field — including an age — throws rather than
 * being silently narrowed, so the caller sees a contract violation instead of a
 * page that quietly dropped a value.
 */
export function decodeVisitorProfile(value: unknown): VisitorProfileView {
  const row = visitorProfileSchema.parse(value)
  return Object.freeze({
    profileId: row.profile_id,
    nickname: row.nickname,
    bio: row.bio,
    genderBucket: admitGenderBucket(row.gender_bucket),
    sharedInterests: Object.freeze(row.shared_items.map(item => item.label)),
    following: false,
    roomAvailable: row.room_available,
  })
}

/**
 * Whether a value may be rendered as a gender bucket.
 *
 * The server decides *whether* to send it (owner opt-in). This is the client's
 * second gate: an unexpected value is dropped rather than displayed.
 */
export function admitGenderBucket(value: unknown): GenderBucket | null {
  return value === 'female' || value === 'male' || value === 'unknown' ? value : null
}

const GENDER_LABEL: Readonly<Record<GenderBucket, string>> = Object.freeze({
  female: 'Female',
  male: 'Male',
  unknown: 'Not specified',
})

/**
 * The shared-interest line, or `null` when there is nothing to claim.
 *
 * An empty list is the absence of a claim, not a claim of no overlap, so the page
 * renders nothing at all for it.
 */
export function sharedInterestLine(interests: readonly string[]): string | null {
  const names = interests.map(name => name.trim()).filter(name => name.length > 0)
  if (names.length === 0) return null
  const shown = names.slice(0, 3)
  const rest = names.length - shown.length
  const list = shown.length === 1 ? shown[0]!
    : shown.length === 2 ? `${shown[0]} and ${shown[1]}`
      : `${shown.slice(0, -1).join(', ')} and ${shown[shown.length - 1]}`
  return rest > 0 ? `You both like ${list} and ${rest} more.` : `You both like ${list}.`
}

export interface PublicProfilePageProps {
  /** `null` is the unavailable state: nothing about the profile is known. */
  readonly view: VisitorProfileView | null
  /** Follow or unfollow. Omitted means the action is unavailable. */
  readonly onFollow?: (() => void) | undefined
  /** The visitor's photos, already authorized and loaded by the shell. */
  readonly posts?: React.ReactNode
}

/** The visitor profile page. */
export function PublicProfilePage({ view, onFollow, posts }: PublicProfilePageProps) {
  const id = useId()
  if (view === null) {
    return <section className="profile-page" aria-labelledby={`${id}-title`}>
      <header className="profile-page__header"><h1 id={`${id}-title`}>Profile</h1></header>
      <p role="status">This profile is unavailable.</p>
      <Link to="/discover/people">Back to people</Link>
    </section>
  }
  const shared = sharedInterestLine(view.sharedInterests)
  return <section className="profile-page" aria-labelledby={`${id}-title`}>
    <header className="profile-page__header">
      <h1 id={`${id}-title`}>Profile</h1>
      {/* The visitor card leads with the same square frame as the owner card.
          It is decorative: the nickname beside it is the accessible name, and
          no substitute character art is ever drawn in the frame. */}
      <div className="profile-page__portrait" aria-hidden="true">
        {view.nickname.slice(0, 1).toUpperCase()}
      </div>
      <p className="profile-page__name">{view.nickname}</p>
      <p className="profile-page__bio">{view.bio === null || view.bio === ''
        ? 'No bio shared.'
        : view.bio}</p>
      <nav className="profile-page__actions" aria-label="Profile actions">
        <button type="button" disabled={!onFollow} aria-pressed={view.following}
          onClick={() => onFollow?.()}>
          {view.following ? 'Following' : 'Follow'}
        </button>
        <Link to={`/report?targetType=USER&targetId=${view.profileId}`}>Report</Link>
      </nav>
    </header>

    {/* Omitted entirely when there is no overlap: no heading, no filler. */}
    {shared !== null && <section className="profile-page__block" aria-labelledby={`${id}-shared`}>
      <h2 id={`${id}-shared`}>Shared interests</h2>
      <p className="profile-page__shared">{shared}</p>
    </section>}

    {/* The gender bucket appears only when the owner opted in. There is no age
        field in the view model, so no age can be rendered. */}
    {view.genderBucket !== null && <section className="profile-page__block" aria-labelledby={`${id}-gender`}>
      <h2 id={`${id}-gender`}>Gender</h2>
      <p>{GENDER_LABEL[view.genderBucket]}</p>
    </section>}

    <section className="profile-page__block" aria-labelledby={`${id}-posts`}>
      <h2 id={`${id}-posts`}>Photos and artwork</h2>
      {posts ?? <p role="status">Photos and artwork this person shared publicly appear here.</p>}
    </section>

    {/* The room is a subordinate screen: one entry, never an inline preview. */}
    <RoomEntry profileId={view.profileId} available={view.roomAvailable} />

    <Link to="/discover/people">Back to people</Link>
  </section>
}

/** The visitor profile path, so callers keep one spelling. */
export { profilePath }
