import { lazy, Suspense, useLayoutEffect, useRef, useState, useSyncExternalStore, type ReactNode } from 'react'
import { Link, Outlet, useLocation, useNavigationType } from 'react-router-dom'
import { Button, Feedback } from '../shared/ui'
import { AccessNotice, scopeKey, useAccess, usePorts, useScopedLoad, validBootstrap } from './access'
import { MAX_SHELL_LOCATIONS, NavigationMemory, ShellLink, usePanelBack } from './navigation'
import type { ActiveScope, ShellBootstrap } from './ports'
import { resolvePanel, type PanelRoute } from './routes'
import { WorldHost } from './WorldHost'
import { BottomNavigation, PanelHeader } from './shell-chrome'
import { ParadeFooter } from './shell-chrome/parade-footer'
import { ConcertWindowEmbedding } from '../shared/ui/concert-window'
import { WorldTools } from './world-presentation/WorldTools'
import { projectCapabilities } from './capabilities'
import type { PrivacyScope } from '../shared/query/privacy'
import { ConnectionStatus } from '../features/connection-status'
import { beginCommunityBootstrap, type CommunityBootstrapBinding } from './community-adoption/bootstrap'

const SessionShellHeader = lazy(() => import('./shell-chrome/identity').then(module => ({ default: module.SessionShellHeader })))
const RootSocialFeatures = lazy(() => import('./world-panels/social').then(module => ({ default: module.RootSocialFeatures })))
const ProductionEraControls = lazy(() => import('./world-panels').then(module => ({ default: module.ProductionEraControls })))
const ProductionWorldFeatures = lazy(() => import('./world-entry/root-features'))
const AccountProvider = lazy(() => import('./account-entry/provider'))
const CharacterProvider = lazy(() => import('./character-entry/provider'))
const RootMediaProvider = lazy(() => import('./media-entry/provider').then(module => ({ default: module.RootMediaProvider })))
const CommunityFeatures = lazy(() => import('./CommunityFeatures'))
const MeetingEntryFeatures = lazy(() => import('./meeting-entry').then(module => ({ default: module.MeetingEntryFeatures })))

export function WorldBoundary() {
  const { scope } = useAccess()
  if (!scope) return <AccessNotice />
  return <BootstrapBoundary key={scopeKey(scope)} scope={scope} />
}
function BootstrapBoundary({ scope }: { scope: ActiveScope }) {
  const ports = usePorts()
  const [attempt, setAttempt] = useState(0)
  const privacy = ports.auth?.privacy
  const policyGeneration = useSyncExternalStore(
    listener => privacy ? privacy.subscribe(listener) : () => {},
    () => privacy?.capture().policyGeneration ?? -1,
  )
  const captured = privacy?.capture() ?? null
  const result = useScopedLoad(scope, async signal => {
    const timing = beginCommunityBootstrap({ privacy, scope: captured, signal, profile: ports.communityClock })
    try {
      const value = privacy && captured
        ? await privacy.run(captured, context => ports.bootstrap.load(scope, context.signal), signal)
        : await ports.bootstrap.load(scope, signal)
      if (!validBootstrap(value, scope)) throw new Error('Invalid bootstrap binding')
      // Keep capabilities attached to the lease that authorized their read. Never
      // relabel an old decision with a newly captured policy generation.
      const bootstrap: ShellBootstrap = Object.freeze({ ...value,
        event: Object.freeze({ ...value.event }), participant: Object.freeze({ ...value.participant }),
        capabilities: projectCapabilities(value.capabilities),
      })
      return { bootstrap, privacyScope: captured, community: timing.complete(value) }
    } catch (error) { timing.dispose(); throw error }
  }, `${attempt}:${policyGeneration}`)
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
  return <WorldShell scope={scope} bootstrap={result.value.bootstrap} privacyScope={result.value.privacyScope} community={result.value.community} />
}

