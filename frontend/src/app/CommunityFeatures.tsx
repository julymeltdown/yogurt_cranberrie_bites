import { createContext, useContext, useEffect, useState, type ReactNode } from 'react'
import { useParams } from 'react-router-dom'
import { createGiftDiscoveryController, GiftDetail, GiftDiscovery, type GiftDiscoveryController } from '../features/gift-discovery'
import { MyGifts } from '../features/my-gifts'
import type { PrivacyScope } from '../shared/query/privacy'
import { usePorts } from './access'
import type { ShellBootstrap } from './ports'

const GiftsContext = createContext<GiftDiscoveryController | null>(null)

/** One feature owner per validated shell, surviving panel close/navigation.
 * Allocate in an effect so StrictMode cleanup never leaves a disposed singleton.
 * Merely entering the world does not start a gift request.
 */
export default function CommunityFeatures({ bootstrap, privacyScope, children }: {
  bootstrap: ShellBootstrap; privacyScope: PrivacyScope | null; children: ReactNode
}) {
  const { auth } = usePorts()
  const [owner, setOwner] = useState<{ auth: typeof auth; bootstrap: ShellBootstrap; scope: PrivacyScope; gifts: GiftDiscoveryController } | null>(null)
  useEffect(() => {
    if (!auth || !privacyScope || !auth.privacy.isCurrent(privacyScope)) return
    const gifts = createGiftDiscoveryController({ auth, context: {
      read: () => auth.privacy.isCurrent(privacyScope) ? {
        scope: privacyScope, capabilities: bootstrap.capabilities, eventPhase: bootstrap.event.phase,
      } : null,
      subscribe: listener => auth.privacy.subscribe(listener),
    } })
    setOwner({ auth, bootstrap, scope: privacyScope, gifts })
    return () => gifts.dispose()
  }, [auth, bootstrap, privacyScope])
  const current = owner?.auth === auth && owner?.bootstrap === bootstrap && owner?.scope === privacyScope
    && privacyScope && auth?.privacy.isCurrent(privacyScope) ? owner.gifts : null
  return <GiftsContext value={current}>{children}</GiftsContext>
}

export function GiftsPanel({ view }: { view: 'list' | 'detail' | 'mine' }) {
  const controller = useContext(GiftsContext)
  const { giveawayId = '' } = useParams()
  const { auth } = usePorts()
  if (!controller) return <p role="status">{auth ? 'Opening gifts…' : 'Gift access is unavailable.'}</p>
  return view === 'mine' ? <MyGifts controller={controller} />
    : view === 'detail' ? <GiftDetail controller={controller} giveawayId={giveawayId} />
      : <GiftDiscovery controller={controller} />
}
