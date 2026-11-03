import { useId, useSyncExternalStore, type ReactNode } from 'react'
import { ProfileActivity, type ProfileActivityInput } from './reference-profile/ProfileActivity'
import { ProfileIcon } from '../../profile/reference-presentation/ProfileIcon'
import { ShellLink } from '../../../app/navigation'
import type { SessionController } from '../../auth/model/session'
import type { Participant } from '../model/contract'
import type { MeController } from '../model/controller'
import { AvatarThumbnail, type ReviewedThumbnailSource } from './AvatarThumbnail'
import { ConcertAction, ConcertIcon, ConcertWindow } from '../../../shared/ui/concert-window'
import './me.css'
export function useMe(controller: MeController) { return useSyncExternalStore(controller.subscribe, controller.read, controller.read) }
export function MeStatus({ controller }: { controller: MeController }) {
  const s = useMe(controller), busy = ['loading', 'saving', 'expired', 'disposed'].includes(s.phase)
  return <><p role="status" aria-live="polite" aria-atomic="true">{s.message || 'Your profile and preferences are optional. You can enter the world without completing them.'}</p>
    {!busy && <button type="button" onClick={() => void controller.load()}>{s.participant ? 'Discard drafts and reload saved version' : 'Load your profile'}</button>}</>
}
export function MeHub({ controller, auth, thumbnailSource, activity, renderAvatar }: { controller: MeController; auth: Pick<SessionController, 'privacy'>; thumbnailSource?: ReviewedThumbnailSource; activity?: ProfileActivityInput; renderAvatar?: (avatar: Participant['profile']['avatar'], label: string) => ReactNode }) {
  const s = useMe(controller)
  return <section className="me-view me-view--concert me-hub me-hub--reference" aria-label="ME"><ConcertWindow title="My profile" icon="me" footer={<ShellLink id="me-enter-world" to="/world">Back to world</ShellLink>}>
    <div className="me-profile-header">
      {s.participant ? <div className="me-identity">{renderAvatar ? <div className="me-avatar me-avatar--presentation">{renderAvatar(s.participant.profile.avatar, `${s.participant.profile.display_name}'s avatar`)}</div> : s.participant.profile.avatar.config.contract_version === '1.0.0' ? <AvatarThumbnail auth={auth} config={s.participant.profile.avatar.config} {...(thumbnailSource ? { source: thumbnailSource } : {})} /> : <div className="me-avatar"><p role="status">Selected character portrait unavailable in this connection.</p></div>}
        <div className="me-identity__copy"><h3>{s.participant.profile.display_name}</h3><p className="me-saved-era">{s.participant.preferences.theme}</p><p className="me-bio">{s.participant.profile.bio || 'No bio yet. Add one whenever you like.'}</p></div></div>
        : <div className="me-identity-unavailable" role="status">{s.message || 'Your saved profile is not loaded yet.'}</div>}
      <nav className="me-profile-icons" aria-label="Profile actions">
        <ShellLink id="me-edit-profile" to="/me/profile" aria-label="Edit profile" title="Edit profile"><ProfileIcon name="edit" /></ShellLink>
        <ShellLink id="me-avatar" to="/me/avatar" aria-label="Dress up" title="Dress up"><ConcertIcon name="star" /></ShellLink>
        <ShellLink id="me-social" to="/me/social" aria-label="Manage private social links" title="Manage private social links"><ProfileIcon name="link" /></ShellLink>
        <ShellLink id="me-requests" to="/me/requests" aria-label="Sharing requests" title="Sharing requests"><ConcertIcon name="meet" /></ShellLink>
      </nav>
    </div>
    <nav className="me-activity-navigation" aria-label="Your activity"><span aria-current="page">Activity</span><ShellLink id="me-parties" to="/meet" aria-label="Parties and MY PLAN">Parties</ShellLink><ShellLink id="me-gifts" to="/gifts/mine" aria-label="My gifts">Gifts</ShellLink></nav>
    {s.participant && <ProfileActivity subject={s.participant.user_id} privacy={auth.privacy} input={activity} />}
    <details className="me-profile-options"><summary>Profile options</summary><p className="me-optionality">Profile details are optional. You can enter the world without completing them.</p><ShellLink id="me-settings" to="/me/settings">Settings</ShellLink><div className="me-status"><MeStatus controller={controller} /></div></details>
  </ConcertWindow>
  </section>
}
/** Owner extension UI owns its own qualified persistence; this form never invents wire fields. */
export interface OptionalProfileFieldsPort { readonly render: (context: { controller: MeController }) => ReactNode }
export function ProfileForm({ controller, optionalFields }: { controller: MeController; optionalFields?: OptionalProfileFieldsPort }) {
  const s = useMe(controller), id = useId(), disabled = !s.participant || ['loading', 'saving', 'expired', 'disposed'].includes(s.phase)
  return <section className="me-view me-view--concert me-profile-form" aria-label="Edit profile"><ConcertWindow title="Edit profile" icon="me" footer={<ShellLink id="profile-back-world" to="/world">Back to world</ShellLink>}><p className="me-optionality">Bio, ticket section and visibility choices are optional. Your world access does not depend on them.</p><div className="me-status"><MeStatus controller={controller} /></div>
    <form onSubmit={e => { e.preventDefault(); void controller.save('profile') }}><fieldset disabled={disabled}><legend>Public profile</legend>
      <label htmlFor={`${id}-name`}>Display name<input id={`${id}-name`} autoComplete="nickname" value={s.draft.display_name} onChange={e => controller.editProfile({ display_name: e.target.value })} aria-describedby={`${id}-name-hint`} required /></label><p id={`${id}-name-hint`}>1–40 characters. Other people can see your display name and bio.</p>
      <label htmlFor={`${id}-bio`}>Bio (optional)<textarea id={`${id}-bio`} rows={4} value={s.draft.bio ?? ''} onChange={e => controller.editProfile({ bio: e.target.value || null })} aria-describedby={`${id}-bio-hint`} /></label><p id={`${id}-bio-hint`}>{Array.from(s.draft.bio ?? '').length}/160 characters. Keep private contact details out of your public bio.</p>
      <div className="me-actions"><ConcertAction type="submit" pending={s.pending === 'profile'} disabled={disabled || s.blocked || !controller.dirty('profile')}>Save profile</ConcertAction><ConcertAction tone="paper" onClick={() => controller.cancel('profile')} disabled={disabled || !controller.dirty('profile')}>Cancel profile changes</ConcertAction></div>
    </fieldset></form>
    {optionalFields ? optionalFields.render({ controller }) : <p>Ticket section and visibility controls are unavailable in this connection. No values are sent or inferred.</p>}
  </ConcertWindow>
  </section>
}
