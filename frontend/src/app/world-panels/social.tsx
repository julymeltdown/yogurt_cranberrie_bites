import { useCallback, useMemo, type ReactNode } from 'react'
import { useNavigate } from 'react-router-dom'
import { SocialFeatures, SocialNavigationBoundary } from '../social-composition'
import type { SocialAvailabilityPort, SocialContext, SocialDestination, SocialOptions } from '../social-composition/ports'
import type { PrivacyScope } from '../../shared/query/privacy'
import type { ShellBootstrap } from '../ports'
import './social.css'
import { usePorts } from '../access'
import { useWorldPanelOwnership } from './ownership'
/** Explicit application producer configuration; C03 capability alone never asserts live service readiness. */
export interface SocialProductionPorts {
  /** Pure projection factory; reads/observers belong to the returned port, not render-time side effects. */
  availability(context: SocialContext): SocialAvailabilityPort | null
  readonly requests?: SocialOptions['requests']
  readonly media?: SocialOptions['media']
  readonly avatars?: SocialOptions['avatars']
  readonly verification?: SocialOptions['verification']
}
const unavailable: SocialAvailabilityPort = { read: () => null, subscribe: () => () => {} }
export function RootSocialFeatures({ bootstrap, scope, children }: { bootstrap: ShellBootstrap; scope: PrivacyScope | null; children: ReactNode }) {
  const ports = usePorts(), navigate = useNavigate(), ownership = useWorldPanelOwnership()
  const { auth, social } = ports
  const availability = useMemo(() => {
    if (!auth || !scope || !auth.privacy.isCurrent(scope)) return unavailable
    try { return social?.availability({ auth, bootstrap, scope }) ?? unavailable } catch { return unavailable }
  }, [auth, bootstrap, scope, social])
  const navigateSocial = useCallback((destination: SocialDestination) => {
    if (auth && scope && destination.scope.userId === scope.userId && destination.scope.eventId === scope.eventId
      && destination.scope.generation === scope.generation && destination.scope.policyGeneration === scope.policyGeneration && destination.scope.gate === scope.gate
      && auth.privacy.isCurrent(scope)) void navigate(destination.path)
  }, [auth, scope, navigate])
  if (!ownership.social || !auth || !scope) return children
  return <div className="mcr-root-social"><SocialFeatures auth={auth} bootstrap={bootstrap} scope={scope} availability={availability} navigate={navigateSocial}
    {...(social?.requests ? { requests: social.requests } : {})} {...(social?.media ? { media: social.media } : {})}
    {...(social?.avatars ? { avatars: social.avatars } : {})} {...(social?.verification ? { verification: social.verification } : {})}>
    <SocialNavigationBoundary>{children}</SocialNavigationBoundary>
  </SocialFeatures></div>
}
