import { StrictMode, useSyncExternalStore, type ReactNode } from 'react'
import { act, cleanup, render, screen } from '@testing-library/react'
import { afterEach, expect, it, vi } from 'vitest'
import { MemoryRouter } from 'react-router-dom'
import { PortsProvider } from '../../../access'
import { RootSocialFeatures } from '../../../world-panels/social'
import { fixture } from '../../../social-composition/__tests__/fixtures'
import type { SocialAvailabilityPort, SocialContext } from '../../../social-composition/ports'
import type { ShellBootstrap } from '../../../ports'

vi.mock('../../../social-composition', () => ({
  SocialFeatures({ availability, children }: { availability: SocialAvailabilityPort; children: ReactNode }) {
    const value = useSyncExternalStore(availability.subscribe, availability.read, () => null)
    return <><output aria-label="Qualified feed">{value?.services.feed ? 'ready' : 'unavailable'}</output>{children}</>
  },
  SocialNavigationBoundary: ({ children }: { children: ReactNode }) => children,
}))
afterEach(cleanup)

it('StrictMode creates fresh committed lifetimes, releases the discarded one and never probes Feed', () => {
  const f = fixture(), owned: { disposed: boolean }[] = []
  const availability = vi.fn((_context: SocialContext) => {
    const record = { disposed: false }; owned.push(record)
    return { ...f.availability, dispose() { record.disposed = true } }
  })
  const ports = { ...f.ports, social: { availability } }
  const view = render(<StrictMode><MemoryRouter><PortsProvider ports={ports}>
    <RootSocialFeatures bootstrap={f.bootstrap} scope={f.scope}>Shell remains</RootSocialFeatures>
  </PortsProvider></MemoryRouter></StrictMode>)
  try {
    expect(owned.filter(value => !value.disposed)).toHaveLength(1)
    expect(owned.length).toBeGreaterThan(1); expect(screen.getByLabelText('Qualified feed').textContent).toBe('ready')
    expect(f.requests).toHaveLength(0); expect(f.listenerCount()).toBe(1)
    view.unmount(); expect(owned.every(value => value.disposed)).toBe(true); expect(f.listenerCount()).toBe(0)
  } finally { view.unmount(); f.dispose() }
})

it('replacing bootstrap withdraws the old port before the next factory and policy loss clears current UI', () => {
  const f = fixture(), records: { bootstrap: ShellBootstrap; disposed: boolean }[] = []
  const availability = (context: SocialContext) => {
    expect(records.every(value => value.disposed)).toBe(true)
    const record = { bootstrap: context.bootstrap, disposed: false }; records.push(record)
    const snapshot = Object.freeze({ bootstrap: context.bootstrap, scope: context.scope, services: Object.freeze({ feed: true }) })
    return { read: () => snapshot,
      subscribe: f.availability.subscribe, dispose() { record.disposed = true } }
  }
  const ports = { ...f.ports, social: { availability } }
  const root = (bootstrap: ShellBootstrap) => <MemoryRouter><PortsProvider ports={ports}>
    <RootSocialFeatures bootstrap={bootstrap} scope={f.scope}>Shell remains</RootSocialFeatures>
  </PortsProvider></MemoryRouter>
  const view = render(root(f.bootstrap))
  try {
    const next = { ...f.bootstrap }; view.rerender(root(next))
    expect(records).toHaveLength(2); expect(records[0]!.disposed).toBe(true); expect(records[1]!.bootstrap).toBe(next)
    expect(screen.getByLabelText('Qualified feed').textContent).toBe('ready')
    act(() => { f.auth.privacy.changePolicy(f.scope, 'block') })
    expect(screen.getByLabelText('Qualified feed').textContent).toBe('unavailable')
    expect(records.every(value => value.disposed)).toBe(true); expect(f.listenerCount()).toBe(0)
    expect(f.requests).toHaveLength(0)
  } finally { view.unmount(); f.dispose() }
})