function typingTarget(target: EventTarget | null) {
  return target instanceof HTMLElement && Boolean(target.closest('input,textarea,select,[contenteditable="true"]'))
}
export function WorldShell({ scope, bootstrap, privacyScope = null, community = null }: {
  scope: ActiveScope; bootstrap: ShellBootstrap; privacyScope?: PrivacyScope | null; community?: CommunityBootstrapBinding | null
}) {
  const { auth, accountAssets, connectionStatus } = usePorts()
  const location = useLocation()
  const panelOpen = location.pathname !== '/world'
  const [size, setSize] = useState<'peek' | 'half' | 'full'>(() => window.matchMedia?.('(max-width: 799px)').matches ? 'full' : 'half')
  const [typing, setTyping] = useState(false)
  const [worldToolsOpen, setWorldToolsOpen] = useState(false)
  const root = useRef<HTMLDivElement>(null)
  const positions = useRef(new Map<string, number>())
  const key = scopeKey(scope)
  useLayoutEffect(() => {
    // Removing a focused panel field need not dispatch blur. Read the committed focus.
    setTyping(typingTarget(document.activeElement))
  }, [location.key])
  useLayoutEffect(() => {
    const viewport = window.visualViewport
    const update = () => root.current?.style.setProperty('--shell-viewport-height', `${viewport?.height ?? window.innerHeight}px`)
    update()
    viewport?.addEventListener('resize', update)
    window.addEventListener('resize', update)
    return () => { viewport?.removeEventListener('resize', update); window.removeEventListener('resize', update) }
  }, [])
  return <Suspense fallback={<p role="status">Opening your event…</p>}><CharacterProvider bootstrap={bootstrap} scope={privacyScope}><RootMediaProvider auth={auth} scope={privacyScope}><CommunityFeatures bootstrap={bootstrap} privacyScope={privacyScope} binding={community}><ProductionWorldFeatures privacyScope={privacyScope}><AccountProvider auth={auth} bootstrap={bootstrap} privacyScope={privacyScope} assets={accountAssets}><MeetingEntryFeatures auth={auth ?? null} release={null}>
    <RootSocialFeatures bootstrap={bootstrap} scope={privacyScope}><NavigationMemory>
    {connectionStatus && <ConnectionStatus controller={connectionStatus.controller} reload={connectionStatus.reload} />}
    <div className="world-shell" data-mcr-shell="" data-panel-size={panelOpen ? size : 'closed'} ref={root}
    onFocusCapture={event => setTyping(typingTarget(event.target))}
    onBlurCapture={event => setTyping(typingTarget(event.relatedTarget))}>
    <a className="skip-link" href="#shell-content">Skip to content</a>
    <SessionShellHeader bootstrap={bootstrap} />
    <div className="shell-era-region"><ProductionEraControls />
      {['READ_ONLY', 'ARCHIVED'].includes(bootstrap.event.phase) && <p className="shell-mode" role="status">Read only</p>}
    </div>
    <div className="shell-stage" data-panel-size={panelOpen ? size : 'closed'}>
      <WorldHost key={key} scope={scope} bootstrap={bootstrap} privacyScope={privacyScope} blocked={panelOpen || typing} covered={panelOpen && size === 'full'} />
      {panelOpen ? <PanelFrame size={size} onSize={setSize} positions={positions.current}><Outlet /></PanelFrame>
        : <WorldTools open={worldToolsOpen} onOpenChange={setWorldToolsOpen} />}
    </div>
    <BottomNavigation />
  </div></NavigationMemory></RootSocialFeatures></MeetingEntryFeatures></AccountProvider></ProductionWorldFeatures></CommunityFeatures></RootMediaProvider></CharacterProvider></Suspense>
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
    const path = location.pathname + location.search
    let lastScroll = positions.get(path) ?? 0
    if (element) element.scrollTop = lastScroll
    let lastExtent = element ? element.scrollHeight - element.clientHeight : 0
    const rememberScroll = () => {
      if (element) { lastScroll = element.scrollTop; lastExtent = element.scrollHeight - element.clientHeight }
    }
    element?.addEventListener('scroll', rememberScroll, { passive: true })
    // On POP NavigationMemory restores a known opener; otherwise focus the destination heading.
    heading.current?.focus({ preventScroll: true })
    return () => {
      if (!element) return
      element.removeEventListener('scroll', rememberScroll)
      // A shorter destination can clamp the shared element before layout cleanup.
      positions.delete(path)
      positions.set(path, element.scrollHeight - element.clientHeight < lastExtent ? lastScroll : element.scrollTop)
      if (positions.size > MAX_SHELL_LOCATIONS) positions.delete(positions.keys().next().value!)
    }
  }, [location.key, location.pathname, location.search, navigation])
  // A destination includes its query target. Reopen on navigation, not on a user size change.
  useLayoutEffect(() => { if (window.matchMedia?.('(max-width: 799px)').matches) onSize('full'); else if (size === 'peek') onSize('half') }, [location.key])
  return <main id="shell-content" tabIndex={-1} className="shell-panel" data-mcr-panel={location.pathname === '/wall' ? 'wall' : location.pathname === '/me/avatar' ? 'customize' : location.pathname === '/feed' ? 'feed' : location.pathname === '/gifts' ? 'gifts' : location.pathname === '/me' ? 'me' : undefined} aria-labelledby="panel-title"
    onKeyDown={event => {
      if (event.key === 'Escape' && !event.defaultPrevented && !typingTarget(event.target)) { event.preventDefault(); back() }
    }}>
    <PanelHeader title={title} headingRef={heading} size={size} back={back}
      toggleSize={() => onSize(size === 'full' ? 'half' : 'full')}
      toggleContent={() => onSize(size === 'peek' ? 'half' : 'peek')} />
    <div id="shell-panel-body" className="shell-panel-body" ref={scroll} hidden={size === 'peek'}>
      {['/meet', '/gifts', '/me'].includes(location.pathname)
        ? <div className="mcr-reference-panel-flow"><div className="mcr-reference-panel-content"><ConcertWindowEmbedding>{children}</ConcertWindowEmbedding></div><ParadeFooter /></div>
        : <ConcertWindowEmbedding>{children}</ConcertWindowEmbedding>}
    </div>
  </main>
}

export function PanelScreen({ route, children }: { route: PanelRoute; children?: ReactNode }) {
  const location = useLocation()
  const valid = resolvePanel(location.pathname)?.valid
  const featureOwnsSections = Boolean(children) && ['/feed', '/me'].includes(route.path)
  const sections = route.links && !featureOwnsSections && <nav className="shell-section-links" aria-label={`${route.title} sections`}>
    {route.links.map(link => <ShellLink key={link.path} id={`section-${link.path.replaceAll('/', '-')}`} to={link.path}>{link.label}<span aria-hidden="true">→</span></ShellLink>)}
  </nav>
  const compactSections = Boolean(sections) && ['/meet', '/gifts'].includes(route.path)
  if (!valid) return <Feedback kind="error" title="This link is unavailable">
    <p>It may have changed or you may not have access.</p><ShellLink className="shell-link" to="/world">Back to world</ShellLink>
  </Feedback>
  return <>
    {compactSections && <details className="shell-more-sections"><summary>More actions</summary>{sections}</details>}
    {children ?? <div className="shell-unavailable"><p className="shell-kicker">Still taking shape</p>
      <h2>{route.title} is not available yet</h2><p>This part of the event is still being prepared. You can browse the other sections.</p></div>}
    {!compactSections && sections}
  </>
}
