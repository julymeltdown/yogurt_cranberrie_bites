import { useId, useSyncExternalStore } from 'react'
import type { ThemeController } from '../model/controller'
import { themeIds } from '../model/preferences'
import './theme-selector.css'

export function ThemeSelector({ controller }: { controller: ThemeController }) {
  const state = useSyncExternalStore(controller.subscribe, controller.read, controller.read), id = useId()
  return <section className="mcr-theme-selector" aria-labelledby={`${id}-title`}>
    <h2 id={`${id}-title`}>CHOOSE AN ERA</h2>
    <p id={`${id}-hint`}>One world, three skins. Your room, position and outfit stay with you.</p>
    <fieldset aria-describedby={`${id}-hint ${id}-status`} disabled={state.phase === 'disposed'}>
      <legend className="mcr-theme-selector__legend">World skin</legend>
      <div className="mcr-theme-selector__choices">
        {themeIds.map(theme => <label key={theme}>
          <input type="radio" name={`${id}-era`} value={theme} checked={(state.pending ?? state.selected) === theme}
            onChange={() => void controller.select(theme)}
            onClick={() => { if (state.phase !== 'loading' && state.selected === theme) void controller.select(theme) }} />
          <span>{theme}{state.pending === theme && <span aria-hidden="true"> …</span>}</span>
        </label>)}
      </div>
    </fieldset>
    <p id={`${id}-status`} role="status" aria-live="polite" aria-atomic="true">{state.message || `${state.selected} selected.`}</p>
  </section>
}
