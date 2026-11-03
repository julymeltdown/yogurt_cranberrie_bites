import { createContext, useContext, useEffect, useState, type ReactNode } from 'react'
import { useParams } from 'react-router-dom'
import { createGiftDiscoveryController, GiftDetail, GiftDiscovery, type GiftDiscoveryController } from '../features/gift-discovery'
import { MyGifts } from '../features/my-gifts'
import { createPartyController, PartyDetail, PartyDiscovery, type PartyController, type PartyDiscoveryPresentation } from '../features/party-discovery'
import { WholeCharacterView, validateWholeCharacterConfig } from '../entities/avatar/whole-character'
import { useCharacterRuntime } from './character-entry/provider'
import type { PrivacyScope } from '../shared/query/privacy'
import { usePorts } from './access'
import type { ShellBootstrap } from './ports'
import { RootGiftImage } from './media-entry/RootMediaView'
const renderGiftImage = (request: Parameters<typeof RootGiftImage>[0]) => <RootGiftImage {...request} />

const GiftsContext = createContext<GiftDiscoveryController | null>(null)
const PartiesContext = createContext<PartyController | null>(null)

/** One feature owner per validated shell, surviving panel close/navigation.
 * Allocate in an effect so StrictMode cleanup never leaves a disposed singleton.
 * Merely entering the world does not start a gift request.
 */
export default function CommunityFeatures({ bootstrap, privacyScope, children }: {
  bootstrap: ShellBootstrap; privacyScope: PrivacyScope | null; children: ReactNode
}) {
  const { auth } = usePorts()
  const [owner, setOwner] = useState<{ auth: typeof auth; bootstrap: ShellBootstrap; scope: PrivacyScope;
    gifts: GiftDiscoveryController; parties: PartyController } | null>(null)
  useEffect(() => {
    if (!auth || !privacyScope || !auth.privacy.isCurrent(privacyScope)) return
    const gifts = createGiftDiscoveryController({ auth, context: {
      read: () => auth.privacy.isCurrent(privacyScope) ? {
        scope: privacyScope, capabilities: bootstrap.capabilities, eventPhase: bootstrap.event.phase,
      } : null,
      subscribe: listener => auth.privacy.subscribe(listener),
    } })
    const parties = createPartyController({ auth, access: {
      read: () => auth.privacy.isCurrent(privacyScope) ? {
        scope: privacyScope, capabilities: bootstrap.capabilities, phase: bootstrap.event.phase,
      } : null,
      subscribe: listener => auth.privacy.subscribe(listener),
    } })
    setOwner({ auth, bootstrap, scope: privacyScope, gifts, parties })
    return () => { gifts.dispose(); parties.dispose() }
  }, [auth, bootstrap, privacyScope])
  const current = owner?.auth === auth && owner?.bootstrap === bootstrap && owner?.scope === privacyScope
    && privacyScope && auth?.privacy.isCurrent(privacyScope) ? owner : null
  return <GiftsContext value={current?.gifts ?? null}><PartiesContext value={current?.parties ?? null}>{children}</PartiesContext></GiftsContext>
}

export function PartiesPanel({ detail = false }: { detail?: boolean }) {
  const controller = useContext(PartiesContext)
  const { partyId = '' } = useParams()
  const { auth } = usePorts()
  const runtime = useCharacterRuntime()
  const renderOwnerPortrait: PartyDiscoveryPresentation['renderOwnerPortrait'] = owner => {
    const avatar = owner.avatar
    if (!runtime || !avatar || typeof avatar !== 'object' || !('config' in avatar)) return null
    try {
      validateWholeCharacterConfig(avatar.config)
      return <WholeCharacterView runtime={runtime} config={avatar.config} label={`${owner.display_name}'s avatar`} size={64} />
    } catch { return null }
  }
  if (!controller) return <p role="status">{auth ? 'Opening parties…' : 'Party access is unavailable.'}</p>
  return detail ? <PartyDetail controller={controller} partyId={partyId} /> : <PartyDiscovery controller={controller} {...(runtime ? { presentation: { renderOwnerPortrait } } : {})} />
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
