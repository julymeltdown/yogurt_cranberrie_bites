import { useLayoutEffect, useRef, useState, type ReactNode } from 'react'
import { Link, Outlet, useLocation, useNavigationType } from 'react-router-dom'
import { Button, Feedback } from '../shared/ui'
import { AccessNotice, scopeKey, useAccess, usePorts, useScopedLoad, validBootstrap } from './access'
import { MAX_SHELL_LOCATIONS, NavigationMemory, ShellLink, usePanelBack } from './navigation'
import type { ActiveScope, ShellBootstrap } from './ports'
import { resolvePanel, tabs, type PanelRoute } from './routes'
import { WorldHost } from './WorldHost'

export function WorldBoundary() {
  const { scope } = useAccess()
  if (!scope) return <AccessNotice />
  return <BootstrapBoundary key={scopeKey(scope)} scope={scope} />
}
function BootstrapBoundary({ scope }: { scope: ActiveScope }) {
  const ports = usePorts()
  const [attempt, setAttempt] = useState(0)
  const result = useScopedLoad(scope, async signal => {
    const value = await ports.bootstrap.load(scope, signal)
    if (!validBootstrap(value, scope)) throw new Error('Invalid bootstrap binding')
    return value
  }, attempt)
  if (result.status !== 'ready') return <div className="shell-gate">
    <Link className="shell-link" to="/">MCR Concert World</Link>
    <h1 tabIndex={-1}>{result.status === 'loading' ? 'Opening your event' : {
      unauthenticated: 'Sign in to continue', denied: 'Access denied', 'not-found': 'Event not found',
      gone: 'This event is no longer available', 'rate-limited': 'Please wait before trying again', unavailable: 'Your event is unavailable',
    }[result.reason]}</h1>
    <Feedback kind={result.status === 'loading' ? 'status' : 'error'} title="Your destination is saved">
      <p>{result.status === 'loading' ? 'Checking the event before opening your link.' : 'We could not confirm access. No private content has been opened.'}</p>
    </Feedback>
    <div className="shell-actions">
      {result.status === 'error' && <Button onClick={() => setAttempt(n => n + 1)}>Try again</Button>}
      <Link className="shell-link" to="/">Back to home</Link>
    </div>
  </div>
  return <WorldShell scope={scope} bootstrap={result.value} />
}

function typingTarget(target: EventTarget | null) {
  return target instanceof HTMLElement && Boolean(target.closest('input,textarea,select,[contenteditable="true"]'))
}
export function WorldShell({ scope, bootstrap }: { scope: ActiveScope; bootstrap: ShellBootstrap }) {
  const location = useLocation()
  const route = resolvePanel(location.pathname)
  const panelOpen = location.pathname !== '/world'
  const [size, setSize] = useState<'peek' | 'half' | 'full'>('half')
  const [typing, setTyping] = useState(false)
  const root = useRef<HTMLDivElement>(null)
  const positions = useRef(new Map<string, number>())
  const key = scopeKey(scope)
  useLayoutEffect(() => {
    const viewport = window.visualViewport
    const update = () => root.current?.style.setProperty('--shell-viewport-height', `${viewport?.height ?? window.innerHeight}px`)
    update()
    viewport?.addEventListener('resize', update)
    window.addEventListener('resize', update)
    return () => { viewport?.removeEventListener('resize', update); window.removeEventListener('resize', update) }
  }, [])
  return <NavigationMemory><div className="world-shell" data-mcr-shell="" ref={root}
    onFocusCapture={event => setTyping(typingTarget(event.target))}
    onBlurCapture={event => setTyping(typingTarget(event.relatedTarget))}>
    <a className="skip-link" href="#shell-content">Skip to content</a>
    <header className="shell-header">
      <div><Link to="/" className="shell-brand" aria-label="MCR Concert World home">MCR <span>Concert World</span></Link>
        <p className="shell-event">{bootstrap.event.title}</p></div>
      <ShellLink to="/notifications" className="shell-link" id="shell-inbox">Inbox</ShellLink>
    </header>
    <div className="shell-era"><ShellLink to="/world/theme" id="shell-era">Revenge / Parade / Danger <span aria-hidden="true">↗</span></ShellLink>
      {['READ_ONLY', 'ARCHIVED'].includes(bootstrap.event.phase) && <span>Read only</span>}</div>
    <div className="shell-stage" data-panel-size={panelOpen ? size : 'closed'}>
      <WorldHost key={key} scope={scope} bootstrap={bootstrap} blocked={panelOpen || typing} covered={panelOpen && size === 'full'} />
      {panelOpen ? <PanelFrame size={size} onSize={setSize} positions={positions.current}><Outlet /></PanelFrame>
        : <main id="shell-content" className="shell-world-tools" tabIndex={-1}>
          <h1 className="mcr-visually-hidden">World</h1>
          <nav aria-label="World tools"><ShellLink id="world-map" to="/world/map">Map</ShellLink>
            <ShellLink id="world-rooms" to="/world/rooms">Rooms</ShellLink>
            <ShellLink id="world-nearby" to="/world/nearby">Nearby</ShellLink>
            <ShellLink id="world-help" to="/world/help">Help</ShellLink></nav>
        </main>}
    </div>
    <nav className="shell-tabs" aria-label="Main navigation">
      {tabs.map(tab => <ShellLink key={tab.path} id={`tab-${tab.label.toLowerCase()}`} to={tab.path}
        aria-current={route?.route.tab === tab.path ? 'page' : undefined}>{tab.label}</ShellLink>)}
    </nav>
  </div></NavigationMemory>
}

