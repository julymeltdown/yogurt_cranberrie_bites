// @vitest-environment jsdom
import '@testing-library/jest-dom/vitest'
import { StrictMode } from 'react'
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { afterEach, expect, it } from 'vitest'
import { createMemoryRouter, RouterProvider } from 'react-router-dom'
import { createAppRoutes } from '../../router'
import { fixture, post, response, user } from '../../social-composition/__tests__/fixtures'
const stops:(()=>void)[]=[]
afterEach(()=>{cleanup();stops.splice(0).reverse().forEach(stop=>stop())})
function setup(path='/feed',enabled=true,uncertain=false){
 let factories=0
 const f=fixture(r=>{if(uncertain&&r.method==='POST')throw new TypeError('Test lost response');return response(r)})
 const ports={...f.ports,bootstrap:{load:async()=>{if(f.auth.privacy.capture().policyGeneration!==f.scope.policyGeneration)throw new Error('Old test lease unavailable');return f.bootstrap}},...(enabled?{social:{availability(context:Parameters<NonNullable<import('../../ports').ShellPorts['social']>['availability']>[0]){
  factories++;const value={bootstrap:context.bootstrap,scope:context.scope,services:{feed:true,people:true,intro:true,age:true,inbox:true}}
  return {read:()=>value,subscribe:()=>()=>{}}
 }}}:{})}
 const router=createMemoryRouter(createAppRoutes(ports),{initialEntries:[path]})
 render(<StrictMode><RouterProvider router={router}/></StrictMode>);stops.push(()=>{router.dispose();f.dispose()})
 return {...f,router,factories:()=>factories}
}
it('actual default routes use one stable social owner with caller-qualified readiness and same WorldHost',async()=>{
 const f=setup();await screen.findByText(post.text);const host=document.querySelector('[data-world-host]'),mounts=f.stats.mounts,created=f.factories()
 await act(()=>f.router.navigate('/feed/new'));fireEvent.change(await screen.findByLabelText('Post text'),{target:{value:'Retained actual root draft'}})
 await act(()=>f.router.navigate('/notifications'));await screen.findByText('37 unread in this event')
 await act(()=>f.router.navigate('/meet/people'));await screen.findByRole('heading',{name:'PEOPLE'})
 await act(()=>f.router.navigate(`/people/${user}`));await screen.findByRole('heading',{name:post.author.display_name})
 expect(screen.getByRole('button',{name:'Request sharing'})).toBeDisabled()
 await act(()=>f.router.navigate('/me/age'));expect(await screen.findByRole('button',{name:'Start verification'})).toBeDisabled()
 await act(()=>f.router.navigate('/feed/new'));expect(await screen.findByLabelText('Post text')).toHaveValue('Retained actual root draft')
 expect(document.querySelector('[data-world-host]')).toBe(host);expect(f.stats.mounts).toBe(mounts);expect(f.stats.peakGames).toBe(1);expect(f.factories()).toBe(created)
})
it('actual default Link and POP retain an uncertain write key/draft with exactly one POST',async()=>{
 const f=setup('/feed',true,true);await screen.findByText(post.text)
 await act(()=>f.router.navigate('/feed/new'));fireEvent.change(await screen.findByLabelText('Post text'),{target:{value:'Keep uncertain root post'}})
 fireEvent.click(screen.getByRole('button',{name:'Publish post'}));await screen.findByRole('button',{name:'Check command status'})
 const sent=f.requests.filter(r=>r.method==='POST'),key=sent[0]!.headers.get('Idempotency-Key');expect(key).toBeTruthy()
 fireEvent.click(screen.getByRole('link',{name:'Inbox'}));expect(await screen.findByRole('button',{name:'Stay with current action'})).toBeVisible()
 expect(f.router.state.location.pathname).toBe('/feed/new');expect(screen.getByLabelText('Post text')).toHaveValue('Keep uncertain root post')
 fireEvent.click(screen.getByRole('button',{name:'Stay with current action'}));await act(()=>f.router.navigate(-1))
 expect(await screen.findByRole('button',{name:'Stay with current action'})).toBeVisible();expect(f.router.state.location.pathname).toBe('/feed/new')
 expect(f.requests.filter(r=>r.method==='POST')).toHaveLength(1);expect(sent[0]!.headers.get('Idempotency-Key')).toBe(key);expect(f.stats.peakGames).toBe(1)
 act(()=>{f.auth.privacy.changePolicy(f.scope,'consent-revoked')});await waitFor(()=>expect(screen.queryByLabelText('Post text')).not.toBeInTheDocument());expect(f.stats.games).toBe(0)
})
it('C03 capability alone does not fabricate social producer readiness',async()=>{
 const f=setup('/feed',false);expect(await screen.findByText('This section is unavailable for the current account or event.')).toBeVisible()
 expect(f.requests).toHaveLength(0);expect(f.factories()).toBe(0)
})
