import { useId, useSyncExternalStore, type ReactNode } from 'react'
import { ShellLink } from '../../../app/navigation'
import type { SessionController } from '../../auth/model/session'
import type { MeController } from '../model/controller'
import { AvatarThumbnail, type ReviewedThumbnailSource } from './AvatarThumbnail'
import './me.css'
export function useMe(controller: MeController) { return useSyncExternalStore(controller.subscribe, controller.read, controller.read) }
export function MeStatus({ controller }: { controller: MeController }) {
  const s = useMe(controller), busy = ['loading', 'saving', 'expired', 'disposed'].includes(s.phase)
  return <><p role="status" aria-live="polite" aria-atomic="true">{s.message || 'Your profile and preferences are optional. You can enter the world without completing them.'}</p>
    {!busy && <button type="button" onClick={() => void controller.load()}>{s.participant ? 'Discard drafts and reload saved version' : 'Load your profile'}</button>}</>
}
export function MeHub({ controller, auth, thumbnailSource }: { controller: MeController; auth: Pick<SessionController, 'privacy'>; thumbnailSource?: ReviewedThumbnailSource }) {
  const s = useMe(controller)
  return <section className="me-view" aria-label="ME"><h2>ME</h2><MeStatus controller={controller} />
    {s.participant && <><AvatarThumbnail auth={auth} config={s.participant.profile.avatar.config} {...(thumbnailSource ? { source: thumbnailSource } : {})} />
      <h3>{s.participant.profile.display_name}</h3><p className="me-bio">{s.participant.profile.bio || 'Add a bio whenever you like.'}</p><p>{s.participant.profile.bio ? 'Bio added.' : 'Bio is optional.'} Your profile does not affect world entry.</p></>}
    <nav className="me-links" aria-label="Your profile"><ShellLink id="me-edit-profile" to="/me/profile">Edit profile</ShellLink><ShellLink id="me-avatar" to="/me/avatar">Dress up</ShellLink><ShellLink id="me-settings" to="/me/settings">Settings</ShellLink></nav>
    <h3>Your activity</h3><nav className="me-links" aria-label="Your activity"><ShellLink id="me-parties" to="/meet">Parties and MY PLAN</ShellLink><ShellLink id="me-gifts" to="/gifts/mine">My gifts</ShellLink></nav><ShellLink id="me-enter-world" to="/world">Back to world</ShellLink>
  </section>
}
/** Owner extension UI owns its own qualified persistence; this form never invents wire fields. */
export interface OptionalProfileFieldsPort { readonly render: (context: { controller: MeController }) => ReactNode }
export function ProfileForm({ controller, optionalFields }: { controller: MeController; optionalFields?: OptionalProfileFieldsPort }) {
  const s = useMe(controller), id = useId(), disabled = !s.participant || ['loading', 'saving', 'expired', 'disposed'].includes(s.phase)
  return <section className="me-view" aria-label="Edit profile"><h2>EDIT PROFILE</h2><p>Bio, ticket section and visibility choices are optional. Your world access does not depend on them.</p><MeStatus controller={controller} />
    <form onSubmit={e => { e.preventDefault(); void controller.save('profile') }}><fieldset disabled={disabled}><legend>Public profile</legend>
      <label htmlFor={`${id}-name`}>Display name<input id={`${id}-name`} autoComplete="nickname" value={s.draft.display_name} onChange={e => controller.editProfile({ display_name: e.target.value })} aria-describedby={`${id}-name-hint`} required /></label><p id={`${id}-name-hint`}>1–40 characters. Other people can see your display name and bio.</p>
      <label htmlFor={`${id}-bio`}>Bio (optional)<textarea id={`${id}-bio`} rows={4} value={s.draft.bio ?? ''} onChange={e => controller.editProfile({ bio: e.target.value || null })} aria-describedby={`${id}-bio-hint`} /></label><p id={`${id}-bio-hint`}>{Array.from(s.draft.bio ?? '').length}/160 characters. Keep private contact details out of your public bio.</p>
      <div className="me-actions"><button type="submit" disabled={s.blocked || !controller.dirty('profile')}>Save profile</button><button type="button" onClick={() => controller.cancel('profile')} disabled={!controller.dirty('profile')}>Cancel profile changes</button></div>
    </fieldset></form>
    {optionalFields ? optionalFields.render({ controller }) : <p>Ticket section and visibility controls are unavailable in this connection. No values are sent or inferred.</p>}
    <ShellLink id="profile-back-world" to="/world">Back to world</ShellLink>
  </section>
}
