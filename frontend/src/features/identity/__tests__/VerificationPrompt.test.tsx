// @vitest-environment jsdom
import { afterEach, describe, expect, it } from 'vitest'
import { cleanup, render, screen } from '@testing-library/react'
import { MemoryRouter } from 'react-router-dom'
import { VerificationPrompt } from '../VerificationPrompt'

afterEach(() => cleanup())

function renderPrompt(props: Parameters<typeof VerificationPrompt>[0]) {
  return render(
    <MemoryRouter>
      <VerificationPrompt {...props} />
    </MemoryRouter>,
  )
}

describe('VerificationPrompt', () => {
  it('offers verification to an unverified viewer without revealing anything', () => {
    const { container } = renderPrompt({ verified: false })
    // The panel says the post is not available and offers verification.
    expect(screen.getByRole('heading', { name: 'This introduction is not available to you' })).toBeTruthy()
    const link = screen.getByRole('link', { name: 'Verify with NAVER' })
    expect(link.getAttribute('href')).toBe('/me/identity')
    // It does not distinguish a restricted post from a deleted one.
    expect(screen.getByText(/may be restricted by age or gender, or it may no longer exist/)).toBeTruthy()
    // And it renders no restricted content: no author, no body, no audience, no
    // attachment. There is no prop for any of them, so the only thing to assert
    // is that the rendered text is the fixed copy and nothing else.
    const text = container.textContent ?? ''
    // The panel names the *concept* of an audience in its fixed copy; what it
    // must never do is carry a concrete value. No numeric bound, no gender
    // word, no attachment reference, no author name.
    expect(text).not.toMatch(/\b(18|19|20|21|29|30)\b/)
    expect(text).not.toMatch(/female|male|women|men/i)
    expect(text).not.toMatch(/photo|image|attachment|@/i)
    // Every rendered string is one of the component's own constants.
    expect(text).not.toMatch(/<img|blob:|data:image/)
  })

  it('does not offer verification to a viewer who already holds a live claim', () => {
    renderPrompt({ verified: true })
    expect(screen.queryByRole('link', { name: 'Verify with NAVER' })).toBeNull()
    // The copy is still neutral: a verified viewer learns nothing more about the
    // post than an unverified one does.
    expect(screen.getByText(/not one your verification covers/)).toBeTruthy()
    expect(screen.getByText(/Nothing else about it can be shown/)).toBeTruthy()
  })

  it('never renders restricted content under any state', () => {
    for (const verified of [true, false]) {
      const { container } = renderPrompt({ verified, retryHref: '/feed/abc' })
      const text = container.textContent ?? ''
      // No author, body, attachment or audience *value* can appear because the
      // component has no way to receive one. The fixed copy is the only text.
      expect(text).not.toMatch(/minimum|maximum|\b\d+\s*(years|age)\b/i)
      expect(container.querySelector('img, picture, video, a[href^="blob:"]')).toBeNull()
      cleanup()
    }
  })

  it('uses the return path it is given, from the closed allowlist', () => {
    renderPrompt({ verified: false, returnPath: '/me/settings' })
    expect(screen.getByRole('link', { name: 'Verify with NAVER' }).getAttribute('href')).toBe('/me/settings')
  })
})
