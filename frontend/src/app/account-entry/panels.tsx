import { useEffect } from 'react'
import { AvatarEditor } from '../../features/avatar'
import { MeHub, ProfileForm } from '../../features/me'
import { SettingsPage } from '../../features/settings'
import { useRuntimePreferences } from '../../features/settings/runtime-adoption/useRuntimePreferences'
import { useWorldPresentation } from '../world-panels'
import { useAccountOwner } from './provider'
import { useCharacterRuntime, useReviewedCharacterOwner } from '../character-entry/provider'
import { WholeCharacterView } from '../../entities/avatar/whole-character'
import { MotionEditor } from '../../features/avatar/motion-adoption/MotionEditor'
import { ReviewedCharacterView } from '../character-entry/ReviewedCharacterView'
import { WholeCharacterEditor } from '../../features/avatar/whole-character/WholeCharacterEditor'
import { AvatarThumbnail } from '../../features/me/ui/AvatarThumbnail'
import { useLoadedProfileActivity } from '../character-entry/activity'
import { RootProfilePostImage } from '../media-entry/RootProfilePostImage'
export type AccountView = 'me' | 'profile' | 'avatar' | 'settings'
export default function AccountPanel({ view }: { view: AccountView }) {
  const owner = useAccountOwner()
  const presentation = useWorldPresentation()
  const runtimePreferences = useRuntimePreferences(owner?.auth ?? null, owner?.scope ?? null, presentation)
  const runtime = useCharacterRuntime(), reviewed = useReviewedCharacterOwner()
  const activity = useLoadedProfileActivity(owner?.scope.userId ?? '')
  useEffect(() => { if (owner && view !== 'avatar' && owner.me.read().phase === 'idle') void owner.me.load() }, [owner, view])
  if (!owner) return <p role="status">Account access is unavailable until your current event and session are verified.</p>
  if (view === 'avatar' && (owner.bootstrap.document?.avatar_manifest as Record<string, unknown> | undefined)?.representation === 'reviewed_selected_source_motion'
    && !owner.motionAvatar) return <p role="status">Your character collection is unavailable.</p>
  if (view === 'avatar') return owner.motionAvatar ? <MotionEditor controller={owner.motionAvatar.controller} /> : owner.wholeAvatar ? <WholeCharacterEditor controller={owner.wholeAvatar} /> : <AvatarEditor controller={owner.avatar} />
  if (view === 'profile') return <ProfileForm controller={owner.me} />
  if (view === 'settings') return <SettingsPage controller={owner.me} auth={owner.auth}
    {...(runtimePreferences && presentation ? { runtime: runtimePreferences, theme: presentation.theme } : {})} />
  return <MeHub controller={owner.me} auth={owner.auth} {...(owner.source ? { thumbnailSource: owner.source } : {})}
    {...(activity ? { activity } : {})}
    renderPostMedia={post => <RootProfilePostImage auth={owner.auth} scope={owner.scope} post={post} />}
    {...(reviewed ? { renderAvatar: (avatar, label) => <ReviewedCharacterView owner={reviewed} config={avatar.config} label={label} size={128} /> } : runtime ? { renderAvatar: (avatar, label) => avatar.config.contract_version === '1.1.0'
      ? <WholeCharacterView runtime={runtime} config={avatar.config} label={label} size={128} />
      : <AvatarThumbnail auth={owner.auth} config={avatar.config} {...(owner.source ? { source: owner.source } : {})} /> } : {})} />
}
