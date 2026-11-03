import { useEffect, useSyncExternalStore } from 'react'
import { Link } from 'react-router-dom'
import type { PeopleController } from '../people/controller'
import { ReviewedProfileAvatar, type ProfileAvatarOptions } from './Avatar'
import '../people/people.css'
export function PublicProfile({ controller, userId, avatars, sharing, blocking }: { controller: PeopleController; userId: string; avatars?: ProfileAvatarOptions | undefined; sharing?: ((userId: string) => void) | undefined; blocking?: ((userId: string) => void) | undefined }) {
  const state = useSyncExternalStore(controller.subscribe, controller.read, controller.read)
  useEffect(() => { controller.start(); if (state.available) void controller.openProfile(userId); return () => controller.closeProfile() }, [controller, userId, state.available])
  const profile = state.available && state.target === userId && state.profile.phase === 'ready' ? state.profile.data : null
  return <section className="intro-surface"><Link to="/meet/people">Back to people</Link><h1>PROFILE</h1>
    {profile ? <><ReviewedProfileAvatar config={profile.avatar.config} label={`${profile.display_name}'s avatar`} options={avatars} /><h2>{profile.display_name}</h2><p className="intro-user-text">{profile.bio ?? 'No bio shared.'}</p>
      <p>Social handles and verified age are private. Sharing requires mutual consent.</p><div className="intro-actions"><button disabled={!sharing} onClick={() => sharing?.(userId)}>Request sharing</button><Link to={`/report?targetType=USER&targetId=${userId}`}>Report profile</Link><button disabled={!blocking} onClick={() => blocking?.(userId)}>Block user</button><Link to="/me/blocks">Manage blocks</Link></div>{(!sharing || !blocking) && <p>Some account actions are currently unavailable.</p>}</>
      : <div role="status"><p>{state.profile.phase === 'loading' ? 'Loading profile…' : 'This profile is unavailable.'}</p>{state.profile.phase === 'error' && <button onClick={() => void controller.openProfile(userId)}>Reload profile</button>}</div>}
  </section>
}
