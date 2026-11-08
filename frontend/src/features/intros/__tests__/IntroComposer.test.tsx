// @vitest-environment jsdom
import { afterEach, describe, expect, it } from 'vitest'
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { QueryClient } from '@tanstack/react-query'
import { createMemorySession, createHttpClient } from '../../../shared/api/http'
import { createQueryPrivacy } from '../../../shared/query/privacy'
import { createIntroComposerController } from '../controller'
import { IntroComposer } from '../IntroComposer'

const disposals: (() => void)[] = []
afterEach(() => {
  cleanup()
  disposals.splice(0).forEach((dispose) => dispose())
})

const userId = '00000000-0000-4000-8000-000000000003'
const introId = '00000000-0000-4000-8000-0000000000aa'

function ownPost(overrides: Record<string, unknown> = {}) {
  return {
    intro_id: introId,
    state: 'DRAFT',
    body: 'A real introduction.',
    favorites_snapshot: [],
    currently_seeking: null,
    interaction_style: null,
    attachment_media_ids: [],
    audience: { allowed_gender: 'female', minimum_age: 18, maximum_age_exclusive: 30 },
    // The server's string. The composer must render exactly this.
    audience_summary: 'Women \u00b7 Ages 18\u201329',
    like_count: 0,
    comment_count: 0,
    published_at: null,
    expires_at: null,
    revision: 1,
    updated_at: '2026-11-09T00:00:00Z',
    ...overrides,
  }
}

function harness(respond: (request: Request) => Promise<Response>) {
  const session = createMemorySession()
  session.replace(0, { userId, eventId: null, accessToken: 'fixture-token' })
  const query = new QueryClient()
  const privacy = createQueryPrivacy({ client: query, session })
  const requests: Request[] = []
  const http = createHttpClient({
    session,
    fetch: async (input) => {
      const request = input as Request
      requests.push(request.clone())
      return respond(request)
    },
  })
  const controller = createIntroComposerController({
    auth: { privacy },
    request: http.request,
    key: () => '00000000-0000-4000-8000-0000000000ff',
  })
  controller.start()
  disposals.push(() => {
    controller.dispose()
    privacy.dispose()
    query.clear()
  })
  render(<IntroComposer controller={controller} />)
  return { controller, requests }
}

async function fillAudience() {
  fireEvent.change(screen.getByLabelText('Gender'), { target: { value: 'female' } })
  fireEvent.click(screen.getByLabelText('Limit by age'))
  fireEvent.change(screen.getByLabelText('Youngest age'), { target: { value: '18' } })
  fireEvent.change(screen.getByLabelText('Oldest age'), { target: { value: '29' } })
}

async function reachReview(bodyText = 'A real introduction.') {
  await fillAudience()
  fireEvent.click(screen.getByRole('button', { name: 'Continue' }))
  fireEvent.change(screen.getByLabelText('Your introduction'), { target: { value: bodyText } })
  fireEvent.click(screen.getByRole('button', { name: 'Continue' }))
  await waitFor(() => expect(screen.getByRole('heading', { name: 'Review' })).toBeTruthy())
}

