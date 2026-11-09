/**
 * The mutuals-intro composer screen, mounted from the shell.
 *
 * WP-05 owns the composer (`features/intros/IntroComposer`) and its controller.
 * This panel is the shell's composition seam: it builds the controller from the
 * ports the shell already created and renders WP-05's screen. It adds no product
 * behaviour and no styling of its own.
 *
 * ## Why the composer is safe to mount and the discovery list is not
 *
 * The composer's operations are `saveIntroPostDraft`, `publishIntroPost` and
 * `deleteIntroPost`, plus `getOwnIntroPost`. All four are served by the
 * concert-hosted profile transport (WP-05's stores exist for every one of them),
 * so this screen works end to end.
 *
 * The discovery list (`listIntroPosts`) and the post detail (`getIntroPost`) are
 * **not** served: they need a visibility-filtered query that does not exist yet.
 * Those routes stay on the explicit placeholder rather than mounting a screen
 * that would fail on every load. See
 * `docs/mcr/integration/profile-producer-coverage.md` §3.
 *
 * ## The visibility policy is not this panel's to choose
 *
 * WP-05's stores run with the fail-closed `deny_restricted` policy until WP-04
 * installs the real one through `IntroPostStore::with_visibility`. That swap is
 * owned elsewhere; this panel does not touch it, and the composer's own
 * operations are author-scoped so the policy does not change their behaviour.
 */
import { useEffect, useId, useMemo } from 'react'
import { usePorts } from '../access'
import { createIntroComposerController } from '../../features/intros/controller'
import { IntroComposer } from '../../features/intros/IntroComposer'
import './section-entry.css'

/**
 * `/discover/intro` — the mutuals-intro composer.
 *
 * The controller is created once per privacy scope and disposed when the scope
 * changes, so a draft from one account can never be saved under another. The
 * component unmounts the composer entirely when the session is not usable,
 * rather than rendering a form whose submit would fail.
 */
export function IntroComposerPanel() {
  const ports = usePorts()
  const id = useId()
  const auth = ports.auth
  const scope = auth?.privacy.capture() ?? null
  const usable = auth !== undefined && scope !== null
    && auth.privacy.isCurrent(scope) && scope.userId !== null

  // The controller is keyed by the privacy scope. A new scope is a new
  // controller, which is what stops a draft crossing accounts.
  const controller = useMemo(() => {
    if (!auth || !usable) return null
    return createIntroComposerController({ auth: { privacy: auth.privacy }, request: auth.request })
  }, [auth, usable, scope?.userId, scope?.generation, scope?.policyGeneration])

  useEffect(() => () => { controller?.dispose() }, [controller])

  if (!controller) {
    return <section className="section-page" aria-labelledby={`${id}-title`}>
      <header className="section-header"><h1 id={`${id}-title`}>Your introduction</h1></header>
      <p role="status">Your introduction is unavailable until your session is verified.</p>
    </section>
  }
  return <section className="section-page" aria-labelledby={`${id}-title`}>
    <header className="section-header"><h1 id={`${id}-title`}>Your introduction</h1></header>
    <IntroComposer controller={controller} />
  </section>
}

/**
 * The intro screens that are not mounted, and why.
 *
 * Exported so a test can assert the placeholder is still deliberate rather than
 * an oversight, and so the coverage document and the code cannot drift apart.
 */
export const INTRO_UNAVAILABLE = Object.freeze({
  list: 'listIntroPosts needs a visibility-filtered page query (WP-04 predicate + WP-05 query).',
  detail: 'getIntroPost needs a visibility-filtered detail query (WP-04 predicate + WP-05 query).',
})
