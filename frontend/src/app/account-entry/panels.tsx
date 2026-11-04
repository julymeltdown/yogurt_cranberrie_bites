import { useEffect } from 'react'
import { AvatarEditor } from '../../features/avatar'
import { MeHub, ProfileForm } from '../../features/me'
import { SettingsPage } from '../../features/settings'
import { useRuntimePreferences } from '../../features/settings/runtime-adoption/useRuntimePreferences'
import { useWorldPresentation } from '../world-panels'
import { useAccountOwner } from './provider'
import { useCharacterRuntime } from '../character-entry/provider'
import { WholeCharacterView } from '../../entities/avatar/whole-character'
import { WholeCharacterEditor } from '../../features/avatar/whole-character/WholeCharacterEditor'
import { AvatarThumbnail } from '../../features/me/ui/AvatarThumbnail'
import { useLoadedProfileActivity } from '../character-entry/activity'
export type AccountView = 'me' | 'profile' | 'avatar' | 'settings'
export default function AccountPanel({ view }: { view: AccountView }) {
  const owner = useAccountOwner()
  const presentation = useWorldPresentation()
  const runtimePreferences = useRuntimePreferences(owner?.auth ?? null, owner?.scope ?? null, presentation)
  const runtime = useCharacterRuntime()
  const activity = useLoadedProfileActivity(owner?.scope.userId ?? '')
  useEffect(() => { if (owner && view !== 'avatar' && owner.me.read().phase === 'idle') void owner.me.load() }, [owner, view])
  if (!owner) return <p role="status">Account access is unavailable until your current event and session are verified.</p>
  if (view === 'avatar') return owner.wholeAvatar ? <WholeCharacterEditor controller={owner.wholeAvatar} /> : <AvatarEditor controller={owner.avatar} />
  if (view === 'profile') return <ProfileForm controller={owner.me} />
  if (view === 'settings') return <SettingsPage controller={owner.me} auth={owner.auth}
    {...(runtimePreferences && presentation ? { runtime: runtimePreferences, theme: presentation.theme } : {})} />
  return <MeHub controller={owner.me} auth={owner.auth} {...(owner.source ? { thumbnailSource: owner.source } : {})}
    {...(activity ? { activity } : {})}
    {...(runtime ? { renderAvatar: (avatar, label) => avatar.config.contract_version === '1.1.0'
      ? <WholeCharacterView runtime={runtime} config={avatar.config} label={label} size={128} />
      : <AvatarThumbnail auth={owner.auth} config={avatar.config} {...(owner.source ? { source: owner.source } : {})} /> } : {})} />
}
