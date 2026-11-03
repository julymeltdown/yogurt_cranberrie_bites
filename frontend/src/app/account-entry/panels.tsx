import { useEffect } from 'react'
import { AvatarEditor } from '../../features/avatar'
import { MeHub, ProfileForm } from '../../features/me'
import { SettingsPage } from '../../features/settings'
import { useAccountOwner } from './provider'
import { useCharacterRuntime } from '../character-entry/provider'
import { WholeCharacterView } from '../../entities/avatar/whole-character'
import { WholeCharacterEditor } from '../../features/avatar/whole-character/WholeCharacterEditor'
export type AccountView = 'me' | 'profile' | 'avatar' | 'settings'
export default function AccountPanel({ view }: { view: AccountView }) {
  const owner = useAccountOwner()
  const runtime = useCharacterRuntime()
  useEffect(() => { if (owner && view !== 'avatar' && owner.me.read().phase === 'idle') void owner.me.load() }, [owner, view])
  if (!owner) return <p role="status">Account access is unavailable until your current event and session are verified.</p>
  if (view === 'avatar') return owner.wholeAvatar ? <WholeCharacterEditor controller={owner.wholeAvatar} /> : <AvatarEditor controller={owner.avatar} />
  if (view === 'profile') return <ProfileForm controller={owner.me} />
  if (view === 'settings') return <SettingsPage controller={owner.me} auth={owner.auth} />
  return <MeHub controller={owner.me} auth={owner.auth} {...(owner.source ? { thumbnailSource: owner.source } : {})}
    {...(runtime ? { renderAvatar: (avatar, label) => <WholeCharacterView runtime={runtime} config={avatar.config} label={label} size={128} /> } : {})} />
}
