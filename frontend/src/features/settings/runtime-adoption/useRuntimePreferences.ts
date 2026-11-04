import { useLayoutEffect, useState } from 'react'
import type { SessionController } from '../../auth/model/session'
import type { PrivacyScope } from '../../../shared/query/privacy'
import { createRuntimePreferencesAdapter, type RuntimeEffects, type RuntimePreferencesAdapter, type RuntimeSound, type RuntimeTheme } from '.'

/** Projection of the existing presentation release; composition provides the same owner.
 * No app/world implementation import and no alternate runtime construction. */
export interface PreferencePresentation {
  readonly request: { readonly privacyScope: PrivacyScope; readonly signal: AbortSignal }
  readonly theme: RuntimeTheme
  readonly runtimePreferences?: { readonly effects: RuntimeEffects; readonly sound?: RuntimeSound }
}
const same = (a: PrivacyScope, b: PrivacyScope) => a.userId === b.userId && a.eventId === b.eventId
  && a.generation === b.generation && a.policyGeneration === b.policyGeneration && a.gate === b.gate

/** Root consumer binding. A missing release stays unavailable, never a fake success port.
 * Call unconditionally; ordinary account panel navigation preserves the incumbent world owner. */
export function useRuntimePreferences(auth: Pick<SessionController, 'privacy'> | null, scope: PrivacyScope | null,
  presentation: PreferencePresentation | null) {
  const [binding, setBinding] = useState<{ presentation: PreferencePresentation; scope: PrivacyScope;
    auth: Pick<SessionController, 'privacy'>; adapter: RuntimePreferencesAdapter } | null>(null)
  useLayoutEffect(() => {
    if (!auth || !scope || !presentation?.runtimePreferences || presentation.request.signal.aborted || !same(scope, presentation.request.privacyScope)
      || !auth.privacy.isCurrent(presentation.request.privacyScope)) return
    const adapter = createRuntimePreferencesAdapter({ privacy: auth.privacy, scope: presentation.request.privacyScope,
      signal: presentation.request.signal, theme: presentation.theme, ...presentation.runtimePreferences })
    setBinding({ presentation, scope, auth, adapter })
    return adapter.dispose
  }, [auth, scope, presentation])
  if (!binding || binding.auth !== auth || binding.scope !== scope || binding.presentation !== presentation
    || !scope || !presentation || presentation.request.signal.aborted || !same(scope, presentation.request.privacyScope)
    || !auth?.privacy.isCurrent(presentation.request.privacyScope) || binding.adapter.read().phase === 'disposed') return undefined
  return binding.adapter
}
