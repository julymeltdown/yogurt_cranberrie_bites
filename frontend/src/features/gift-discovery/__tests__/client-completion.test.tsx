// @vitest-environment jsdom
import '@testing-library/jest-dom/vitest'
import {afterEach, expect, it} from 'vitest'
import {act, cleanup, fireEvent, render, screen, waitFor} from '@testing-library/react'
import {MemoryRouter} from 'react-router-dom'
import {GiftDiscovery} from '../ui/Gifts'
import {MyGifts} from '../../my-gifts/MyGifts'
import {fixture} from './fixtures/data'

const owners: ReturnType<typeof fixture>[] = []
afterEach(() => {cleanup(); owners.splice(0).forEach(owner => owner.dispose())})
const setup = () => {const owner = fixture(); owners.push(owner); return owner}

it('refreshes the selected giving collection after same-scope event decisions change', async () => {
  const owner = setup()
  render(<MemoryRouter><GiftDiscovery controller={owner.controller}/></MemoryRouter>)
  fireEvent.click(await screen.findByRole('tab', {name: 'My giving'}))
  expect(await screen.findByText('Only you can see this draft.')).toBeInTheDocument()
  const context = owner.context()!
  await act(async () => owner.publish({...context, eventPhase: 'READ_ONLY', capabilities: {...context.capabilities,
    mutations: {available: false, reason: 'EVENT_READ_ONLY'}}}))
  await waitFor(() => expect(owner.controller.read().giving.phase).toBe('ready'))
  expect(screen.getByText('Only you can see this draft.')).toBeInTheDocument()
  expect(owner.requests.filter(request => new URL(request.url).pathname.endsWith('/me/giveaways'))).toHaveLength(2)
})

it('restores My gifts after current gifts access is withdrawn and restored in the same privacy scope', async () => {
  const owner = setup()
  render(<MemoryRouter><MyGifts controller={owner.controller}/></MemoryRouter>)
  expect(await screen.findByText('Only you can see this draft.')).toBeInTheDocument()
  const context = owner.context()!
  await act(async () => owner.publish({...context, capabilities: {...context.capabilities,
    gifts: {available: false, reason: 'DEPENDENCY_UNAVAILABLE'}}}))
  expect(screen.getByRole('heading', {name: 'Gifts are unavailable'})).toBeInTheDocument()
  expect(screen.queryByText('Only you can see this draft.')).not.toBeInTheDocument()
  await act(async () => owner.publish(context))
  await waitFor(() => expect(owner.controller.read().giving.phase).toBe('ready'))
  expect(screen.getByText('Only you can see this draft.')).toBeInTheDocument()
  fireEvent.click(screen.getByRole('button', {name: 'RECEIVING'}))
  expect(screen.getByText('Held')).toBeInTheDocument()
  expect(owner.requests.filter(request => new URL(request.url).pathname.endsWith('/me/giveaways'))).toHaveLength(2)
})
