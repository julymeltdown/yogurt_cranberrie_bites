import { useEffect, useId, useSyncExternalStore } from 'react'
import { Link } from 'react-router-dom'
import type { DiscoveryController } from './controller'
import {
  PEOPLE_CATEGORIES,
  PEOPLE_CATEGORY_LABEL,
  sharedInterestLine,
  type PeopleCard,
} from './contract'
import './discovery.css'

/**
 * WP-03 T-805: the discovery screen.
 *
 * Two tabs, never merged:
 *
 * * **People** — profiles whose catalog picks overlap with the viewer's. A card
 *   shows the avatar, nickname, the names of the actual shared items, an
 *   optional interaction-style line, an open-profile action and an optional
 *   follow action.
 * * **Mutuals-intro posts** — a separate list of post cards with their own
 *   heart button. It is rendered by its own component with its own audience
 *   filter; nothing here converts one list into the other.
 *
 * Two rules are visible in the markup rather than only in the data:
 *
 * * the shared-interest line is **omitted** when there is no overlap — there is
 *   no empty paragraph and no generic "you might get along" claim;
 * * a discovery card offers **no room preview and no room action**. Opening the
 *   room is a separate action inside the profile.
 */
export function Discovery({
  controller,
  mutualsIntros,
}: {
  readonly controller: DiscoveryController
  /** The separate mutuals-intro list. Rendered as its own tab, never merged. */
  readonly mutualsIntros?: React.ReactNode
}) {
  const state = useSyncExternalStore(controller.subscribe, controller.read, controller.read)
  const id = useId()
  useEffect(() => {
    controller.start()
  }, [controller])
  return (
    <section className="discovery-surface" aria-labelledby={`${id}-title`}>
      <header>
        <p>FIND PEOPLE</p>
        <h1 id={`${id}-title`}>Discover</h1>
      </header>
      <div className="discovery-tabs" role="tablist" aria-label="Discovery lists">
        <button
          type="button"
          role="tab"
          id={`${id}-tab-people`}
          aria-selected={state.tab === 'PEOPLE'}
          aria-controls={`${id}-panel-people`}
          tabIndex={state.tab === 'PEOPLE' ? 0 : -1}
          className={state.tab === 'PEOPLE' ? 'discovery-tab discovery-tab-active' : 'discovery-tab'}
          onClick={() => controller.selectTab('PEOPLE')}
        >
          People
        </button>
        <button
          type="button"
          role="tab"
          id={`${id}-tab-intros`}
          aria-selected={state.tab === 'MUTUALS_INTROS'}
          aria-controls={`${id}-panel-intros`}
          tabIndex={state.tab === 'MUTUALS_INTROS' ? 0 : -1}
          className={state.tab === 'MUTUALS_INTROS' ? 'discovery-tab discovery-tab-active' : 'discovery-tab'}
          onClick={() => controller.selectTab('MUTUALS_INTROS')}
        >
          Mutuals intro posts
        </button>
      </div>
      {state.tab === 'PEOPLE' ? (
        <div className="discovery-panel" role="tabpanel" id={`${id}-panel-people`} aria-labelledby={`${id}-tab-people`}>
          <PeoplePanel controller={controller} id={id} />
        </div>
      ) : (
        <div className="discovery-panel" role="tabpanel" id={`${id}-panel-intros`} aria-labelledby={`${id}-tab-intros`}>
          {mutualsIntros ?? <p role="status">Mutuals intro posts are not available here.</p>}
        </div>
      )}
    </section>
  )
}

function PeoplePanel({ controller, id }: { readonly controller: DiscoveryController; readonly id: string }) {
  const state = useSyncExternalStore(controller.subscribe, controller.read, controller.read)
  const people = state.people
  return (
    <>
      <div className="discovery-categories" role="tablist" aria-label="Shared interest categories">
        {PEOPLE_CATEGORIES.map((category) => (
          <button
            key={category}
            type="button"
            role="tab"
            aria-selected={state.category === category}
            tabIndex={state.category === category ? 0 : -1}
            className={state.category === category ? 'discovery-category discovery-category-active' : 'discovery-category'}
            onClick={() => controller.selectCategory(category)}
          >
            {PEOPLE_CATEGORY_LABEL[category]}
          </button>
        ))}
      </div>
      {!state.available ? (
        <p role="status">People are unavailable for this session.</p>
      ) : people.phase === 'loading' && people.data === null ? (
        <p role="status">Finding people…</p>
      ) : people.phase === 'error' ? (
        <div role="status">
          <p>{state.message}</p>
          <button type="button" onClick={() => void controller.load()}>
            Retry
          </button>
        </div>
      ) : people.phase === 'ready' && (people.data ?? []).length === 0 ? (
        <p role="status">
          Nobody shares your {PEOPLE_CATEGORY_LABEL[state.category].toLowerCase()} picks yet. Add more
          favorites or check another category.
        </p>
      ) : null}
      <ul className="discovery-people" aria-label="People">
        {(people.data ?? []).map((card) => (
          <li key={card.profile_id}>
            <PeopleCardView card={card} controller={controller} id={id} />
          </li>
        ))}
      </ul>
      {state.nextCursor !== null && (
        <button type="button" disabled={people.phase === 'loading'} onClick={() => void controller.more()}>
          More people
        </button>
      )}
      {state.message !== '' && people.phase !== 'error' && (
        <p role="alert">{state.message}</p>
      )}
    </>
  )
}

function PeopleCardView({
  card,
  controller,
  id,
}: {
  readonly card: PeopleCard
  readonly controller: DiscoveryController
  readonly id: string
}) {
  const state = useSyncExternalStore(controller.subscribe, controller.read, controller.read)
  // The shared-interest line is omitted entirely when there is no overlap. The
  // card cannot claim a shared interest it cannot name.
  const shared = sharedInterestLine(card.shared_items)
  const following = state.following.includes(card.profile_id)
  return (
    <article className="discovery-card" aria-labelledby={`${id}-name-${card.profile_id}`}>
      <div className="discovery-avatar" aria-hidden="true">
        {card.profile_image_media_id === null ? card.nickname.slice(0, 1).toUpperCase() : null}
      </div>
      <div className="discovery-card-body">
        <h2 id={`${id}-name-${card.profile_id}`}>{card.nickname}</h2>
        {shared !== null && <p className="discovery-shared">{shared}</p>}
        {card.interaction_style !== null && (
          <p className="discovery-interaction">{card.interaction_style}</p>
        )}
        <div className="discovery-actions">
          <Link to={`/profiles/${card.profile_id}`}>Open profile</Link>
          <button
            type="button"
            aria-pressed={following}
            disabled={following}
            onClick={() => void controller.follow(card)}
          >
            {following ? 'Following' : 'Follow'}
          </button>
        </div>
      </div>
    </article>
  )
}
