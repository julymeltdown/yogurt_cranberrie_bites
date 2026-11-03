import { useId, useSyncExternalStore, type Ref } from 'react'
import { useLocation } from 'react-router-dom'
import { ShellLink } from '../navigation'
import { resolvePanel, tabs, type TabPath } from '../routes'
import { ConcertIcon, type ConcertIconName } from '../../shared/ui/concert-window'
import type { ThemeController, ThemeState } from '../../features/theme/model/controller'
import { themeIds, type ThemeId } from '../../features/theme/model/preferences'
import './shell-chrome.css'

/** Caller supplies only current authorized event text. No identity/art fallback. */
export function ShellHeader({ eventTitle }: { eventTitle?: string }) {
  return <header className="concert-kit mcr-chrome-header">
    <ShellLink to="/world" className="mcr-chrome-brand" id="shell-brand" aria-label="MCR Concert World">
      <span>MCR <small>Concert World</small></span>
      {eventTitle && <span className="mcr-chrome-event" title={eventTitle}>{eventTitle}</span>}
    </ShellLink>
    <div className="mcr-chrome-header__actions">
      <ShellLink to="/notifications" id="shell-inbox" aria-label="Inbox"><ConcertIcon name="feed" /></ShellLink>
      <ShellLink to="/me/settings" id="shell-settings" aria-label="Settings">
        <svg className="concert-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" aria-hidden="true" focusable="false">
          <path d="m9 3 1 3h4l1-3 3 2-1 3 2 3 3 1-1 4-3 0-2 3 0 3h-4l-1-3-3-1-3 1-2-3 2-2V9L2 7l3-3 3 2Z" /><circle cx="12" cy="12" r="3" />
        </svg>
      </ShellLink>
    </div>
  </header>
}

export type EraPort = Pick<ThemeController, 'read' | 'subscribe' | 'select'>
const absent: ThemeState | null = null
const readAbsent = () => absent
const subscribeAbsent = () => () => {}
const label = (theme: ThemeId) => theme[0] + theme.slice(1).toLowerCase()

/** Observe an existing shell-owned controller; never create or dispose it here.
 * Parent withdraws the port on authority loss. No local async/optimistic selection.
 */
export function EraControls({ controller = null }: { controller?: EraPort | null }) {
  const id = useId()
  const state = useSyncExternalStore(controller?.subscribe ?? subscribeAbsent, controller?.read ?? readAbsent, readAbsent)
  const available = !!controller && !!state && state.phase !== 'disposed'
  const pending = available && state.phase === 'loading' ? state.pending : null
  const status = !available ? 'Eras unavailable.' : pending ? `Loading ${label(pending)}. ${label(state.selected)} is still selected.`
    : state.phase === 'error' ? `${label(state.selected)} is selected. The era could not be loaded or saved. Choose an era to try again.`
    : `${label(state.selected)} selected.`
  function select(theme: ThemeId) {
    if (!controller || !available) return
    // The existing controller publishes its safe state and owns failure, atomic
    // rollback and persistence. A rejected external port never leaks raw text.
    try { void controller.select(theme).catch(() => {}) } catch { /* Owner must publish failure through its state port. */ }
  }
  return <section className="concert-kit mcr-chrome-eras" aria-label="World era">
    <div className="mcr-chrome-eras__choices" role="group" aria-label="Choose an era" aria-describedby={`${id}-status`}>
      {themeIds.map(theme => <button key={theme} type="button" disabled={!available} onClick={() => select(theme)}
        aria-pressed={available && state.selected === theme} data-pending={pending === theme ? 'true' : undefined}
        aria-label={`${label(theme)}${pending === theme ? ', loading' : ''}`}>
        {theme}<span className="mcr-chrome-eras__pending" aria-hidden="true">{pending === theme ? '…' : ''}</span>
      </button>)}
    </div>
    <p id={`${id}-status`} role="status" aria-live="polite" aria-atomic="true"
      className={available && state.phase === 'ready' ? 'mcr-chrome-sr' : 'mcr-chrome-eras__status'}>{status}</p>
  </section>
}

const icons: Record<TabPath, ConcertIconName> = { '/world': 'world', '/feed': 'feed', '/meet': 'meet', '/gifts': 'gifts', '/wall': 'wall', '/me': 'me' }
/** Uses the incumbent router and caller's NavigationMemory; no plain-anchor reload. */
export function BottomNavigation() {
  const location = useLocation(), resolved = resolvePanel(location.pathname)
  return <nav className="concert-kit mcr-chrome-bottom" aria-label="Main navigation">
    {tabs.map(tab => <ShellLink key={tab.path} to={tab.path} id={`tab-${tab.label.toLowerCase()}`}
      aria-current={resolved?.valid && resolved.route.tab === tab.path ? 'page' : undefined}>
      <ConcertIcon name={icons[tab.path]} /><span>{tab.label}</span>
    </ShellLink>)}
  </nav>
}

/** A replacement for the existing header only. Parent retains Back/Escape,
 * heading focus, scroll restoration, sizing, visibility and panel lifetime.
 */
export function PanelHeader({ title, headingRef, size, back, toggleSize, toggleContent }: {
  title: string; headingRef?: Ref<HTMLHeadingElement>; size: 'peek' | 'half' | 'full'
  back: () => void; toggleSize: () => void; toggleContent: () => void
}) {
  return <header className="concert-kit mcr-chrome-panel-header">
    <button type="button" onClick={back} aria-label="Back to previous panel"><span aria-hidden="true">←</span></button>
    <h1 id="panel-title" ref={headingRef} tabIndex={-1}>{title}</h1>
    <button type="button" onClick={toggleSize} aria-label={size === 'full' ? 'Reduce panel' : 'Expand panel'} aria-expanded={size === 'full'}><span aria-hidden="true">{size === 'full' ? '−' : '+'}</span></button>
    <button type="button" onClick={toggleContent} aria-label={size === 'peek' ? 'Show panel content' : 'Minimize panel'} aria-expanded={size !== 'peek'}><span aria-hidden="true">{size === 'peek' ? '▴' : '▾'}</span></button>
  </header>
}
