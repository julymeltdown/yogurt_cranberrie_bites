import { useEffect, useSyncExternalStore, type ReactNode } from 'react'
import { Link } from 'react-router-dom'
import type { PublicProfile as PublicProfileData } from '../people/contract'
import type { PeopleController } from '../people/controller'
import { ReviewedProfileAvatar, type ProfileAvatarOptions } from './Avatar'
import { ConcertWindow } from '../../shared/ui/concert-window'
import { ProfileActivity, type ActivityPrivacy, type ProfileActivityInput } from '../me/ui/reference-profile/ProfileActivity'
import { ProfileIcon } from './reference-presentation/ProfileIcon'
import './profile.css'
export function PublicProfile({ controller, userId, avatars, sharing, blocking, activity, activityPrivacy, renderAvatar }: {
  controller: PeopleController; userId: string; avatars?: ProfileAvatarOptions | undefined;
  sharing?: ((userId: string) => void) | undefined; blocking?: ((userId: string) => void) | undefined;
  activity?: ProfileActivityInput | undefined; activityPrivacy?: ActivityPrivacy | undefined; renderAvatar?: ((avatar: PublicProfileData['avatar'], label: string) => ReactNode) | undefined
}) {
  const state = useSyncExternalStore(controller.subscribe, controller.read, controller.read)
  useEffect(() => { controller.start(); if (state.available) void controller.openProfile(userId); return () => controller.closeProfile() }, [controller, userId, state.available])
  const profile = state.available && state.target === userId && state.profile.phase === 'ready' ? state.profile.data : null
  return <section className="reference-public-profile"><ConcertWindow title="Profile" headingLevel={1} icon="me" footer={<Link to="/meet/people">Back to people</Link>}>
    {profile ? <>
      <div className="public-profile-header"><div className="public-profile-identity">
        {renderAvatar ? <div className="intro-avatar">{renderAvatar(profile.avatar, `${profile.display_name}'s avatar`)}</div> : profile.avatar.config.contract_version === '1.0.0' ? <ReviewedProfileAvatar config={profile.avatar.config} label={`${profile.display_name}'s avatar`} options={avatars} /> : <div className="intro-avatar" role="img" aria-label={`${profile.display_name}'s avatar: selected character portrait unavailable`}>Selected character portrait unavailable in this connection.</div>}
        <div className="public-profile-copy"><h2>{profile.display_name}</h2><p className="public-profile-bio">{profile.bio ?? 'No bio shared.'}</p></div>
      </div><div className="public-profile-actions" aria-label="Profile actions">
        <button type="button" disabled={!sharing} onClick={() => sharing?.(userId)} aria-label="Request sharing" title="Request sharing"><ProfileIcon name="link" /></button>
        <Link to={`/report?targetType=USER&targetId=${userId}`} aria-label="Report profile" title="Report profile"><ProfileIcon name="report" /></Link>
        <button type="button" disabled={!blocking} onClick={() => blocking?.(userId)} aria-label="Block user" title="Block user"><ProfileIcon name="block" /></button>
      </div></div>
      <nav className="public-profile-navigation" aria-label="Profile activity"><span aria-current="page">Activity</span><button type="button" disabled title="Private party activity is unavailable">Parties</button><button type="button" disabled title="Private gift activity is unavailable">Gifts</button></nav>
      <ProfileActivity subject={profile.user_id} input={activity} privacy={activityPrivacy} />
      <div className="public-profile-privacy"><p>Social handles and verified age are private. Sharing requires mutual consent.</p>{(!sharing || !blocking) && <p>Some account actions are currently unavailable.</p>}<Link to="/me/blocks">Manage blocks</Link></div>
    </> : <div role="status" className="public-profile-unavailable"><p>{state.profile.phase === 'loading' ? 'Loading profile…' : 'This profile is unavailable.'}</p>{state.profile.phase === 'error' && <button type="button" onClick={() => void controller.openProfile(userId)}>Reload profile</button>}</div>}
  </ConcertWindow></section>
}