function PanelFrame({ size, onSize, positions, children }: { size: 'peek' | 'half' | 'full'; onSize: (size: 'peek' | 'half' | 'full') => void; positions: Map<string, number>; children: ReactNode }) {
  const location = useLocation()
  const navigation = useNavigationType()
  const back = usePanelBack()
  const title = resolvePanel(location.pathname)?.route.title ?? 'Unavailable'
  const heading = useRef<HTMLHeadingElement>(null)
  const scroll = useRef<HTMLDivElement>(null)
  useLayoutEffect(() => {
    const element = scroll.current
    if (element) element.scrollTop = positions.get(location.pathname + location.search) ?? 0
    // On POP NavigationMemory restores a known opener; otherwise focus the destination heading.
    heading.current?.focus({ preventScroll: true })
    return () => {
      if (!element) return
      const path = location.pathname + location.search
      positions.delete(path)
      positions.set(path, element.scrollTop)
      if (positions.size > MAX_SHELL_LOCATIONS) positions.delete(positions.keys().next().value!)
    }
  }, [location.key, location.pathname, location.search, navigation])
  // A destination includes its query target. Reopen on navigation, not on a user size change.
  useLayoutEffect(() => { if (size === 'peek') onSize('half') }, [location.key])
  return <main id="shell-content" tabIndex={-1} className="shell-panel" aria-labelledby="panel-title"
    onKeyDown={event => {
      if (event.key === 'Escape' && !event.defaultPrevented && !typingTarget(event.target)) { event.preventDefault(); back() }
    }}>
    <header className="shell-panel-header">
      <Button variant="quiet" onClick={back} aria-label="Back to previous panel">← Back</Button>
      <h1 id="panel-title" ref={heading} tabIndex={-1}>{title}</h1>
      <Button variant="quiet" onClick={() => onSize(size === 'full' ? 'half' : 'full')}
        aria-label={size === 'full' ? 'Reduce panel' : 'Expand panel'} aria-expanded={size === 'full'}>{size === 'full' ? 'Reduce' : 'Expand'}</Button>
      <Button variant="quiet" onClick={() => onSize(size === 'peek' ? 'half' : 'peek')}
        aria-label={size === 'peek' ? 'Show panel content' : 'Minimize panel'} aria-expanded={size !== 'peek'}>{size === 'peek' ? 'Show' : 'Hide'}</Button>
    </header>
    <div className="shell-panel-body" ref={scroll} hidden={size === 'peek'}>{children}</div>
  </main>
}

export function PanelScreen({ route, children }: { route: PanelRoute; children?: ReactNode }) {
  const location = useLocation()
  const valid = resolvePanel(location.pathname)?.valid
  if (!valid) return <Feedback kind="error" title="This link is unavailable">
    <p>It may have changed or you may not have access.</p><ShellLink className="shell-link" to="/world">Back to world</ShellLink>
  </Feedback>
  return <>
    {children ?? <div className="shell-unavailable"><p className="shell-kicker">Still taking shape</p>
      <h2>{route.title} is not available yet</h2><p>This part of the event is still being prepared. You can browse the other sections.</p></div>}
    {route.links && <nav className="shell-section-links" aria-label={`${route.title} sections`}>
      {route.links.map(link => <ShellLink key={link.path} id={`section-${link.path.replaceAll('/', '-')}`} to={link.path}>{link.label}<span aria-hidden="true">→</span></ShellLink>)}
    </nav>}
  </>
}
