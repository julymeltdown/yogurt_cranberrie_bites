// @vitest-environment jsdom
// Synthetic presentation tests. No live Auth/World/room admission claim.
import { useEffect, useState } from 'react'
import { afterEach, expect, it, vi } from 'vitest'
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react'
import { createMemoryRouter, RouterProvider, useLocation } from 'react-router-dom'
import { NavigationMemory } from '../../../app/navigation'
import { BottomNavigation, EraControls } from '../../../app/shell-chrome'
import { WorldHudHeaderSlot, WorldHudLayout, WorldHudToolsSlot, WorldHudViewport } from './index'
import type { ShellPorts, WorldAccessBinding } from '../../../app/ports'
import type { ReplicationView } from '../../../world/network/replication'
import { WorldContextStrip } from './world-context'

let ports: Partial<ShellPorts> = {}
vi.mock('../../../app/access', () => ({ usePorts: () => ports }))
const routers: ReturnType<typeof createMemoryRouter>[] = []
afterEach(() => { cleanup(); routers.splice(0).forEach(router => router.dispose()); ports = {} })
let mounts = 0, disposals = 0
function ExistingWorld() { useEffect(() => { mounts++; return () => { disposals++ } }, []); return <div data-existing-world="" /> }
function Layout() {
  const location = useLocation(), [open, setOpen] = useState(false)
  return <NavigationMemory><WorldHudLayout active={location.pathname === '/world'} toolsOpen={open} onToolsChange={setOpen}>
    <WorldHudHeaderSlot><span>Current authorized identity</span></WorldHudHeaderSlot>
    <WorldHudViewport><ExistingWorld /></WorldHudViewport>
    <WorldHudToolsSlot><nav id="world-tools-destinations" hidden={!open}><a href="/world/map">Map</a></nav></WorldHudToolsSlot>
    <BottomNavigation />
  </WorldHudLayout></NavigationMemory>
}
function setup() {
  mounts = disposals = 0
  const router = createMemoryRouter([{ path: '*', element: <Layout /> }], { initialEntries: ['/world'] })
  routers.push(router); render(<RouterProvider router={router} />); return router
}
it('six incumbent navigation actions preserve one World child through every route', async () => {
  const router = setup(), nav = screen.getByRole('navigation', { name: 'Main navigation' })
  expect(nav.querySelectorAll('a')).toHaveLength(6)
  for (const [name,path] of [['Feed','/feed'],['Meet','/meet'],['Gifts','/gifts'],['Wall','/wall'],['Me','/me'],['World','/world']] as const) {
    fireEvent.click(screen.getByRole('link', { name }))
    await act(async () => {})
    expect(router.state.location.pathname).toBe(path)
    expect(nav.querySelector('[aria-current="page"]')?.textContent).toBe(name)
    expect(mounts).toBe(1); expect(disposals).toBe(0)
  }
})
it('the close-shaped action opens the real Feed route without replacing World', async () => {
  const router=setup()
  fireEvent.click(screen.getByRole('link',{name:'Open Feed'})); await act(async()=>{})
  expect(router.state.location.pathname).toBe('/feed');expect(mounts).toBe(1);expect(disposals).toBe(0)
})
it('minimizing only the new details is reversible and retains the existing World', () => {
  setup();expect(screen.getByText('Virtual presence, not physical arrival.')).toBeTruthy()
  fireEvent.click(screen.getByRole('button',{name:'Minimize World details'}))
  expect(screen.queryByText('Virtual presence, not physical arrival.')).toBeNull()
  expect(screen.getByRole('button',{name:'Show World details'}).getAttribute('aria-expanded')).toBe('false')
  fireEvent.click(screen.getByRole('button',{name:'Show World details'}))
  expect(screen.getByText('Virtual presence, not physical arrival.')).toBeTruthy();expect(mounts).toBe(1)
})
it('the destination menu retains account shortcuts and Escape returns to its real opener', () => {
  setup();const trigger=screen.getByRole('button',{name:'World destinations'})
  fireEvent.click(trigger);expect(trigger.getAttribute('aria-expanded')).toBe('true')
  const inbox=screen.getByRole('link',{name:'Inbox'});inbox.focus()
  fireEvent.keyDown(inbox,{key:'Escape'})
  expect(trigger.getAttribute('aria-expanded')).toBe('false');expect(document.activeElement).toBe(trigger)
})
it('absent era authority remains disabled without choosing PARADE for the UI appearance', () => {
  render(<EraControls />)
  expect(screen.getByRole('status').textContent).toBe('Eras unavailable.')
  expect(document.querySelector('[aria-pressed="true"]')).toBeNull()
  for(const button of screen.getAllByRole('button'))expect((button as HTMLButtonElement).disabled).toBe(true)
})
it('an incumbent pending era retains its committed pressed state and selection callback', async () => {
  const listeners=new Set<()=>void>(), select=vi.fn(async()=>{})
  let value={phase:'loading',selected:'REVENGE',pending:'PARADE',message:'',saved:false} as const
  render(<EraControls controller={{read:()=>value,subscribe:fn=>{listeners.add(fn);return()=>{listeners.delete(fn)}},select}} />)
  expect(screen.getByRole('button',{name:'Revenge'}).getAttribute('aria-pressed')).toBe('true')
  expect(screen.getByRole('button',{name:'Parade, loading'}).getAttribute('aria-pressed')).toBe('false')
  fireEvent.click(screen.getByRole('button',{name:'Danger'}));expect(select).toHaveBeenCalledWith('DANGER')
  expect(screen.getByRole('status').textContent).toContain('Revenge is still selected')
  // No new controller or optimistic local selected state is introduced.
  value={...value};act(()=>listeners.forEach(fn=>fn()))
  expect(screen.getByRole('button',{name:'Revenge'}).getAttribute('aria-pressed')).toBe('true')
})
function authorizedView() {
  const scope={userId:'00000000-0000-4000-8000-000000000001',eventId:'00000000-0000-4000-8000-000000000201',generation:1}
  const listeners=new Set<()=>void>(), lifetime=new AbortController()
  let current={scope,phase:'live',visible:true,baseline:{eventId:scope.eventId,selfUserId:scope.userId},
    entities:[{self:true,player:{userId:scope.userId,displayName:'Synthetic self',moving:false}}]} as unknown as ReplicationView
  let allowed=true
  const binding={release:{scope,privacyScope:{...scope,policyGeneration:1,gate:'open'},signal:lifetime.signal},
    replication:{read:()=>current,subscribe:(fn:()=>void)=>{listeners.add(fn);return()=>{listeners.delete(fn)}},switchRoom:vi.fn()}} as unknown as WorldAccessBinding
  ports={auth:{session:{isCurrent:()=>allowed},privacy:{isCurrent:()=>allowed}} as unknown as NonNullable<ShellPorts['auth']>,
    worldAccess:{read:()=>binding,subscribe:fn=>{listeners.add(fn);return()=>{listeners.delete(fn)}}}}
  return {emit:()=>act(()=>listeners.forEach(fn=>fn())),withdraw:()=>{allowed=false;lifetime.abort()},
    change:(patch:Partial<ReplicationView>)=>{current={...current,...patch}}}
}
function context() {
  const router=createMemoryRouter([{path:'*',element:<NavigationMemory><WorldContextStrip expanded /></NavigationMemory>}],{initialEntries:['/world']})
  routers.push(router);render(<RouterProvider router={router} />)
}
it('presence is an authorized in-view projection, not an invented room occupancy', () => {
  authorizedView();context();expect(screen.getByText('1 in view')).toBeTruthy()
  expect(screen.getByRole('link',{name:'Nearby'}).getAttribute('href')).toBe('/world/nearby')
  expect(document.body.textContent).not.toMatch(/Plaza|Lv\.|JOIN|queue group/)
})
it('withdrawn authority clears the old count even when the underlying reader retains data', () => {
  const source=authorizedView();context();source.withdraw();source.emit()
  expect(screen.queryByText('1 in view')).toBeNull();expect(screen.getByText('Presence unavailable')).toBeTruthy()
})
it('room switching does not relabel the old visible roster as a current count', () => {
  const source=authorizedView();source.change({phase:'switching'});context()
  expect(screen.queryByText('1 in view')).toBeNull();expect(screen.getByText('Changing rooms…')).toBeTruthy()
})
it('foreign-scope replication cannot become a visible count', () => {
  const source=authorizedView();source.change({scope:{userId:'other',eventId:'other',generation:2}});context()
  expect(screen.queryByText('1 in view')).toBeNull();expect(screen.getByText('Presence unavailable')).toBeTruthy()
})
