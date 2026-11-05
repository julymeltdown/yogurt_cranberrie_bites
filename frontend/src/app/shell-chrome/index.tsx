import { useId, useSyncExternalStore, type ReactNode, type Ref } from 'react'
import { useLocation } from 'react-router-dom'
import { ShellLink } from '../navigation'
import { resolvePanel, tabs, type TabPath } from '../routes'
import { ConcertIcon, type ConcertIconName } from '../../shared/ui/concert-window'
import type { ThemeController, ThemeState } from '../../features/theme/model/controller'
import { themeIds, type ThemeId } from '../../features/theme/model/preferences'
import { WindowInk } from '../reference-chrome-v2/WindowInk'
import { WorldEraView, WorldNavigationView } from '../../features/parade-ink/world-hud'
import './shell-chrome.css'
import '../reference-chrome-v2/chrome.css'

/** Current identity text and approved visual only; no fabricated level, balances or presence. */
export function ShellHeader({ eventTitle, displayName, avatar }: { eventTitle?: string; displayName?: string; avatar?: ReactNode }) {
  return <header className="concert-kit mcr-chrome-header">
    <ShellLink to="/world" className="mcr-chrome-brand" id="shell-brand" aria-label="MCR Concert World">
      <span className="mcr-chrome-identity-visual" aria-hidden="true">{avatar ?? <ConcertIcon name="star" />}</span>
      <span className="mcr-chrome-identity-copy"><strong>{displayName || 'MCR Concert World'}</strong>
        {eventTitle && <span className="mcr-chrome-event" title={eventTitle}>{eventTitle}</span>}
      </span>
    </ShellLink>
    <div className="mcr-chrome-header__actions">
      <ShellLink to="/notifications" id="shell-inbox" aria-label="Inbox"><ConcertIcon name="bell" /></ShellLink>
      <ShellLink to="/me/settings" id="shell-settings" aria-label="Settings"><ConcertIcon name="settings" /></ShellLink>
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
  return <WorldEraView statusId={`${id}-status`} status={status} quiet={available && state.phase === 'ready'}>
      {themeIds.map(theme => <button key={theme} type="button" disabled={!available} onClick={() => select(theme)}
        aria-pressed={available && state.selected === theme} data-pending={pending === theme ? 'true' : undefined}
        aria-label={`${label(theme)}${pending === theme ? ', loading' : ''}`}>
        {theme}<span className="mcr-chrome-eras__pending" aria-hidden="true">{pending === theme ? '…' : ''}</span>
      </button>)}
  </WorldEraView>
}

const icons: Record<TabPath, ConcertIconName> = { '/world': 'world', '/feed': 'feed', '/meet': 'meet', '/gifts': 'gifts', '/wall': 'wall', '/me': 'me' }
/** Uses the incumbent router and caller's NavigationMemory; no plain-anchor reload. */
export function BottomNavigation() {
  const location = useLocation(), resolved = resolvePanel(location.pathname)
  return <WorldNavigationView>
    {tabs.map(tab => <ShellLink key={tab.path} to={tab.path} id={`tab-${tab.label.toLowerCase()}`}
      aria-current={resolved?.valid && resolved.route.tab === tab.path ? 'page' : undefined}>
      <ConcertIcon name={icons[tab.path]} /><span>{tab.label}</span>
    </ShellLink>)}
  </WorldNavigationView>
}

/** A replacement for the existing header only. Parent retains Back/Escape,
 * heading focus, scroll restoration, sizing, visibility and panel lifetime.
 */
export function PanelHeader({ title, headingRef, size, back, toggleSize, toggleContent }: {
  title: string; headingRef?: Ref<HTMLHeadingElement>; size: 'peek' | 'half' | 'full'
  back: () => void; toggleSize: () => void; toggleContent: () => void
}) {
  return <header className="concert-kit mcr-chrome-panel-header">
    <h1 id="panel-title" ref={headingRef} tabIndex={-1}>{title}</h1>
    <button type="button" onClick={toggleSize} aria-label={size === 'full' ? 'Reduce panel' : 'Expand panel'} aria-expanded={size === 'full'}><WindowInk name={size === 'full' ? 'reduce' : 'expand'} /></button>
    <button type="button" onClick={toggleContent} aria-label={size === 'peek' ? 'Show panel content' : 'Minimize panel'} aria-expanded={size !== 'peek'} aria-controls="shell-panel-body"><WindowInk name="back" className={size === 'peek' ? 'mcr-reference-window-ink--restore' : 'mcr-reference-window-ink--collapse'} /></button>
    <button className="mcr-chrome-panel-close" type="button" onClick={back} aria-label="Back to previous panel"><WindowInk name="close" /></button>
  </header>
}
