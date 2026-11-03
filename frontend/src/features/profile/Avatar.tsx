import { useEffect, useRef, useState } from 'react'
import { createReviewedAvatarCompositor, isLayeredAvatarConfig, validateConfig, type AvatarCompositor } from '../../entities/avatar/runtime'
import type { PrivacyScope } from '../../shared/query/privacy'
import type { SocialOptions } from '../people/runtime'
export type AvatarResources = Omit<Parameters<typeof createReviewedAvatarCompositor>[0], 'signal'>
export interface ProfileAvatarOptions { auth: SocialOptions['auth']; scope: PrivacyScope;
  resolve(scope: PrivacyScope, signal: AbortSignal): Promise<AvatarResources>
  /** Root may forward the account owner's retained source; never rebind it to a fresh epoch. */
  source?: { readonly scope: PrivacyScope; readonly options: AvatarResources }
}
/** Retained original bootstrap scope is required. No synthetic or legacy production fallback. */
export function ReviewedProfileAvatar({ config, label, options }: { config: unknown; label: string; options?: ProfileAvatarOptions | undefined }) {
  const host = useRef<HTMLDivElement>(null), [status, setStatus] = useState('Avatar unavailable')
  useEffect(() => {
    setStatus('Avatar unavailable')
    if (!options || !options.auth.privacy.isCurrent(options.scope) || !isLayeredAvatarConfig(config)) return
    const controller = new AbortController(), { auth, scope } = options
    let owner: Awaited<ReturnType<typeof createReviewedAvatarCompositor>> | null = null, thumbnail: ReturnType<AvatarCompositor['thumbnail']> | null = null
    const dispose = () => { controller.abort(); thumbnail?.canvas.remove(); thumbnail?.dispose(); owner?.dispose(); host.current?.replaceChildren() }
    const unregister = auth.privacy.registerCleanup(dispose)
    void auth.privacy.run(scope, async ({ signal }) => {
      if (options.source && !auth.privacy.isCurrent(options.source.scope)) throw new Error('Expired avatar source')
      const input = options.source?.options ?? await options.resolve(scope, signal)
      const compositor = await createReviewedAvatarCompositor({ ...input, signal })
      if (controller.signal.aborted || !auth.privacy.isCurrent(scope)) { compositor.dispose(); return }
      owner = compositor; validateConfig(config, compositor.manifest)
      thumbnail = compositor.thumbnail(config, { direction: 'down', animation: 'idle', frame: 0 }, signal)
      if (thumbnail.fallback) throw new Error('Unavailable config')
      if (!controller.signal.aborted && auth.privacy.isCurrent(scope)) { host.current?.replaceChildren(thumbnail.canvas); setStatus('') }
    }, controller.signal).catch(() => { dispose(); if (auth.privacy.isCurrent(scope)) setStatus('Avatar unavailable') })
    return () => { unregister(); dispose() }
  }, [config, options])
  return <div className="intro-avatar" role="img" aria-label={status ? `${label}: avatar unavailable` : label}><div ref={host} />{status && <span>{status}</span>}</div>
}