describe('IntroComposer', () => {
  it('walks audience, content, review in order', async () => {
    harness(async () => Response.json(ownPost()))
    expect(screen.getByRole('heading', { name: 'Who can view my mutuals-intro post' })).toBeTruthy()
    await fillAudience()
    fireEvent.click(screen.getByRole('button', { name: 'Continue' }))
    expect(screen.getByLabelText('Your introduction')).toBeTruthy()
    fireEvent.change(screen.getByLabelText('Your introduction'), { target: { value: 'Hello.' } })
    fireEvent.click(screen.getByRole('button', { name: 'Continue' }))
    await waitFor(() => expect(screen.getByRole('heading', { name: 'Review' })).toBeTruthy())
  })

  // The acceptance case: the review step never computes the summary locally.
  it('shows the server-derived summary and never computes one', async () => {
    const { requests } = harness(async () => Response.json(ownPost()))
    await reachReview()
    // Before saving there is no summary, and the composer says so instead of
    // deriving one from the bounds it already knows (female, 18..30).
    expect(screen.getByText('Save the draft to see the audience summary.')).toBeTruthy()
    expect(screen.queryByText(/Women .* Ages 18/)).toBeNull()
    // Save the draft; the summary now comes from the server response.
    fireEvent.click(screen.getByRole('button', { name: 'Save draft' }))
    await waitFor(() =>
      expect(screen.getByText('Women \u00b7 Ages 18\u201329')).toBeTruthy(),
    )
    // The rendered text is exactly the server's string.
    const saved = requests.find((request) => request.method === 'PUT')!
    const body = JSON.parse(await saved.clone().text())
    expect(body.audience).toEqual({
      allowed_gender: 'female',
      minimum_age: 18,
      maximum_age_exclusive: 30,
    })
  })

  // A server summary that disagrees with the bounds is rendered verbatim: the
  // client never overrides it with its own computation.
  it('renders a server summary verbatim even when it looks unexpected', async () => {
    harness(async () =>
      Response.json(ownPost({ audience_summary: 'Server decided: everyone' })),
    )
    await reachReview()
    fireEvent.click(screen.getByRole('button', { name: 'Save draft' }))
    await waitFor(() => expect(screen.getByText('Server decided: everyone')).toBeTruthy())
    // The client's own reading of the bounds would have said "Women · Ages 18–29".
    expect(screen.queryByText(/Women \u00b7 Ages 18/)).toBeNull()
  })

  it('does not import a client-side summary helper', async () => {
    // The composer module must not contain a summary derivation. The only
    // summary text it can show is the one the controller hands it.
    const source = await import('../IntroComposer?raw').then((module) => (module as { default: string }).default)
    for (const forbidden of ['Ages ${', 'audienceSummary =', 'summaryFrom', 'formatAudience']) {
      expect(source).not.toContain(forbidden)
    }
  })

  // The acceptance case: publishing without a body is blocked client-side.
  it('blocks publishing without a body', async () => {
    const { requests } = harness(async () => Response.json(ownPost({ body: '' })))
    // Reach review with an empty body: the content step allows a draft.
    await fillAudience()
    fireEvent.click(screen.getByRole('button', { name: 'Continue' }))
    fireEvent.click(screen.getByRole('button', { name: 'Continue' }))
    await waitFor(() => expect(screen.getByRole('heading', { name: 'Review' })).toBeTruthy())
    fireEvent.click(screen.getByRole('button', { name: 'Save draft' }))
    await waitFor(() => expect(screen.getByRole('button', { name: 'Publish' })).toBeTruthy())
    const publish = screen.getByRole('button', { name: 'Publish' }) as HTMLButtonElement
    expect(publish.disabled).toBe(true)
    fireEvent.click(publish)
    // No publish request was sent.
    expect(requests.filter((request) => request.method === 'POST')).toHaveLength(0)
  })

  it('publishes a saved draft with a body', async () => {
    const { requests } = harness(async (request) => {
      if (request.method === 'PUT') return Response.json(ownPost())
      return Response.json(ownPost({ state: 'PUBLISHED', published_at: '2026-11-09T00:00:00Z', revision: 2 }))
    })
    await reachReview()
    fireEvent.click(screen.getByRole('button', { name: 'Save draft' }))
    await waitFor(() => expect(screen.getByRole('button', { name: 'Publish' })).toBeTruthy())
    const publish = screen.getByRole('button', { name: 'Publish' }) as HTMLButtonElement
    expect(publish.disabled).toBe(false)
    fireEvent.click(publish)
    await waitFor(() =>
      expect(requests.some((request) => request.method === 'POST' && request.url.includes('/publish'))).toBe(true),
    )
    const posted = requests.find((request) => request.url.includes('/publish'))!
    expect(JSON.parse(await posted.clone().text())).toEqual({ expected_revision: 1 })
  })

  it('keeps the draft when the save fails', async () => {
    harness(async () => new Response('{}', { status: 503 }))
    await reachReview('Keep me.')
    fireEvent.click(screen.getByRole('button', { name: 'Save draft' }))
    await waitFor(() => expect(screen.getByRole('alert').textContent).toContain('could not be saved'))
    // Going back shows the text is still there.
    fireEvent.click(screen.getByRole('button', { name: 'Back' }))
    expect((screen.getByLabelText('Your introduction') as HTMLTextAreaElement).value).toBe('Keep me.')
  })

  it('blocks Continue while the audience is invalid', async () => {
    harness(async () => Response.json(ownPost()))
    fireEvent.click(screen.getByLabelText('Limit by age'))
    fireEvent.change(screen.getByLabelText('Youngest age'), { target: { value: '30' } })
    fireEvent.change(screen.getByLabelText('Oldest age'), { target: { value: '18' } })
    await waitFor(() => expect(screen.getByRole('alert')).toBeTruthy())
    const next = screen.getByRole('button', { name: 'Continue' }) as HTMLButtonElement
    expect(next.disabled).toBe(true)
  })
})
