import { useEffect, useId, useSyncExternalStore } from 'react'
import type { FavoritesController } from './controller'
import {
  CATEGORIES,
  CATEGORY_LABEL,
  MAX_PER_CATEGORY,
  type Category,
} from './contract'
import { catalogExternalLinkLabel, catalogExternalUrl, catalogSubtitle } from './catalog-provider'
import './favorites.css'

/**
 * WP-03 T-804: the favorite picker.
 *
 * Three tabs, in the product's order: bands and artists, anime, games. Search
 * and select up to five per category. There is no intensity, importance or
 * preference control anywhere, because the destination has no such concept.
 *
 * Skipping every category is allowed. This component never disables its save
 * action for an empty selection and never asks the user to pick at least one;
 * the copy says so explicitly, because a first-time user who does not care
 * about music should not be made to feel the product is broken.
 */
export function FavoritePicker({ controller }: { readonly controller: FavoritesController }) {
  const state = useSyncExternalStore(controller.subscribe, controller.read, controller.read)
  const id = useId()
  useEffect(() => {
    controller.start()
  }, [controller])
  const picks = state.selection
  const total = picks.music.length + picks.anime.length + picks.games.length
  return (
    <section className="favorites-picker" aria-labelledby={`${id}-title`}>
      <header>
        <h1 id={`${id}-title`}>Your favorites</h1>
        <p>
          Pick up to {MAX_PER_CATEGORY} in each category. Nothing here is required — you can skip
          every category and still use your profile, photos and room.
        </p>
      </header>
      <div className="favorites-tabs" role="tablist" aria-label="Favorite categories">
        {CATEGORIES.map((category) => (
          <button
            key={category}
            type="button"
            role="tab"
            id={`${id}-tab-${category}`}
            aria-selected={state.category === category}
            aria-controls={`${id}-panel`}
            tabIndex={state.category === category ? 0 : -1}
            className={state.category === category ? 'favorites-tab favorites-tab-active' : 'favorites-tab'}
            onClick={() => controller.selectCategory(category)}
          >
            {CATEGORY_LABEL[category]}
            <span className="favorites-tab-count" aria-hidden="true">
              {picks[category === 'MUSIC' ? 'music' : category === 'ANIME' ? 'anime' : 'games'].length}
            </span>
          </button>
        ))}
      </div>
      <div
        className="favorites-panel"
        role="tabpanel"
        id={`${id}-panel`}
        aria-labelledby={`${id}-tab-${state.category}`}
      >
        <form
          onSubmit={(event) => {
            event.preventDefault()
            void controller.submit()
          }}
        >
          <label htmlFor={`${id}-search`}>Search {CATEGORY_LABEL[state.category].toLowerCase()}</label>
          <div className="favorites-search">
            <input
              id={`${id}-search`}
              type="search"
              role="searchbox"
              autoComplete="off"
              maxLength={80}
              value={state.term}
              onChange={(event) => controller.setTerm(event.target.value)}
            />
            <button type="submit" disabled={!state.available || state.term.trim().length === 0}>
              Search
            </button>
          </div>
        </form>
        <SearchResults controller={controller} category={state.category} />
        <PickedItems controller={controller} category={state.category} />
      </div>
      <p className="favorites-summary" role="status">
        {total === 0
          ? 'No picks yet. That is fine — you can save now and add favorites later.'
          : `${total} pick${total === 1 ? '' : 's'} selected.`}
      </p>
      {state.message !== '' && (
        <p className="favorites-message" role="alert">
          {state.message}
        </p>
      )}
    </section>
  )
}

function SearchResults({ controller, category }: { readonly controller: FavoritesController; readonly category: Category }) {
  const state = useSyncExternalStore(controller.subscribe, controller.read, controller.read)
  const results = state.results
  if (results.phase === 'idle') return null
  if (results.phase === 'loading' && results.data === null) {
    return <p role="status">Searching the catalog…</p>
  }
  if (results.phase === 'error') {
    return (
      <div role="status">
        <p>{state.message}</p>
        <button type="button" onClick={() => void controller.submit()}>
          Retry search
        </button>
      </div>
    )
  }
  const items = results.data ?? []
  if (items.length === 0) {
    return <p role="status">No {CATEGORY_LABEL[category].toLowerCase()} match that search.</p>
  }
  const full = !controller.canAdd(category)
  return (
    <>
      <ul className="favorites-results" aria-live="polite">
        {items.map((item) => {
          const selected = controller.selected(item)
          const url = catalogExternalUrl(item)
          return (
            <li key={item.catalog_item_id} className="favorites-result">
              <div className="favorites-result-copy">
                <strong>{item.label}</strong>
                <span>{catalogSubtitle(item)}</span>
              </div>
              {url !== null && (
                <a
                  className="favorites-result-external"
                  href={url}
                  target="_blank"
                  rel="noopener noreferrer"
                  aria-label={catalogExternalLinkLabel(item)}
                >
                  Open
                </a>
              )}
              <button
                type="button"
                aria-label={selected ? `${item.label} is already picked` : `Pick ${item.label}`}
                disabled={selected || full}
                onClick={() => controller.add(item)}
              >
                {selected ? 'Picked' : 'Pick'}
              </button>
            </li>
          )
        })}
      </ul>
      {state.nextCursor !== null && (
        <button type="button" onClick={() => void controller.more()}>
          More results
        </button>
      )}
    </>
  )
}

function PickedItems({ controller, category }: { readonly controller: FavoritesController; readonly category: Category }) {
  const state = useSyncExternalStore(controller.subscribe, controller.read, controller.read)
  const items = state.selection[category === 'MUSIC' ? 'music' : category === 'ANIME' ? 'anime' : 'games']
  return (
    <section className="favorites-picked" aria-label={`Picked ${CATEGORY_LABEL[category].toLowerCase()}`}>
      <h2>
        Picked ({items.length} / {MAX_PER_CATEGORY})
      </h2>
      {items.length === 0 ? (
        <p>Nothing picked in this category. You can leave it empty.</p>
      ) : (
        <ul>
          {items.map((item) => (
            <li key={item.catalog_item_id}>
              <span>{item.label}</span>
              <button
                type="button"
                aria-label={`Remove ${item.label}`}
                onClick={() => controller.remove(item)}
              >
                Remove
              </button>
            </li>
          ))}
        </ul>
      )}
    </section>
  )
}
