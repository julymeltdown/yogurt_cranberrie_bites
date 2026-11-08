/**
 * WP-09 T-902: the profile page.
 *
 * One component, two variants, chosen by `isOwner`:
 *
 * * **owner** — introduction, favorites, the emo character, the photo/artwork
 *   posts, the room entry, and the owner-only settings links;
 * * **visitor** — the introduction, the shared interests, the photos, the room
 *   entry and a follow action. No owner-only control appears.
 *
 * The page is presentational: it receives an already-authorized view model and
 * renders it. It never fetches, never decides visibility and never decides who
 * the viewer is. That is what makes the two variants testable in isolation and
 * what keeps the shell's authority where it belongs.
 *
 * Two rules are structural:
 *
 * 1. **No room preview inline.** The page renders one `RoomEntry` link and no
 *    room scene, thumbnail, poster or layout. The room opens from its entry.
 * 2. **No age.** There is no age field in the view model, so the page cannot
 *    render one. The owner's verified age belongs to the identity settings
 *    screen.
 */
import { useId } from 'react'
import { Link } from 'react-router-dom'
import { RoomEntry } from './RoomEntry'
import { profilePath } from '../discovery/contract'
import './profile.css'

/** One favorite, as the page shows it. */
export interface ProfilePageFavorite {
  readonly category: 'MUSIC' | 'ANIME' | 'GAME'
  readonly label: string
}

/**
 * What the profile page may show.
 *
 * Note what is absent: there is no `age` field. The type makes "render the age"
 * impossible rather than merely discouraged.
 */
export interface ProfilePageView {
  readonly profileId: string
  readonly nickname: string
  readonly bio: string | null
  /** Empty means the section is omitted, not rendered as "none". */
  readonly favorites: readonly ProfilePageFavorite[]
  /** `null` when the owner did not opt in to showing it (product decision P-10). */
  readonly genderBucket?: 'female' | 'male' | 'unknown' | null
  /** Empty means the shared-interest line is omitted entirely. */
  readonly sharedInterests?: readonly string[]
  readonly following?: boolean
  readonly roomAvailable: boolean
}

export interface ProfilePageProps {
  /** `null` is the unavailable state: nothing about the profile is known. */
  readonly view: ProfilePageView | null
  /** Whether the viewer owns this profile. */
  readonly isOwner: boolean
  /** Follow or unfollow, for the visitor variant. Omitted means unavailable. */
  readonly onFollow?: (() => void) | undefined
  /** The owner's posts, already authorized and loaded by the shell. */
  readonly posts?: React.ReactNode
  /** The owner's character editor, already authorized by the shell. */
  readonly character?: React.ReactNode
}

const CATEGORY_LABEL: Readonly<Record<ProfilePageFavorite['category'], string>> = Object.freeze({
  MUSIC: 'Music',
  ANIME: 'Anime',
  GAME: 'Games',
})

const GENDER_LABEL: Readonly<Record<NonNullable<ProfilePageView['genderBucket']>, string>> = Object.freeze({
  female: 'Female',
  male: 'Male',
  unknown: 'Not specified',
})

/**
 * The shared-interest line, or `null` when there is nothing to claim.
 *
 * An empty list is the absence of a claim, not a claim of no overlap, so the
 * page renders nothing at all for it.
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

/** The profile page. */
export function ProfilePage({ view, isOwner, onFollow, posts, character }: ProfilePageProps) {
  const id = useId()
  const title = isOwner ? 'My Profile' : 'Profile'

  if (view === null) {
    return <section className="profile-page" aria-labelledby={`${id}-title`}>
      <header className="profile-page__header"><h1 id={`${id}-title`}>{title}</h1></header>
      <p role="status">This profile is unavailable.</p>
    </section>
  }

  const shared = sharedInterestLine(view.sharedInterests ?? [])
  const gender = view.genderBucket ?? null

  return <section className="profile-page" aria-labelledby={`${id}-title`}>
    <header className="profile-page__header">
      <h1 id={`${id}-title`}>{title}</h1>
      <p className="profile-page__name">{view.nickname}</p>
      <p className="profile-page__bio">{view.bio === null || view.bio === ''
        ? 'No bio yet.'
        : view.bio}</p>
      <nav className="profile-page__actions" aria-label={isOwner ? 'Profile settings' : 'Profile actions'}>
        {isOwner
          ? <>
            <Link to="/me/profile">Edit profile</Link>
            <Link to="/me/avatar">Customize character</Link>
            <Link to="/me/identity">Identity verification</Link>
            <Link to="/me/settings">Settings</Link>
          </>
          : <>
            <button type="button" disabled={!onFollow} aria-pressed={view.following ?? false}
              onClick={() => onFollow?.()}>
              {view.following ? 'Following' : 'Follow'}
            </button>
            <Link to={`/report?targetType=USER&targetId=${view.profileId}`}>Report</Link>
          </>}
      </nav>
    </header>

    {/* Omitted entirely when empty: no heading, no filler. */}
    {view.favorites.length > 0 && <section className="profile-page__block" aria-labelledby={`${id}-favorites`}>
      <h2 id={`${id}-favorites`}>Favorites</h2>
      {(['MUSIC', 'ANIME', 'GAME'] as const).map(category => {
        const picks = view.favorites.filter(favorite => favorite.category === category)
        if (picks.length === 0) return null
        return <div key={category}>
          <h3>{CATEGORY_LABEL[category]}</h3>
          <ul>{picks.map(favorite => <li key={favorite.label}>{favorite.label}</li>)}</ul>
        </div>
      })}
    </section>}

    {/* Omitted entirely when there is no overlap. */}
    {shared !== null && <section className="profile-page__block" aria-labelledby={`${id}-shared`}>
      <h2 id={`${id}-shared`}>Shared interests</h2>
      <p>{shared}</p>
    </section>}

    {/* The gender bucket appears only when the owner opted in. There is no age
        field in the view model, so no age can be rendered. */}
    {gender !== null && <section className="profile-page__block" aria-labelledby={`${id}-gender`}>
      <h2 id={`${id}-gender`}>Gender</h2>
      <p>{GENDER_LABEL[gender]}</p>
    </section>}

    {isOwner && <section className="profile-page__block" aria-labelledby={`${id}-character`}>
      <h2 id={`${id}-character`}>Character</h2>
      {character ?? <p role="status">Your character is not loaded yet.</p>}
    </section>}

    <section className="profile-page__block" aria-labelledby={`${id}-posts`}>
      <h2 id={`${id}-posts`}>Photos and artwork</h2>
      {isOwner && <Link to="/me/posts/new">Post a photo or artwork</Link>}
      {posts ?? <p role="status">{isOwner
        ? 'You have not posted any photos yet.'
        : 'Photos and artwork this person shared publicly appear here.'}</p>}
    </section>

    {/* The room is a subordinate screen: one entry, never an inline preview. */}
    <RoomEntry profileId={view.profileId} available={view.roomAvailable} isOwner={isOwner} />

    {!isOwner && <Link to="/discover/people">Back to people</Link>}
  </section>
}

/** The visitor profile path, so callers keep one spelling. */
export { profilePath }
