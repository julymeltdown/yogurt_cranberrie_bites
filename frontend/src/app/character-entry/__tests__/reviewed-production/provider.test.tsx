// @vitest-environment jsdom
import { StrictMode, useLayoutEffect } from 'react'
import { cleanup, render, waitFor } from '@testing-library/react'
import { afterEach, expect, it, vi } from 'vitest'
import { createSessionController } from '../../../../features/auth/model/session'
import { reviewedBundle } from '../../../../features/avatar/__tests__/reviewed-production/fixtures'
import { decodeBootstrap } from '../../../../features/auth/model/bootstrap'
import { object } from '../../../../features/auth/model/contract'
import { createFixturePorts } from '../../../__tests__/fixtures/ports'
import { PortsProvider } from '../../../access'
import { createRootCharacterRegistry } from '../../runtime'
import CharacterProvider, { useCharacterRuntime, useReviewedCharacterOwner } from '../../provider'
import { createAccountOwner } from '../../../account-entry/owner'
import type { ReviewedCharacterOwner } from '../../reviewed-registry'

const stops: (() => void)[] = []
afterEach(() => { cleanup(); stops.splice(0).reverse().forEach(fn => fn()); vi.restoreAllMocks() })
async function fixture() {
  const bundle = await reviewedBundle(), eventId = String(object(bundle.event).event_id), userId = String(object(bundle.participant).user_id)
  const fetch = vi.fn(async () => Response.json(bundle))
  const auth = createSessionController({ eventId, fetch }); auth.session.replace(0, { userId, eventId, accessToken: 'TEST_ONLY_reviewed_root' })
  const scope = auth.privacy.capture(), bootstrap = await decodeBootstrap(bundle, { generation: scope.generation, userId, eventId })
  const characters = createRootCharacterRegistry(auth, 'https://mcr.test'), fake = createFixturePorts()
  stops.push(() => { characters.dispose(); auth.dispose() })
  return { auth, scope, bootstrap, characters, fetch, bundle, ports: { ...fake.ports, auth, characters, session: auth.session } }
}
it('admits one reviewed runtime before children mount in StrictMode and retains it across route navigation/unmount', async () => {
  const f = await fixture(), owners = new Set<ReviewedCharacterOwner>()
  function Child({ route }: { route: string }) {
    const whole = useCharacterRuntime(), reviewed = useReviewedCharacterOwner()
    useLayoutEffect(() => { expect(whole).toBeNull(); expect(reviewed?.isCurrent()).toBe(true); if (reviewed) owners.add(reviewed) }, [whole, reviewed])
    return <p>{route}</p>
  }
  const tree = (route: string) => <StrictMode><PortsProvider ports={f.ports}><CharacterProvider bootstrap={f.bootstrap} scope={f.scope}><Child route={route} /></CharacterProvider></PortsProvider></StrictMode>
  const view = render(tree('World'))
  await waitFor(() => expect(view.container.textContent).toBe('World'))
  view.rerender(tree('Customize')); view.rerender(tree('Profile'))
  expect(owners.size).toBe(1); expect(f.fetch).toHaveBeenCalledTimes(1)
  const borrowed = [...owners][0]!
  view.unmount(); expect(borrowed.isCurrent()).toBe(true)
  f.characters.dispose(); expect(borrowed.isCurrent()).toBe(false)
})
it('failed protected admission opens an honest unavailable context instead of hanging or manufacturing a preset', async () => {
  const f = await fixture(); object(f.bundle.participant).user_id = '00000000-0000-4000-8000-000000000099'
  function Child() { const reviewed = useReviewedCharacterOwner(), whole = useCharacterRuntime(); return <p>{reviewed || whole ? 'Unexpected authority' : 'Character unavailable'}</p> }
  const view = render(<PortsProvider ports={f.ports}><CharacterProvider bootstrap={f.bootstrap} scope={f.scope}><Child /></CharacterProvider></PortsProvider>)
  await waitFor(() => expect(view.container.textContent).toBe('Character unavailable'))
  expect(f.fetch).toHaveBeenCalledTimes(1)
})
it('Account borrows the identical authority/runtime and disposes its editor without destroying the shared character', async () => {
  const f = await fixture(), reviewed = (await f.characters.admitReviewed(f.scope, f.bootstrap))!
  const owner = createAccountOwner({ auth: f.auth, bootstrap: f.bootstrap, scope: f.scope, reviewed }); stops.push(owner.dispose)
  expect(owner.characters).toBeNull(); expect(owner.wholeAvatar).toBeNull()
  expect(owner.motionAvatar?.controller.lifetime).toBe('owner')
  expect(owner.reviewed).toBe(reviewed); expect(owner.motionAvatar?.isCurrent()).toBe(true)
  owner.dispose()
  expect(owner.motionAvatar?.isCurrent()).toBe(false); expect(reviewed.isCurrent()).toBe(true)
  expect(reviewed.runtime.diagnostics()).toMatchObject({ disposed: false, decodedAtlases: 0 })
  f.auth.privacy.changePolicy(f.scope, 'block'); expect(reviewed.isCurrent()).toBe(false)
})
