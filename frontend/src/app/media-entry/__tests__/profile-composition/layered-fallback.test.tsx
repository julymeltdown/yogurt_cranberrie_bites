// @vitest-environment jsdom
import '@testing-library/jest-dom/vitest'
import { cleanup, render, screen, waitFor } from '@testing-library/react'
import { afterEach, expect, it, vi } from 'vitest'
import { MemoryRouter } from 'react-router-dom'
import selected from '../../../../../../contracts/fixtures/assets/selected-static/avatar-manifest.json'
import { PortsProvider } from '../../../access'
import CharacterProvider from '../../../character-entry/provider'
import { createRootCharacterRegistry } from '../../../character-entry/runtime'
import { SocialFeatures } from '../../../social-composition/provider'
import { SocialPanel } from '../../../social-composition/panels'
import type { SocialOptions } from '../../../social-composition/ports'
import { fixture, publicIntro, user } from '../../../social-composition/__tests__/fixtures'
const stops: (() => void)[] = []
afterEach(() => { cleanup(); stops.splice(0).reverse().forEach(stop => stop()) })
it('keeps the authorized 1.0 profile on its existing scoped layered resolver even with a current selected-character runtime', async () => {
  const f = fixture(), bootstrap = { ...f.bootstrap, document: { avatar_manifest: selected } }
  f.setLease({ ...f.availability.read()!, bootstrap })
  const characters = createRootCharacterRegistry(f.auth, 'https://mcr.example'), ports = { ...f.ports, characters }
  const runtime = characters.get(f.scope, bootstrap)!, acquire = vi.spyOn(runtime, 'acquire')
  const resolve = vi.fn<NonNullable<SocialOptions['avatars']>['resolve']>(async () => { throw new Error('Explicit unavailable layered fixture resources') })
  stops.push(() => { characters.dispose(); f.dispose() })
  render(<MemoryRouter><PortsProvider ports={ports}><CharacterProvider bootstrap={bootstrap} scope={f.scope}>
    <SocialFeatures {...f.options} bootstrap={bootstrap} avatars={{ resolve }}><SocialPanel path={`/people/${user}`} /></SocialFeatures>
  </CharacterProvider></PortsProvider></MemoryRouter>)
  await screen.findByRole('heading', { name: publicIntro().author.display_name })
  await waitFor(() => expect(resolve).toHaveBeenCalledTimes(1))
  expect(resolve.mock.calls[0]![0]).toMatchObject({ auth: f.auth, bootstrap, scope: f.scope })
  expect(screen.getByText('Avatar unavailable', { exact: true })).toBeVisible()
  expect(acquire).not.toHaveBeenCalled()
  expect(f.requests.some(r => new URL(r.url).pathname.endsWith('/feed'))).toBe(false)
})
