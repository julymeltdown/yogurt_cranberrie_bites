/**
 * WP-09 T-902: the My Profile section home — the owner's profile page.
 *
 * The page composes what the product plan puts in the section: the introduction,
 * the favorites, the emo character, the photo and artwork posts, and the room
 * entry. The static presentation is `features/profile/ProfilePage`; this panel
 * supplies the live parts (the character editor and the posts gallery) from the
 * account owner the shell already created.
 *
 * Two rules are structural rather than stylistic:
 *
 * 1. **No room preview inline.** The room is a subordinate screen reached from
 *    its entry (WP-07). The page renders one link and no room scene, thumbnail or
 *    data.
 * 2. **No age.** The owner's verified age belongs to the identity settings screen
 *    (T-906), which is the only place it appears.
 */
import { useEffect, useId, useMemo, useSyncExternalStore } from 'react'
import { usePorts } from '../access'
import { useAccountOwner } from '../account-entry/provider'
import { ProfilePage, type ProfilePageFavorite } from '../../features/profile/ProfilePage'
import { ProfileAvatarPanel } from '../../features/profile-avatar'
import {
  ProfileMediaGallery,
  createGalleryReader,
  createGalleryStore,
} from '../../features/profile-media'
import {
  CATEGORIES,
  itemsFor,
  type FavoriteSelection,
} from '../../features/favorites'
import './section-entry.css'

/**
 * The owner's profile page.
 *
 * It reads the account owner the shell already created. When there is no owner —
 * the session is not verified yet — it says so rather than rendering another
 * account's data.
 */
export function MyProfilePanel() {
  const owner = useAccountOwner()
  const ports = usePorts()
  const id = useId()
  const auth = ports.auth
  const me = owner?.me ?? null
  const meState = useSyncExternalStore(
    me?.subscribe ?? (() => () => {}),
    me?.read ?? (() => null),
    me?.read ?? (() => null),
  )
  useEffect(() => {
    if (me && me.read().phase === 'idle') void me.load()
  }, [me])

  const galleryStore = useMemo(() => {
    if (!auth || !owner) return null
    const scope = auth.privacy.capture()
    if (!auth.privacy.isCurrent(scope) || scope.userId === null) return null
    return createGalleryStore(createGalleryReader({
      auth: { privacy: auth.privacy, request: auth.request },
      scope,
      profileId: scope.userId,
    }))
  }, [auth, owner])
  useEffect(() => () => { galleryStore?.dispose() }, [galleryStore])

  if (!owner) {
    return <section className="section-page" aria-labelledby={`${id}-title`}>
      <header className="section-header"><h1 id={`${id}-title`}>My Profile</h1></header>
      <p role="status">Your profile is unavailable until your session is verified.</p>
    </section>
  }

  const participant = meState?.participant ?? null
  const view = {
    profileId: owner.scope.userId ?? '',
    nickname: participant?.profile.display_name ?? '',
    bio: participant?.profile.bio ?? null,
    favorites: favoritesOf(participant),
    roomAvailable: true,
  }

  return <ProfilePage view={participant ? view : null} isOwner
    character={<ProfileAvatarPanel owner={owner.profileAvatar} scope={owner.scope} />}
    posts={galleryStore
      ? <ProfileMediaGallery store={galleryStore} isAuthor />
      : <p role="status">Your posts are unavailable for this session.</p>} />
}

/**
 * The favorites a participant carries, as the page shows them.
 *
 * The concert participant projection does not carry favorites; the profile
 * service owns them (`core.OwnProfile.favorites`). This reads them when the
 * projection has them and returns an empty list otherwise, so the section is
 * omitted rather than claiming the user has none.
 */
function favoritesOf(participant: object | null): readonly ProfilePageFavorite[] {
  if (!participant) return []
  const value = (participant as { favorites?: unknown }).favorites
  if (value === null || typeof value !== 'object') return []
  const selection = value as FavoriteSelection
  return CATEGORIES.flatMap(category =>
    itemsFor(selection, category).map(item => ({
      category: category === 'MUSIC' ? 'MUSIC' as const : category === 'ANIME' ? 'ANIME' as const : 'GAME' as const,
      label: item.label,
    })))
}
