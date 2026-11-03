// @vitest-environment jsdom
import { StrictMode, useLayoutEffect } from 'react'
import { act, cleanup, render, waitFor } from '@testing-library/react'
import { afterEach, expect, it, vi } from 'vitest'
import selected from '../../../../../contracts/fixtures/assets/selected-static/avatar-manifest.json'
import { PortsProvider } from '../../access'
import { bootstrapFor, createFixturePorts } from '../../__tests__/fixtures/ports'
import { setup } from '../../../features/me/__tests__/fixtures'
import { createRootCharacterRegistry } from '../runtime'
import CharacterProvider, { useCharacterRuntime } from '../provider'

const stops: (() => void)[] = []
afterEach(() => { cleanup(); stops.splice(0).reverse().forEach(stop => stop()); vi.restoreAllMocks() })
function fixture() {
  const f = setup(vi.fn(async () => Response.json({}))), scope = f.auth.privacy.capture()
  const bootstrap = { ...bootstrapFor({ ...scope, userId: scope.userId!, eventId: scope.eventId! }), document: { avatar_manifest: selected } }
  const characters = createRootCharacterRegistry(f.auth, 'https://mcr.test'), fake = createFixturePorts()
  const ports = { ...fake.ports, auth: f.auth, characters, session: f.auth.session }
  stops.push(() => { characters.dispose(); f.dispose() })
  return { ...f, scope, bootstrap, characters, ports }
}
it('admits the shared runtime before a World child mounts, and retains it over route-like rerenders in StrictMode', async () => {
  const f = fixture(), runtimes = new Set<unknown>(), mount = vi.fn()
  function Child({ route }: { route: string }) {
    const runtime = useCharacterRuntime()
    useLayoutEffect(() => { mount(); runtimes.add(runtime); expect(runtime?.isCurrent()).toBe(true) }, [])
    return <p>{route}</p>
  }
  const tree = (route: string) => <StrictMode><PortsProvider ports={f.ports}><CharacterProvider bootstrap={f.bootstrap} scope={f.scope}><Child route={route} /></CharacterProvider></PortsProvider></StrictMode>
  const view = render(tree('World'))
  await waitFor(() => expect(view.container.textContent).toBe('World'))
  expect(runtimes.size).toBe(1); const current = f.characters.get(f.scope, f.bootstrap)
  view.rerender(tree('Me')); view.rerender(tree('Customize'))
  expect(f.characters.get(f.scope, f.bootstrap)).toBe(current); expect(current?.isCurrent()).toBe(true)
  view.unmount(); expect(current?.isCurrent()).toBe(true)
  f.characters.dispose(); expect(current?.isCurrent()).toBe(false)
})
it('withdraws the borrowed identity synchronously on policy revocation and never recaptures an old source', async () => {
  const f = fixture(), pin = { manifest_id: selected.manifest_id, manifest_version: selected.manifest_version, content_hash: selected.content_hash }
  function Child() { const runtime = useCharacterRuntime(); return <p>{runtime ? 'Current character' : 'Unavailable'}</p> }
  const view = render(<PortsProvider ports={f.ports}><CharacterProvider bootstrap={f.bootstrap} scope={f.scope}><Child /></CharacterProvider></PortsProvider>)
  await waitFor(() => expect(view.container.textContent).toBe('Current character'))
  const runtime = f.characters.read(f.scope, pin)!
  act(() => { f.auth.privacy.changePolicy(f.scope, 'block') })
  expect(runtime.isCurrent()).toBe(false); expect(f.characters.read(f.scope, pin)).toBeNull()
  view.rerender(<PortsProvider ports={f.ports}><CharacterProvider bootstrap={f.bootstrap} scope={f.scope}><Child /></CharacterProvider></PortsProvider>)
  expect(view.container.textContent).toBe('Unavailable')
})
