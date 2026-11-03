// @vitest-environment jsdom
import '@testing-library/jest-dom/vitest'
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react'
import { afterEach, expect, it, vi } from 'vitest'
import { MemoryRouter } from 'react-router-dom'
import { MeHub, ProfileForm } from '../../ui/MePages'
import { ProfileActivity, qualifiedProfilePosts } from '../../ui/reference-profile/ProfileActivity'
import { participant, setup } from '../fixtures'
import { activity } from './data'
const disposers: (() => void)[] = []
afterEach(() => { cleanup(); disposers.splice(0).forEach(f=>f()) })
async function fixture() { const h=setup(vi.fn(async()=>Response.json(participant()))); disposers.push(h.dispose); await h.controller.load(); return h }
it('shows two real-schema own posts only in the partial event-page view; does not fetch on presentation', async()=>{
  const h=await fixture(), input=activity(h.auth.privacy.capture(),participant().user_id)
  const renderPosts=()=>render(<MemoryRouter><MeHub controller={h.controller} auth={h.auth} activity={input}/></MemoryRouter>)
  const view=renderPosts()
  expect(screen.getAllByRole('link',{name:/Posted to the feed/})).toHaveLength(2)
  expect(screen.getByText(/This is a partial view of activity/)).toBeVisible()
  expect(screen.getByText('12 reactions · 3 replies',{exact:false})).toBeVisible()
  expect(screen.queryByText(/Lv\.|Reserved MCR|Joined “/)).toBeNull()
  expect(view.container.querySelector('details')?.open).toBe(false)
  expect(screen.getByRole('link',{name:'Manage private social links'})).toHaveAttribute('href','/me/social')
  expect(screen.getByRole('link',{name:'Sharing requests'})).toHaveAttribute('href','/me/requests')
  expect(screen.getByRole('link',{name:'My gifts'})).toHaveAttribute('href','/gifts/mine')
  act(()=>h.auth.privacy.changePolicy(h.auth.privacy.capture(),'consent-revoked'))
  expect(screen.queryByRole('heading',{name:participant().profile.display_name})).toBeNull()
  expect(screen.queryByRole('link',{name:/Posted to the feed/})).toBeNull()
})
it('rejects foreign subject/event/lease and unexpected private fields without showing their text',async()=>{
  const h=await fixture(), subject=participant().user_id, input=activity(h.auth.privacy.capture(),subject)
  expect(qualifiedProfilePosts({...input,subjectUserId:'00000000-0000-4000-8000-000000000002'},subject,h.auth.privacy)).toBeNull()
  expect(qualifiedProfilePosts({...input,scope:{...input.scope,policyGeneration:99}},subject,h.auth.privacy)).toBeNull()
  expect(qualifiedProfilePosts({...input,page:{...input.page,items:input.page.items.map(p=>({...p,event_id:'00000000-0000-4000-8000-000000000001'}))}},subject,h.auth.privacy)).toBeNull()
  const bad={...input,page:{...input.page,items:input.page.items.map(p=>({...p,private_handle:'PRIVATE_PAYLOAD'}))}}
  render(<MemoryRouter><ProfileActivity input={bad} subject={subject} privacy={h.auth.privacy}/></MemoryRouter>)
  expect(screen.getByText('Recent activity is unavailable in this connection.')).toBeVisible();expect(document.body.textContent).not.toContain('PRIVATE_PAYLOAD')
})
it('filters other authors; empty current page never claims an empty history',async()=>{
  const h=await fixture(), subject=participant().user_id, input=activity(h.auth.privacy.capture(),subject)
  const foreign={...input,page:{...input.page,items:input.page.items.map(p=>({...p,author:{...p.author,user_id:'00000000-0000-4000-8000-000000000002'}}))}}
  render(<MemoryRouter><ProfileActivity input={foreign} subject={subject} privacy={h.auth.privacy}/></MemoryRouter>)
  expect(screen.getByText('No posts by this profile in the loaded feed page.')).toBeVisible()
  expect(screen.queryByRole('link')).toBeNull()
})
it('retains saved identity, honest art absence, current draft cancellation and no private handle links',async()=>{
  const h=await fixture();h.controller.editProfile({display_name:'Unsaved draft'})
  const view=render(<MemoryRouter><MeHub controller={h.controller} auth={h.auth}/></MemoryRouter>)
  expect(screen.getByRole('heading',{name:participant().profile.display_name})).toBeVisible();expect(screen.queryByText('Unsaved draft')).toBeNull()
  expect(view.container.querySelector('img,canvas')).toBeNull();expect(screen.getByText(/Avatar preview unavailable/)).toBeVisible()
  expect(screen.getByText('Recent activity is unavailable in this connection.')).toBeVisible()
  view.rerender(<MemoryRouter><ProfileForm controller={h.controller}/></MemoryRouter>)
  fireEvent.change(screen.getByLabelText('Bio (optional)'),{target:{value:'Local draft'}})
  fireEvent.click(screen.getByRole('button',{name:'Cancel profile changes'}))
  expect(screen.getByLabelText('Display name')).toHaveValue(participant().profile.display_name)
  expect(screen.getByLabelText('Bio (optional)')).toHaveValue('')
})
