import { useLayoutEffect, useState } from 'react'
import { ShellLink } from '../../app/navigation'
import type { SessionController } from '../auth/model/session'
import type { PrivacyScope } from '../../shared/query/privacy'
import { useMe, MeStatus } from '../me/ui/MePages'
import type { MeController } from '../me/model/controller'
import type { Preferences } from '../me/model/contract'
import { ThemeSelector } from '../theme/ui/ThemeSelector'
import type { ThemeController } from '../theme/model/controller'
/** Supplied by the existing shell owner; never allocate a second runtime or identity. */
export interface RuntimePreferencesPort { apply(preferences: Readonly<Preferences>, scope: PrivacyScope, signal: AbortSignal): Promise<void> }
export function SettingsPage({ controller, auth, runtime, theme }: { controller: MeController; auth: Pick<SessionController, 'privacy'>; runtime?: RuntimePreferencesPort; theme?: ThemeController }) {
  const s = useMe(controller), [application, setApplication] = useState(''), [applying, setApplying] = useState(false)
  const disabled = !s.participant || ['loading', 'saving', 'expired', 'disposed'].includes(s.phase)
  const [lifetime] = useState(() => ({ current: new AbortController() }))
  useLayoutEffect(() => {
    lifetime.current = new AbortController()
    const remove = auth.privacy.registerCleanup(() => { lifetime.current.abort(); setApplication(''); setApplying(false) })
    return () => { remove(); lifetime.current.abort() }
  }, [auth, lifetime])
  async function apply() {
    if (!runtime || !s.participant || disabled || applying || !auth.privacy.isCurrent(controller.scope)) return
    const signal = lifetime.current.signal, preferences = s.participant.preferences
    setApplying(true); setApplication('Applying saved preferences…')
    try {
      await auth.privacy.run(controller.scope, context => runtime!.apply(preferences, controller.scope, context.signal), signal)
      if (!signal.aborted) setApplication('Saved preferences applied in this session.')
    } catch { if (!signal.aborted && auth.privacy.isCurrent(controller.scope)) setApplication('Could not apply preferences. Saved values are unchanged. Try again.') }
    finally { if (!signal.aborted) setApplying(false) }
  }
  return <section className="me-view" aria-label="Settings"><h2>SETTINGS</h2><p>Saved for your account and this event. Sound stays off until you explicitly apply it in this session.</p><MeStatus controller={controller} />
    <form onSubmit={e => { e.preventDefault(); void controller.save('preferences') }}><fieldset disabled={disabled}><legend>Saved preferences</legend>
      <label>Preferred era<select value={s.draft.preferences.theme} onChange={e => controller.editPreferences({ theme: e.target.value as Preferences['theme'] })}>{['REVENGE', 'PARADE', 'DANGER'].map(t => <option key={t}>{t}</option>)}</select></label>
      <label className="me-check"><input type="checkbox" checked={s.draft.preferences.low_power} onChange={e => controller.editPreferences({ low_power: e.target.checked })} />Low power</label>
      <label className="me-check"><input type="checkbox" checked={s.draft.preferences.reduced_motion} onChange={e => controller.editPreferences({ reduced_motion: e.target.checked })} />Reduced motion</label>
      <label className="me-check"><input type="checkbox" checked={s.draft.preferences.sound_enabled} onChange={e => controller.editPreferences({ sound_enabled: e.target.checked })} />Enable sound</label><p className="me-note">Sound is off by default. Saving a preference does not start audio.</p>
      <div className="me-actions"><button type="submit" disabled={s.blocked || !controller.dirty('preferences')}>Save preferences</button><button type="button" disabled={!controller.dirty('preferences')} onClick={() => controller.cancel('preferences')}>Cancel preference changes</button></div>
    </fieldset></form>
    {runtime ? <><button type="button" disabled={disabled || applying || s.blocked || controller.dirty('preferences')} onClick={() => void apply()}>Apply saved preferences in this session</button><p role="status">{application}</p></> : <p>Live preference controls are unavailable in this connection. Saving values does not change the active world or start audio.</p>}
    {theme && !disabled && <ThemeSelector controller={theme} />}
    <ShellLink id="settings-back-world" to="/world">Back to world</ShellLink>
  </section>
}
