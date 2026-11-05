import { createContext, useContext, useRef, useState, type ReactNode, type RefObject } from 'react'
import { ShellLink } from '../../../app/navigation'
import { WorldContextStrip } from './world-context'
import './world-hud.css'

interface HudLayout {
  readonly active: boolean
  readonly details: boolean
  readonly toggleDetails: () => void
  readonly toolsOpen: boolean
  readonly onToolsChange: (open: boolean) => void
  readonly trigger: RefObject<HTMLButtonElement | null>
}
const Context = createContext<HudLayout | null>(null)
function useHud() {
  const value = useContext(Context)
  if (!value) throw new Error('World HUD requires its shell presentation slot')
  return value
}

/** Presentation state only. The incumbent shell still owns every game/input lease. */
export function WorldHudLayout({ active, toolsOpen, onToolsChange, children }: {
  active: boolean; toolsOpen: boolean; onToolsChange: (open: boolean) => void; children: ReactNode
}) {
  const [details, setDetails] = useState(true)
  const trigger = useRef<HTMLButtonElement>(null)
  return <Context value={{ active, details, toggleDetails: () => setDetails(value => !value), toolsOpen, onToolsChange, trigger }}>{children}</Context>
}

function WorldWindowGlyph({ shape }: { shape: 'minimize' | 'restore' | 'destinations' | 'close' }) {
  const paths = { minimize: 'M5 12h14', restore: 'M5 12h14M12 5v14', destinations: 'm6 9 6 6 6-6', close: 'm6 6 12 12M18 6 6 18' }
  return <svg className="pi08-window-glyph" viewBox="0 0 24 24" width="24" height="24" aria-hidden="true" focusable="false">
    <path d={paths[shape]} fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="square" strokeLinejoin="miter" />
  </svg>
}

/** Keep the original identity component mounted with its original props. */
export function WorldHudHeaderSlot({ children }: { children: ReactNode }) {
  const hud = useHud()
  return <div className="pi08-header-slot">
    <div className="pi08-identity-slot" hidden={hud.active}>{children}</div>
    {hud.active && <header className="concert-kit pi08-world-title">
      <strong>WORLD</strong>
      <button type="button" onClick={hud.toggleDetails} aria-expanded={hud.details}
        aria-controls="pi08-world-context" aria-label={hud.details ? 'Minimize World details' : 'Show World details'}>
        <WorldWindowGlyph shape={hud.details ? 'minimize' : 'restore'} />
      </button>
      <button ref={hud.trigger} type="button" onClick={() => hud.onToolsChange(!hud.toolsOpen)}
        aria-expanded={hud.toolsOpen} aria-controls="world-tools-destinations pi08-account-shortcuts" aria-label="World destinations">
        <WorldWindowGlyph shape="destinations" />
      </button>
      <ShellLink id="pi08-open-feed" to="/feed" className="pi08-world-close" aria-label="Open Feed">
        <WorldWindowGlyph shape="close" />
      </ShellLink>
    </header>}
  </div>
}

export function WorldHudViewport({ children }: { children: ReactNode }) {
  const hud = useHud()
  return <div className="pi08-viewport" data-world-hud-active={hud.active ? 'true' : 'false'}>
    {children}
    {hud.active && <WorldContextStrip expanded={hud.details} />}
  </div>
}

/** Existing destination JSX and navigation memory remain owned by the shell. */
export function WorldHudToolsSlot({ children }: { children: ReactNode }) {
  const hud = useHud()
  return <div className="pi08-tools-slot" onKeyDownCapture={event => {
    if (event.key !== 'Escape' || !hud.toolsOpen) return
    event.preventDefault(); event.stopPropagation()
    hud.onToolsChange(false); hud.trigger.current?.focus()
  }}>
    {children}
    <nav id="pi08-account-shortcuts" className="pi08-account-shortcuts" aria-label="Account shortcuts" hidden={!hud.toolsOpen}>
      <ShellLink id="shell-inbox-pi08" to="/notifications">Inbox</ShellLink>
      <ShellLink id="shell-settings-pi08" to="/me/settings">Settings</ShellLink>
    </nav>
  </div>
}

/** The caller retains all era state, pending/error handling and selection logic. */
export function WorldEraView({ children, status, statusId, quiet }: {
  children: ReactNode; status: string; statusId: string; quiet: boolean
}) {
  return <section className="concert-kit mcr-chrome-eras pi08-era-controls" aria-label="World era">
    <div className="mcr-chrome-eras__choices" role="group" aria-label="Choose an era" aria-describedby={statusId}>{children}</div>
    <p id={statusId} role="status" aria-live="polite" aria-atomic="true"
      className={quiet ? 'mcr-chrome-sr' : 'mcr-chrome-eras__status'}>{status}</p>
  </section>
}

export function WorldNavigationView({ children }: { children: ReactNode }) {
  return <nav className="concert-kit mcr-chrome-bottom pi08-navigation" aria-label="Main navigation">{children}</nav>
}
