// @vitest-environment jsdom
import '@testing-library/jest-dom/vitest'
import { StrictMode } from 'react'
import { cleanup, fireEvent, render, screen, within } from '@testing-library/react'
import { afterEach, expect, it, vi } from 'vitest'
import { createMemoryRouter, RouterProvider } from 'react-router-dom'
import { createAppRoutes } from '../../../../app/router'
import { createFixturePorts } from '../../../../app/__tests__/fixtures/ports'
import type { SessionReadiness } from '../../../../app/ports'
const stops: (() => void)[] = []
afterEach(() => { cleanup(); stops.splice(0).reverse().forEach(stop => stop()); vi.unstubAllGlobals() })
function setup(readiness: SessionReadiness = 'anonymous') {
  vi.stubGlobal('fetch', vi.fn(async () => new Response(null, { status: 404 })))
  const fixture = createFixturePorts(readiness)
  fixture.ports.session.replace(fixture.ports.session.read().generation, null)
  const retry = vi.fn(() => fixture.setReadiness('checking'))
  const router = createMemoryRouter(createAppRoutes({ ...fixture.ports, readiness: { ...fixture.ports.readiness, retry } }), { initialEntries: ['/'] })
  render(<StrictMode><RouterProvider router={router} /></StrictMode>)
  stops.push(() => router.dispose())
  return { fixture, router, retry }
}
it('uses the target CTA wording with the existing explicit sign-in destination and accessible name', async () => {
  const f = setup(), cta = await screen.findByRole('link', { name: 'Sign in to enter' })
  expect(cta).toHaveTextContent('Enter the world'); expect(cta).toHaveAttribute('href', '/login?returnTo=%2Fworld')
  expect(cta).toHaveAttribute('aria-describedby', 'public-entry-status'); expect(screen.getByRole('status')).toHaveTextContent('Sign in to check entry to the world.')
  expect(f.fixture.stats.mounts).toBe(0); expect(document.querySelector('[data-world-host]')).toBeNull()
})
it('keeps one visible heading and real Parade Ink navigation without obsolete ornament controls or fallback art', async () => {
  setup(); await screen.findByRole('link', { name: 'Sign in to enter' })
  expect(screen.getAllByRole('heading', { level: 1 })).toHaveLength(1)
  expect(screen.getByRole('link', { name: 'MCR Concert World home' })).toHaveAttribute('href', '/')
  expect(screen.getByRole('link', { name: 'Sign in' })).toHaveAttribute('href', '/login')
  expect(screen.getByRole('heading', { level: 1 })).toBeVisible()
  expect(screen.getByRole('link', { name: 'MCR Concert World home' })).toHaveTextContent('MCR Concert World')
  expect(document.querySelector('.landing-window-stars,.landing-wordmark,.landing-web,.landing-scene')).toBeNull()
  expect(document.querySelector('.landing-crowd')).toHaveAttribute('data-art-state', 'unavailable')
  expect(document.querySelectorAll('.landing-crowd > span')).toHaveLength(0)
  expect(screen.queryByRole('button', { name: /close/i })).not.toBeInTheDocument()
})
it('preserves public legal navigation without applying the poster frame to documents', async () => {
  setup(); await screen.findByRole('link', { name: 'Sign in to enter' })
  fireEvent.click(within(screen.getByRole('navigation', { name: 'Public information' })).getByRole('link', { name: 'Terms' }))
  await screen.findByRole('heading', { name: 'Terms' })
  expect(document.querySelector('.public-entry--poster')).toBeNull(); expect(document.querySelector('.landing-wordmark')).toBeNull()
  expect(screen.getByRole('link', { name: 'Back to home' })).toHaveAttribute('href', '/')
})
it('keeps secondary community copy disclosed outside the initial poster while retaining its safety link', async () => {
  setup(); await screen.findByRole('link', { name: 'Sign in to enter' })
  const details = document.querySelector('.public-about-disclosure')!
  expect(details).not.toHaveAttribute('open'); expect(screen.getByText('A place for the crowd.')).not.toBeVisible()
  fireEvent.click(screen.getByText('About this space'))
  expect(details).toHaveAttribute('open'); expect(screen.getByRole('link', { name: 'Read the safety information' })).toHaveAttribute('href', '/legal/safety')
})
it('retains retry/readiness behavior and never turns an error into a sign-in or world grant', async () => {
  const f = setup('error'); fireEvent.click(await screen.findByRole('button', { name: 'Check entry again' }))
  expect(f.retry).toHaveBeenCalledTimes(1); expect(await screen.findByRole('button', { name: 'Checking entry…' })).toBeDisabled()
  expect(screen.getByRole('status')).toHaveTextContent('Checking your session…'); expect(f.router.state.location.pathname).toBe('/')
  expect(f.fixture.stats.mounts).toBe(0); expect(screen.queryByRole('link', { name: 'Sign in to enter' })).not.toBeInTheDocument()
})
it('keeps dynamic checking text instead of painting the anonymous printed CTA over a readiness state', async () => {
  setup('checking')
  expect(await screen.findByRole('button', { name: 'Checking entry…' })).toBeDisabled()
  expect(document.querySelector('.landing-entry-print')).toBeNull()
  expect(screen.getByRole('status')).toHaveTextContent('Checking your session…')
  expect(screen.queryByRole('link', { name: 'Sign in to enter' })).not.toBeInTheDocument()
})
