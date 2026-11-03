import { useEffect } from 'react'
import { AvatarEditor } from '../../features/avatar'
import { MeHub, ProfileForm } from '../../features/me'
import { SettingsPage } from '../../features/settings'
import { useAccountOwner } from './provider'
export type AccountView = 'me' | 'profile' | 'avatar' | 'settings'
export default function AccountPanel({ view }: { view: AccountView }) {
  const owner = useAccountOwner()
  useEffect(() => { if (owner && view !== 'avatar' && owner.me.read().phase === 'idle') void owner.me.load() }, [owner, view])
  if (!owner) return <p role="status">Account access is unavailable until your current event and session are verified.</p>
  if (view === 'avatar') return <AvatarEditor controller={owner.avatar} />
  if (view === 'profile') return <ProfileForm controller={owner.me} />
  if (view === 'settings') return <SettingsPage controller={owner.me} auth={owner.auth} />
  return <MeHub controller={owner.me} auth={owner.auth} {...(owner.source ? { thumbnailSource: owner.source } : {})} />
}
