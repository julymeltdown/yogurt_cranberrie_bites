import { createContext, useContext, useEffect, useState, type ReactNode } from 'react'
import { useParams } from 'react-router-dom'
import { GiftDetail, GiftDiscovery, type GiftDiscoveryController } from '../features/gift-discovery'
import { MyGifts } from '../features/my-gifts'
import { PartyDetail, PartyDiscovery, type PartyController, type PartyDiscoveryPresentation } from '../features/party-discovery'
import { WholeCharacterView, validateWholeCharacterConfig } from '../entities/avatar/whole-character'
import { useCharacterRuntime, useReviewedCharacterOwner } from './character-entry/provider'
import type { PrivacyScope } from '../shared/query/privacy'
import { usePorts } from './access'
import type { ShellBootstrap } from './ports'
import { ReviewedCharacterView } from './character-entry/ReviewedCharacterView'
import { RootGiftImage } from './media-entry/RootMediaView'
import { CommunityEntryProvider } from './community-entry'
import { createCommunityOwner } from './community-adoption/owner'
import { usePartyCreation } from './community-adoption/usePartyCreation'
import type { CommunityBootstrapBinding } from './community-adoption/bootstrap'
const renderGiftImage = (request: Parameters<typeof RootGiftImage>[0]) => <RootGiftImage {...request} />

const GiftsContext = createContext<GiftDiscoveryController | null>(null)
const PartiesContext = createContext<PartyController | null>(null)

/** One feature owner per validated shell, surviving panel close/navigation.
 * Allocate in an effect so StrictMode cleanup never leaves a disposed singleton.
 * Merely entering the world does not start a gift request.
 */
export default function CommunityFeatures({ bootstrap, privacyScope, binding = null, children }: {
  bootstrap: ShellBootstrap; privacyScope: PrivacyScope | null; binding?: CommunityBootstrapBinding | null; children: ReactNode
}) {
  const { auth } = usePorts()
  const [owner, setOwner] = useState<{ auth: typeof auth; bootstrap: ShellBootstrap; scope: PrivacyScope;
    binding: typeof binding; value: ReturnType<typeof createCommunityOwner> } | null>(null)
  useEffect(() => {
    if (!auth || !privacyScope || !auth.privacy.isCurrent(privacyScope)) return
    const value = createCommunityOwner({ auth, bootstrap, scope: privacyScope, binding })
    setOwner({ auth, bootstrap, scope: privacyScope, binding, value })
    return () => value.dispose()
  }, [auth, bootstrap, privacyScope, binding])
  const current = owner?.auth === auth && owner?.bootstrap === bootstrap && owner?.scope === privacyScope && owner.binding === binding
    && privacyScope && auth?.privacy.isCurrent(privacyScope) ? owner.value : null
  return <GiftsContext value={current?.gifts ?? null}><PartiesContext value={current?.parties ?? null}><CommunityEntryProvider auth={auth ?? null} release={current?.release ?? null}>{children}</CommunityEntryProvider></PartiesContext></GiftsContext>
}

export function PartiesPanel({ detail = false }: { detail?: boolean }) {
  const controller = useContext(PartiesContext)
  const canCreate = usePartyCreation()
  const { partyId = '' } = useParams()
  const { auth } = usePorts()
  const runtime = useCharacterRuntime(), reviewed = useReviewedCharacterOwner()
  const renderOwnerPortrait: PartyDiscoveryPresentation['renderOwnerPortrait'] = owner => {
    const avatar = owner.avatar
    if (!avatar || typeof avatar !== 'object' || !('config' in avatar)) return null
    if (reviewed) return <ReviewedCharacterView owner={reviewed} config={avatar.config} label={`${owner.display_name}'s avatar`} size={64} />
    if (!runtime) return null
    try {
      validateWholeCharacterConfig(avatar.config)
      return <WholeCharacterView runtime={runtime} config={avatar.config} label={`${owner.display_name}'s avatar`} size={64} />
    } catch { return null }
  }
  if (!controller) return <p role="status">{auth ? 'Opening parties…' : 'Party access is unavailable.'}</p>
  return detail ? <PartyDetail controller={controller} partyId={partyId} /> : <PartyDiscovery controller={controller} presentation={{ canCreate, ...((runtime || reviewed) ? { renderOwnerPortrait } : {}) }} />
}

export function GiftsPanel({ view }: { view: 'list' | 'detail' | 'mine' }) {
  const controller = useContext(GiftsContext)
  const { giveawayId = '' } = useParams()
  const { auth } = usePorts()
  if (!controller) return <p role="status">{auth ? 'Opening gifts…' : 'Gift access is unavailable.'}</p>
  return view === 'mine' ? <MyGifts controller={controller} />
    : view === 'detail' ? <GiftDetail controller={controller} giveawayId={giveawayId} />
      : <GiftDiscovery controller={controller} renderGiftImage={renderGiftImage} />
}
