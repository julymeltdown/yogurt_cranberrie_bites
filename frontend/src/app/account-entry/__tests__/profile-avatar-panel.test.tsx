// @vitest-environment jsdom
import '@testing-library/jest-dom/vitest'
import { StrictMode } from 'react'
import { cleanup, render, screen, waitFor } from '@testing-library/react'
import { afterEach, expect, it, vi } from 'vitest'
import { createMemoryRouter, RouterProvider } from 'react-router-dom'
import schema from '../../../../../contracts/assets/v2/reviewed/avatar-manifest.schema.json'
import { createAppRoutes } from '../../router'
import { createFixturePorts, bootstrapFor } from '../../__tests__/fixtures/ports'
import { setup } from '../../../features/me/__tests__/fixtures'

const disposers: (() => void)[] = []
afterEach(() => { cleanup(); disposers.splice(0).forEach(fn => fn()); vi.unstubAllGlobals(); vi.restoreAllMocks() })
const manifest = schema.$defs.ReviewedMotionAvatarManifest.const

/**
 * WP-08 T-704: the shell selects the *profile* avatar editor, not the concert
 * `MotionEditor`, and nothing about the selection depends on an event.
 *
 * The concert panel selected `MotionEditor` only when an event-scoped
 * `motionAvatar` owner existed and otherwise fell back to a legacy editor. This
 * test drives the real router with a reviewed-motion release and asserts the
 * profile editor is the one mounted.
 */
function root() {
  vi.stubGlobal('location', { origin: 'https://mcr.test' })
  const fetcher = vi.fn(async () => Response.json({ revision: 1, config: manifest.presets[0]!.config }))
  const f = setup(fetcher), world = createFixturePorts()
  const boot = vi.fn(async (scope: Parameters<typeof bootstrapFor>[0]) => {
    const base = bootstrapFor(scope)
    return Object.freeze({ ...base, document: Object.freeze({ ...base.document, avatar_manifest: structuredClone(manifest) }) })
  })
  const router = createMemoryRouter(createAppRoutes({ ...world.ports, auth: f.auth, session: f.auth.session, bootstrap: { load: boot } }), { initialEntries: ['/me/avatar'] })
  disposers.push(() => { router.dispose(); f.dispose() })
  const view = render(<StrictMode><RouterProvider router={router} /></StrictMode>)
  return { ...f, ...world, ...view, router, boot, fetcher }
}

it('mounts the profile avatar editor for a reviewed release, with no concert editor and no event gate', async () => {
  const f = root()
  // The profile editor's own heading, not the concert window's "CUSTOMIZE".
  await waitFor(() => expect(screen.getByRole('heading', { name: 'Your character' })).toBeTruthy())
  expect(screen.getByRole('region', { name: 'Character editor' })).toBeTruthy()
  // The concert editor's chrome is gone.
  expect(screen.queryByRole('heading', { name: 'CUSTOMIZE' })).toBeNull()
  expect(screen.queryByText(/Reviewed avatar artwork is unavailable/)).toBeNull()
  // The profile editor is the one with the profile's own controls.
  expect(screen.getByRole('button', { name: 'Save character' })).toBeTruthy()
  expect(screen.queryByRole('button', { name: 'Save outfit' })).toBeNull()
  // The shell still holds exactly one world lease; the avatar panel adds none.
  expect(f.stats.peakGames).toBe(1)
  expect(f.stats.peakConnections).toBe(1)
  f.unmount()
  expect(f.stats.mounts).toBe(f.stats.disposals)
})

it('shows the explicit unavailable state rather than a fallback editor when admission fails', async () => {
  vi.stubGlobal('location', { origin: 'https://mcr.test' })
  const fetcher = vi.fn(async () => Response.json({ revision: 1, config: manifest.presets[0]!.config }))
  const f = setup(fetcher), world = createFixturePorts()
  const boot = vi.fn(async (scope: Parameters<typeof bootstrapFor>[0]) => {
    const base = bootstrapFor(scope)
    // A tampered manifest: the reviewed pin no longer matches.
    const tampered = structuredClone(manifest) as Record<string, unknown>
    tampered.content_hash = `sha256:${'0'.repeat(64)}`
    return Object.freeze({ ...base, document: Object.freeze({ ...base.document, avatar_manifest: tampered }) })
  })
  const router = createMemoryRouter(createAppRoutes({ ...world.ports, auth: f.auth, session: f.auth.session, bootstrap: { load: boot } }), { initialEntries: ['/me/avatar'] })
  disposers.push(() => { router.dispose(); f.dispose() })
  const { container } = render(<StrictMode><RouterProvider router={router} /></StrictMode>)
  await waitFor(() => expect(container.textContent).toContain('Your character'))
  await waitFor(() => expect(container.textContent).toContain('unavailable'))
  expect(screen.queryByRole('button', { name: 'Save character' })).toBeNull()
  expect(screen.queryByRole('button', { name: 'Save outfit' })).toBeNull()
  expect(document.querySelector('canvas')).toBeNull()
})
