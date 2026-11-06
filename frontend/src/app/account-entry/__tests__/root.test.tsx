// @vitest-environment jsdom
import '@testing-library/jest-dom/vitest'
import { StrictMode } from 'react'
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { afterEach, expect, it, vi } from 'vitest'
import { createMemoryRouter, RouterProvider } from 'react-router-dom'
import { createAppRoutes } from '../../router'
import { createFixturePorts, bootstrapFor } from '../../__tests__/fixtures/ports'
import { eventId, participant, setup } from '../../../features/me/__tests__/fixtures'
const disposers:(()=>void)[]=[]
afterEach(()=>{cleanup();disposers.splice(0).forEach(f=>f())})
function root(path='/me') {
  const fetcher=vi.fn(async()=>Response.json(participant())),f=setup(fetcher),world=createFixturePorts(),boot=vi.fn(async scope=>bootstrapFor(scope))
  const router=createMemoryRouter(createAppRoutes({...world.ports,auth:f.auth,session:f.auth.session,bootstrap:{load:boot}}),{initialEntries:[path]})
  disposers.push(()=>{router.dispose();f.dispose()})
  const view=render(<StrictMode><RouterProvider router={router}/></StrictMode>)
  return{...f,...world,...view,router,boot,fetcher}
}
it('mounts the actual four lazy account routes while preserving one shell lease and private draft',async()=>{
  const f=root();await screen.findByRole('heading',{name:'Synthetic listener'})
  const host=document.querySelector('[data-world-host]'),mounts=f.stats.mounts
  await act(()=>f.router.navigate('/me/profile'));fireEvent.change(await screen.findByLabelText('Bio (optional)'),{target:{value:'Keep across panels'}})
  await act(()=>f.router.navigate('/me/settings'));await screen.findByRole('heading',{name:'Settings',level:1})
  expect(screen.getByLabelText('Enable sound')).not.toBeChecked()
  await act(()=>f.router.navigate('/me/avatar'));await screen.findByText(/Reviewed avatar artwork is unavailable/)
  expect(screen.getByRole('button',{name:'Save outfit'})).toBeDisabled();expect(document.querySelector('canvas')).toBeNull()
  await act(()=>f.router.navigate('/world'));await act(()=>f.router.navigate('/me/profile'))
  expect(await screen.findByLabelText('Bio (optional)')).toHaveValue('Keep across panels')
  expect(document.querySelector('[data-world-host]')).toBe(host);expect(f.stats.mounts).toBe(mounts)
  expect(f.stats.peakGames).toBe(1);expect(f.stats.peakConnections).toBe(1);expect(f.fetcher).toHaveBeenCalledTimes(1);expect(f.boot).toHaveBeenCalledTimes(1)
})
it('does not start account network work while viewing the world and releases all shell leases on exit',async()=>{
  const f=root('/world');await screen.findByRole('navigation',{name:'Main navigation'})
  expect(f.fetcher).not.toHaveBeenCalled();f.unmount();expect(f.stats.games).toBe(0);expect(f.stats.connections).toBe(0);expect(f.stats.mounts).toBe(f.stats.disposals)
})
it('clears the old profile draft across a canonical identity change without showing foreign responses',async()=>{
  const f=root('/me/profile');const bio=await screen.findByLabelText('Bio (optional)');await waitFor(()=>expect(bio).not.toBeDisabled())
  fireEvent.change(bio,{target:{value:'Old identity draft'}})
  await act(()=>{f.auth.session.replace(f.auth.session.read().generation,{userId:eventId,eventId,accessToken:'NEXT_SYNTHETIC'})})
  await screen.findByText('Could not load your profile. Retry to read the latest saved version.')
  expect(screen.getByLabelText('Bio (optional)')).toHaveValue('');expect(screen.getByLabelText('Display name')).toHaveValue('')
  expect(f.stats.peakGames).toBe(1);expect(f.stats.peakConnections).toBe(1)
})
it('rebinds after policy-generation change and discards prior private drafts',async()=>{
  const f=root('/me/profile');const bio=await screen.findByLabelText('Bio (optional)');await waitFor(()=>expect(bio).not.toBeDisabled())
  fireEvent.change(bio,{target:{value:'Old policy draft'}})
  await act(()=>{f.auth.privacy.changePolicy(f.auth.privacy.capture(),'consent-revoked')})
  await waitFor(()=>expect(f.boot).toHaveBeenCalledTimes(2));await waitFor(()=>expect(screen.getByLabelText('Bio (optional)')).toHaveValue(''))
  expect(f.stats.peakGames).toBe(1);expect(f.stats.peakConnections).toBe(1)
})
it('leaves public login outside account, participant and world allocation',async()=>{
  const f=root('/login');await screen.findByRole('heading',{name:'You’re signed in'})
  expect(f.fetcher).not.toHaveBeenCalled();expect(f.boot).not.toHaveBeenCalled();expect(f.stats.mounts).toBe(0)
})
