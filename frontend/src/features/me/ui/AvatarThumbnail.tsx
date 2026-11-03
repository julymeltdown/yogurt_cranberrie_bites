import { useLayoutEffect, useRef, useState } from 'react'
import { createReviewedAvatarCompositor, type AvatarConfig, type AvatarCompositor } from '../../../entities/avatar/runtime'
import type { SessionController } from '../../auth/model/session'
import type { PrivacyScope } from '../../../shared/query/privacy'
/** Preserve the captured bootstrap lease; never attach a fresh lease to stale assets. */
export interface ReviewedThumbnailSource {
  readonly scope: PrivacyScope
  readonly options: Omit<Parameters<typeof createReviewedAvatarCompositor>[0], 'signal'>
}
// options.leases, when supplied by the account owner, retains only the verified atlas.
// Each mount still owns its compositor, thumbnail canvas and cancellation independently.
export function AvatarThumbnail({ auth, config, source }: { auth: Pick<SessionController, 'privacy'>; config: AvatarConfig; source?: ReviewedThumbnailSource }) {
  const host = useRef<HTMLDivElement>(null), [message, setMessage] = useState('Avatar preview unavailable. Your saved outfit is unchanged.')
  useLayoutEffect(() => {
    let active = true, compositor: AvatarCompositor | null = null, thumbnail: ReturnType<AvatarCompositor['thumbnail']> | null = null
    const lifetime = new AbortController()
    setMessage('Avatar preview unavailable. Your saved outfit is unchanged.')
    const clear = () => { lifetime.abort(); thumbnail?.dispose(); compositor?.dispose(); host.current?.replaceChildren(); if (active) setMessage('Avatar preview unavailable. Your saved outfit is unchanged.') }
    const remove = auth.privacy.registerCleanup(clear)
    if (source && auth.privacy.isCurrent(source.scope)) {
      setMessage('Loading reviewed avatar preview…')
      void auth.privacy.run(source.scope, async ({ signal }) => {
        const candidate = await createReviewedAvatarCompositor({ ...source.options, signal })
        if (!active || signal.aborted || !auth.privacy.isCurrent(source.scope)) { candidate.dispose(); return }
        compositor = candidate
        thumbnail = candidate.thumbnail(config, { direction: 'down', animation: 'idle', frame: 0 }, lifetime.signal)
        if (!active || !auth.privacy.isCurrent(source.scope)) { clear(); return }
        thumbnail.canvas.setAttribute('role', 'img'); thumbnail.canvas.setAttribute('aria-label', 'Your saved avatar, front view')
        host.current?.replaceChildren(thumbnail.canvas)
        setMessage(thumbnail.fallback ? 'Reviewed fallback preview. Your saved outfit is unchanged.' : '')
      }, lifetime.signal).catch(() => { if (active) clear() })
    }
    return () => { active = false; remove(); clear() }
  }, [auth, config, source])
  return <div className="me-avatar"><div ref={host} />{message && <p role="status">{message}</p>}</div>
}
