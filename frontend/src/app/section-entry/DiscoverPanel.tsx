/**
 * WP-09 T-901: the Discover section's screens.
 *
 * Discover is the first primary section. It holds two lists that are never
 * merged (product-plan.md §7):
 *
 * * **People** — profiles whose catalog picks overlap with the viewer's;
 * * **Mutuals-intro posts** — a separate list with its own audience filter.
 *
 * Both are owned by `features/discovery` (WP-03). This module only binds the
 * shell's session and transport to them, and it is the *only* place the section
 * decides which list is open — the URL, not local state, is the source of truth,
 * so a link to `/discover/people` and a click on "People" agree.
 *
 * The intro list is a reserved slot: WP-05 owns its implementation. Until that
 * lands, the intro tab renders an explicit placeholder rather than an empty list
 * that would read as "nobody posted".
 */
import { useMemo } from 'react'
import { useLocation } from 'react-router-dom'
import { usePorts } from '../access'
import { Discovery, createDiscoveryController } from '../../features/discovery'
import { IntroSlot } from './IntroSlot'

/** Build a discovery controller for the current session, or `null` without one. */
function useDiscoveryController() {
  const ports = usePorts()
  const auth = ports.auth
  return useMemo(() => {
    if (!auth) return null
    return createDiscoveryController({ auth, request: auth.request })
  }, [auth])
}

/**
 * `/discover` — the section home.
 *
 * It shows the same two-list screen as `/discover/people`, with the intro tab
 * selected when the URL says so. A section home is not a separate product screen;
 * it is the section's first list.
 */
export function DiscoverPanel() {
  return <DiscoveryScreen />
}

/** `/discover/people` — the people list, selected explicitly. */
export function PeoplePanel() {
  return <DiscoveryScreen forceTab="PEOPLE" />
}

function DiscoveryScreen({ forceTab }: { forceTab?: 'PEOPLE' | 'MUTUALS_INTROS' }) {
  const controller = useDiscoveryController()
  const location = useLocation()
  // The URL decides the tab. `/discover/intro*` is the intro list; everything
  // else under the section is the people list.
  const tab = forceTab ?? (location.pathname.startsWith('/discover/intro') ? 'MUTUALS_INTROS' : 'PEOPLE')
  if (!controller) {
    return <section aria-labelledby="discover-unavailable"><h1 id="discover-unavailable">Discover</h1>
      <p role="status">Discover is unavailable until your session is verified.</p></section>
  }
  // The controller's tab is a view state, not a route. Selecting it here keeps a
  // deep link and the in-screen tabs consistent without writing a second router.
  if (controller.read().tab !== tab) controller.selectTab(tab)
  return <Discovery controller={controller}
    mutualsIntros={<IntroSlot view="list" embedded />} />
}
