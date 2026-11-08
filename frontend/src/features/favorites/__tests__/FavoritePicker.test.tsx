// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest'
import { cleanup, fireEvent, render, screen, within } from '@testing-library/react'
import { MemoryRouter } from 'react-router-dom'
import { FavoritePicker } from '../FavoritePicker'
import {
  addFavorite,
  CATEGORIES,
  CATEGORY_LABEL,
  emptySelection,
  MAX_PER_CATEGORY,
  validateSelection,
  type FavoriteItem,
} from '../contract'
import { catalogItem, favoritesHarness, page } from './harness'

const disposals: (() => void)[] = []
afterEach(() => {
  cleanup()
  disposals.splice(0).forEach((dispose) => dispose())
})

function item(category: 'MUSIC' | 'ANIME' | 'GAME', label: string): FavoriteItem {
  return catalogItem(category, label)
}

async function setup(respond?: (request: Request) => Promise<Response>) {
  const harness = favoritesHarness()
  if (respond) harness.respond(respond)
  disposals.push(() => harness.dispose())
  render(
    <MemoryRouter>
      <FavoritePicker controller={harness.controller} />
    </MemoryRouter>,
  )
  harness.controller.start()
  return harness
}

describe('FavoritePicker', () => {
  it('shows the three tabs in the order music, anime, games', async () => {
    await setup()
    const tabs = screen.getAllByRole('tab')
    expect(tabs.map((tab) => (tab.textContent ?? '').replace(/\d+$/, '').trim())).toEqual([
      CATEGORY_LABEL.MUSIC,
      CATEGORY_LABEL.ANIME,
      CATEGORY_LABEL.GAME,
    ])
    expect(CATEGORIES).toEqual(['MUSIC', 'ANIME', 'GAME'])
    expect(tabs[0]?.getAttribute('aria-selected')).toBe('true')
  })

  it('searches the open category and offers the results', async () => {
    const harness = await setup(async (request) => {
      const category = new URL(request.url).pathname.split('/')[3]?.toUpperCase() as 'MUSIC'
      return Response.json(page(category, ['My Chemical Romance', 'Paramore']))
    })
    fireEvent.change(screen.getByRole('searchbox'), { target: { value: 'mcr' } })
    fireEvent.click(screen.getByRole('button', { name: 'Search' }))
    await vi.waitFor(() => expect(screen.getByText('My Chemical Romance')).toBeTruthy())
    const request = harness.requests.at(-1)!
    const url = new URL(request.url)
    expect(url.pathname).toBe('/api/catalog/music/search')
    expect(url.searchParams.get('q')).toBe('mcr')
    expect(url.searchParams.get('limit')).toBe('20')
    expect(screen.getByRole('button', { name: 'Pick Paramore' })).toBeTruthy()
  })

  it('switches category and searches the new category path', async () => {
    const harness = await setup(async (request) => {
      const category = new URL(request.url).pathname.split('/')[3]?.toUpperCase() as 'ANIME'
      return Response.json(page(category, ['NANA']))
    })
    fireEvent.click(screen.getByRole('tab', { name: /Anime/ }))
    fireEvent.change(screen.getByRole('searchbox'), { target: { value: 'nana' } })
    fireEvent.click(screen.getByRole('button', { name: 'Search' }))
    await vi.waitFor(() => expect(screen.getByText('NANA')).toBeTruthy())
    expect(new URL(harness.requests.at(-1)!.url).pathname).toBe('/api/catalog/anime/search')
  })

  // The acceptance case: a sixth selection in one category is blocked.
  it('blocks a sixth pick in one category', async () => {
    const labels = ['One', 'Two', 'Three', 'Four', 'Five', 'Six']
    const harness = await setup(async () => Response.json(page('MUSIC', labels)))
    fireEvent.change(screen.getByRole('searchbox'), { target: { value: 'band' } })
    fireEvent.click(screen.getByRole('button', { name: 'Search' }))
    await vi.waitFor(() => expect(screen.getByText('One')).toBeTruthy())
    for (const label of labels.slice(0, MAX_PER_CATEGORY)) {
      fireEvent.click(screen.getByRole('button', { name: `Pick ${label}` }))
    }
    // The sixth result's action is disabled, and the picker says why.
    const sixth = screen.getByRole('button', { name: 'Pick Six' }) as HTMLButtonElement
    expect(sixth.disabled).toBe(true)
    fireEvent.click(sixth)
    // The controller refuses it too, with the reason, even though the control
    // is already disabled — a caller that bypasses the UI gets the same answer.
    expect(harness.controller.add(catalogItem('MUSIC', 'Six'))).toBe(false)
    await vi.waitFor(() =>
      expect(screen.getByRole('alert').textContent).toContain(`up to ${MAX_PER_CATEGORY}`),
    )
    // The picked list still holds exactly five.
    const picked = screen.getByRole('region', { name: /Picked bands and artists/i })
    expect(within(picked).getAllByRole('listitem')).toHaveLength(MAX_PER_CATEGORY)
    // And the contract refuses a sixth item directly, not only through the UI.
    let selection = emptySelection()
    for (const label of labels.slice(0, MAX_PER_CATEGORY)) {
      const result = addFavorite(selection, item('MUSIC', label))
      expect(result.ok).toBe(true)
      if (result.ok) selection = result.selection
    }
    expect(addFavorite(selection, item('MUSIC', 'Six'))).toEqual({ ok: false, reason: 'full' })
    expect(validateSelection({ ...selection, music: [...selection.music, item('MUSIC', 'Six')] })).toBeNull()
  })

  it('allows five in each category independently', async () => {
    const harness = await setup()
    for (const category of ['MUSIC', 'ANIME', 'GAME'] as const) {
      for (let index = 0; index < MAX_PER_CATEGORY; index += 1) {
        expect(harness.controller.add(item(category, `${category}-${index}`))).toBe(true)
      }
      expect(harness.controller.canAdd(category)).toBe(false)
    }
    const state = harness.controller.read().selection
    expect(state.music).toHaveLength(MAX_PER_CATEGORY)
    expect(state.anime).toHaveLength(MAX_PER_CATEGORY)
    expect(state.games).toHaveLength(MAX_PER_CATEGORY)
  })

  it('refuses a duplicate pick in the same category', async () => {
    const harness = await setup()
    const first = item('ANIME', 'NANA')
    expect(harness.controller.add(first)).toBe(true)
    expect(harness.controller.add(first)).toBe(false)
    expect(harness.controller.read().selection.anime).toHaveLength(1)
    expect(harness.controller.read().message).toBe('That is already one of your picks.')
  })

  // The acceptance case: skipping all categories leaves the profile usable.
  it('skipping every category leaves the profile usable', async () => {
    const harness = await setup()
    const state = harness.controller.read()
    // Nothing is picked, and the picker says so without blocking anything.
    expect(state.selection.music).toHaveLength(0)
    expect(state.selection.anime).toHaveLength(0)
    expect(state.selection.games).toHaveLength(0)
    expect(harness.controller.canSave()).toBe(true)
    expect(screen.getByText(/you can skip every category/i)).toBeTruthy()
    expect(screen.getByText(/No picks yet\. That is fine/i)).toBeTruthy()
    // No control in the picker is disabled because the selection is empty.
    for (const button of screen.getAllByRole('button')) {
      if (button.textContent === 'Search') expect(button).toBeTruthy()
    }
    // Clearing an already-empty selection is a no-op, not an error.
    harness.controller.clear()
    expect(harness.controller.read().selection).toEqual(emptySelection())
    // The empty selection is a valid contract value.
    expect(validateSelection(emptySelection())).toEqual(emptySelection())
  })

  it('removes a pick and frees the slot', async () => {
    const harness = await setup()
    for (let index = 0; index < MAX_PER_CATEGORY; index += 1) {
      harness.controller.add(item('GAME', `Game ${index}`))
    }
    expect(harness.controller.canAdd('GAME')).toBe(false)
    const first = harness.controller.read().selection.games[0]
    expect(first).toBeDefined()
    harness.controller.remove(first!)
    expect(harness.controller.canAdd('GAME')).toBe(true)
    expect(harness.controller.read().selection.games).toHaveLength(MAX_PER_CATEGORY - 1)
  })

  it('renders no intensity, importance or preference control', async () => {
    await setup(async () => Response.json(page('MUSIC', ['MCR'])))
    fireEvent.change(screen.getByRole('searchbox'), { target: { value: 'mcr' } })
    fireEvent.click(screen.getByRole('button', { name: 'Search' }))
    await vi.waitFor(() => expect(screen.getByText('MCR')).toBeTruthy())
    expect(screen.queryByRole('slider')).toBeNull()
    expect(screen.queryByRole('spinbutton')).toBeNull()
    expect(screen.queryByRole('radiogroup')).toBeNull()
    const text = document.body.textContent ?? ''
    for (const forbidden of ['intensity', 'importance', 'preference', 'weight']) {
      expect(text.toLowerCase()).not.toContain(forbidden)
    }
    // The only text input is the search field. There is no numeric input, no
    // slider and no radio group, so no intensity can be expressed at all.
    const inputs = [...document.querySelectorAll('input')]
    expect(inputs).toHaveLength(1)
    expect(inputs[0]?.getAttribute('type')).toBe('search')
  })

  it('reports a failed search without losing the picks', async () => {
    const harness = await setup(async () => new Response('{}', { status: 503 }))
    harness.controller.add(item('MUSIC', 'Kept Band'))
    fireEvent.change(screen.getByRole('searchbox'), { target: { value: 'x' } })
    fireEvent.click(screen.getByRole('button', { name: 'Search' }))
    await vi.waitFor(() => expect(screen.getByRole('button', { name: 'Retry search' })).toBeTruthy())
    expect(harness.controller.read().selection.music).toHaveLength(1)
  })

  it('refuses a catalog page from a different category', async () => {
    await setup(async () => Response.json(page('ANIME', ['Wrong'])))
    fireEvent.change(screen.getByRole('searchbox'), { target: { value: 'x' } })
    fireEvent.click(screen.getByRole('button', { name: 'Search' }))
    await vi.waitFor(() => expect(screen.getByRole('button', { name: 'Retry search' })).toBeTruthy())
    expect(screen.queryByText('Wrong')).toBeNull()
  })

  it('does not send a blank search', async () => {
    const harness = await setup()
    fireEvent.change(screen.getByRole('searchbox'), { target: { value: '   ' } })
    const button = screen.getByRole('button', { name: 'Search' }) as HTMLButtonElement
    expect(button.disabled).toBe(true)
    await harness.controller.submit()
    expect(harness.requests).toHaveLength(0)
  })
})
