// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest'
import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { MemoryRouter } from 'react-router-dom'
import { Discovery } from '../Discovery'
import {
  decodePeoplePage,
  DEFAULT_PEOPLE_CATEGORY,
  PEOPLE_CATEGORIES,
  sharedInterestLine,
} from '../contract'
import { card, discoveryHarness, peoplePage, sharedItem } from './harness'

const disposals: (() => void)[] = []
afterEach(() => {
  cleanup()
  disposals.splice(0).forEach((dispose) => dispose())
})

async function setup(
  respond?: (request: Request) => Promise<Response>,
  mutualsIntros?: React.ReactNode,
) {
  const harness = discoveryHarness()
  if (respond) harness.respond(respond)
  disposals.push(() => harness.dispose())
  render(
    <MemoryRouter>
      <Discovery controller={harness.controller} mutualsIntros={mutualsIntros} />
    </MemoryRouter>,
  )
  harness.controller.start()
  await harness.controller.load()
  return harness
}

describe('Discovery', () => {
  it('defaults to the music people tab and asks for the music category', async () => {
    const harness = await setup(async () =>
      Response.json(peoplePage([card('Mina', [sharedItem('MUSIC', 'My Chemical Romance')])])),
    )
    expect(DEFAULT_PEOPLE_CATEGORY).toBe('MUSIC')
    expect(PEOPLE_CATEGORIES).toEqual(['MUSIC', 'ANIME', 'GAME'])
    const url = new URL(harness.requests.at(-1)!.url)
    expect(url.pathname).toBe('/api/discover/people')
    expect(url.searchParams.get('category')).toBe('MUSIC')
    await vi.waitFor(() => expect(screen.getByText('Mina')).toBeTruthy())
  })

  it('shows the names of the actual shared items', async () => {
    await setup(async () =>
      Response.json(
        peoplePage([
          card('Mina', [sharedItem('MUSIC', 'My Chemical Romance'), sharedItem('MUSIC', 'NANA')]),
        ]),
      ),
    )
    await vi.waitFor(() =>
      expect(screen.getByText('You both love My Chemical Romance and NANA.')).toBeTruthy(),
    )
  })

  // The acceptance case: the shared-interest line is omitted with no overlap.
  it('omits the shared-interest line when there is no overlap', async () => {
    // The server never returns a card with no shared items in the people list,
    // so the empty case is proven where it can occur: the rendering rule and
    // the decoder. Both must produce no claim at all.
    expect(sharedInterestLine([])).toBeNull()
    expect(sharedInterestLine([{ ...sharedItem('MUSIC', '   ') }])).toBeNull()
    // A page whose card claims nothing is refused rather than rendered.
    expect(() => decodePeoplePage(peoplePage([card('Nobody', [])]))).toThrow()

    await setup(async () =>
      Response.json(peoplePage([card('Mina', [sharedItem('MUSIC', 'MCR')])])),
    )
    await vi.waitFor(() => expect(screen.getByText('Mina')).toBeTruthy())
    // The card that does have overlap shows exactly one line, and the screen
    // never shows an empty shared-interest paragraph.
    const shared = document.querySelectorAll('.discovery-shared')
    expect(shared).toHaveLength(1)
    for (const node of shared) expect((node.textContent ?? '').trim().length).toBeGreaterThan(0)
  })

  it('renders an interaction style only when the profile set one', async () => {
    await setup(async () =>
      Response.json(
        peoplePage([
          card('With style', [sharedItem('MUSIC', 'A')], { interaction_style: 'Slow replies, long messages.' }),
          card('Without style', [sharedItem('MUSIC', 'B')]),
        ]),
      ),
    )
    await vi.waitFor(() => expect(screen.getByText('Slow replies, long messages.')).toBeTruthy())
    expect(document.querySelectorAll('.discovery-interaction')).toHaveLength(1)
  })

  // The acceptance case: no room preview or room action on a discovery card.
  it('offers no room preview or room action on a discovery card', async () => {
    await setup(async () => Response.json(peoplePage([card('Mina', [sharedItem('MUSIC', 'MCR')])])))
    await vi.waitFor(() => expect(screen.getByText('Mina')).toBeTruthy())
    const text = (document.body.textContent ?? '').toLowerCase()
    expect(text).not.toContain('room')
    for (const link of screen.getAllByRole('link')) {
      expect(link.getAttribute('href')).not.toContain('/room')
    }
    // The card offers exactly the two documented actions.
    const article = screen.getByRole('article')
    expect(article.querySelectorAll('a').length).toBe(1)
    expect(article.querySelectorAll('button').length).toBe(1)
    expect(screen.getByRole('link', { name: 'Open profile' })).toBeTruthy()
    expect(screen.getByRole('button', { name: 'Follow' })).toBeTruthy()
  })

  it('keeps the people list and the mutuals-intro list as separate tabs', async () => {
    await setup(
      async () => Response.json(peoplePage([card('Mina', [sharedItem('MUSIC', 'MCR')])])),
      <ul aria-label="Mutuals intro posts">
        <li>
          <article>
            <p>An intro post body</p>
            <button type="button" aria-label="Heart this intro post">
              Heart
            </button>
          </article>
        </li>
      </ul>,
    )
    await vi.waitFor(() => expect(screen.getByText('Mina')).toBeTruthy())
    // The people tab shows no intro post.
    expect(screen.queryByText('An intro post body')).toBeNull()
    expect(screen.queryByRole('button', { name: 'Heart this intro post' })).toBeNull()
    fireEvent.click(screen.getByRole('tab', { name: 'Mutuals intro posts' }))
    // The intro tab shows no people card.
    expect(screen.getByText('An intro post body')).toBeTruthy()
    expect(screen.getByRole('button', { name: 'Heart this intro post' })).toBeTruthy()
    expect(screen.queryByText('Mina')).toBeNull()
    expect(screen.queryByRole('link', { name: 'Open profile' })).toBeNull()
  })

  it('switches the people category and reloads', async () => {
    const harness = await setup(async (request) => {
      const category = new URL(request.url).searchParams.get('category')
      return Response.json(
        peoplePage([card(category === 'ANIME' ? 'Anime fan' : 'Music fan', [sharedItem('ANIME', 'NANA')])]),
      )
    })
    await vi.waitFor(() => expect(screen.getByText('Music fan')).toBeTruthy())
    fireEvent.click(screen.getByRole('tab', { name: 'Anime' }))
    await vi.waitFor(() => expect(screen.getByText('Anime fan')).toBeTruthy())
    expect(new URL(harness.requests.at(-1)!.url).searchParams.get('category')).toBe('ANIME')
  })

  it('follows explicitly and only on request', async () => {
    const harness = await setup(async (request) => {
      if (request.method === 'PUT') return Response.json({ following: true, followed_by: false })
      return Response.json(peoplePage([card('Mina', [sharedItem('MUSIC', 'MCR')])]))
    })
    await vi.waitFor(() => expect(screen.getByText('Mina')).toBeTruthy())
    // Merely viewing the card sends no follow.
    expect(harness.requests.filter((request) => request.method === 'PUT')).toHaveLength(0)
    fireEvent.click(screen.getByRole('button', { name: 'Follow' }))
    await vi.waitFor(() => expect(screen.getByRole('button', { name: 'Following' })).toBeTruthy())
    const follow = harness.requests.find((request) => request.method === 'PUT')!
    expect(new URL(follow.url).pathname).toMatch(/^\/api\/profiles\/[0-9a-f-]{36}\/follow$/)
  })

  it('reports an empty people list without inventing a claim', async () => {
    await setup(async () => Response.json(peoplePage([])))
    await vi.waitFor(() =>
      expect(screen.getByText(/Nobody shares your bands and artists picks yet/i)).toBeTruthy(),
    )
    expect(document.querySelectorAll('.discovery-card')).toHaveLength(0)
  })

  it('reports a failed load and retries on request', async () => {
    await setup(async () => new Response('{}', { status: 503 }))
    await vi.waitFor(() => expect(screen.getByRole('button', { name: 'Retry' })).toBeTruthy())
    expect(screen.queryByRole('article')).toBeNull()
  })

  it('rejects a page that carries a score field', async () => {
    await setup(async () => {
      const page = peoplePage([card('Mina', [sharedItem('MUSIC', 'MCR')])])
      return Response.json({ ...page, items: [{ ...page.items[0], score: 92 }] })
    })
    await vi.waitFor(() => expect(screen.getByRole('button', { name: 'Retry' })).toBeTruthy())
    expect(screen.queryByText('Mina')).toBeNull()
  })

  it('rejects a page whose cards repeat a profile', async () => {
    const id = crypto.randomUUID()
    await setup(async () =>
      Response.json(
        peoplePage([
          card('Mina', [sharedItem('MUSIC', 'MCR')], { profile_id: id }),
          card('Mina again', [sharedItem('MUSIC', 'MCR')], { profile_id: id }),
        ]),
      ),
    )
    await vi.waitFor(() => expect(screen.getByRole('button', { name: 'Retry' })).toBeTruthy())
  })
})
