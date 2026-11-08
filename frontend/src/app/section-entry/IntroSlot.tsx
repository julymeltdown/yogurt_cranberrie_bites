/**
 * WP-09 T-904: the reserved slot for the mutuals-intro screens.
 *
 * WP-05 owns the intro composer and the intro detail screens. This package
 * registers their routes, links to them, and marks the slot — it does not
 * implement them and does not depend on their internals, so the two packages can
 * merge in either order.
 *
 * What the slot must not do is pretend. An empty list would read as "nobody
 * posted a mutuals-intro", which is a claim about other people that this package
 * cannot make. The slot therefore says exactly what is true: the screens are not
 * part of this build yet, and the rest of Discover still works.
 *
 * When WP-05 lands, it replaces the body of this module with its own screens and
 * keeps the same two exports, so the router registration does not change.
 */

export interface IntroSlotProps {
  readonly view: 'list' | 'detail'
  /** Rendered inside the Discover screen's own tab panel rather than as a page. */
  readonly embedded?: boolean
}

export function IntroSlot({ view, embedded = false }: IntroSlotProps) {
  const heading = view === 'list' ? 'Mutuals-intro posts' : 'Mutuals-intro post'
  const body = <section className="intro-slot" aria-labelledby="intro-slot-title">
    <h2 id="intro-slot-title">{heading}</h2>
    <p role="status">
      Mutuals-intro posts are not part of this build yet. People, your profile, your photos and
      your room all work without them.
    </p>
  </section>
  return embedded ? body : <div className="intro-slot-page">{body}</div>
}
