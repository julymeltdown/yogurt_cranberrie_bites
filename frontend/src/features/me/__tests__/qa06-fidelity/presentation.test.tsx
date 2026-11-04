// @vitest-environment jsdom
import '@testing-library/jest-dom/vitest'
import {act,cleanup,render,screen} from '@testing-library/react'
import {afterEach,expect,it,vi} from 'vitest'
import {MemoryRouter} from 'react-router-dom'
import {MeHub} from '../../ui/MePages'
import {ProfileActivity} from '../../ui/reference-profile/ProfileActivity'
import {participant,setup} from '../fixtures'
import {activity} from '../reference-target/data'

const disposers:(()=>void)[]=[]
afterEach(()=>{cleanup();disposers.splice(0).forEach(f=>f())})
async function fixture(){const fetcher=vi.fn(async()=>Response.json(participant())),h=setup(fetcher);disposers.push(h.dispose);await h.controller.load();return {...h,fetcher}}
it('offers an explicit feed destination for unavailable own activity without starting a read or inventing history',async()=>{
 const h=await fixture(),calls=h.fetcher.mock.calls.length
 render(<MemoryRouter><MeHub controller={h.controller} auth={h.auth}/></MemoryRouter>)
 expect(screen.getByText('Recent activity is unavailable in this connection.')).toBeVisible()
 expect(screen.getByRole('link',{name:'Open feed'})).toHaveAttribute('href','/feed')
 expect(h.fetcher).toHaveBeenCalledTimes(calls)
 expect(document.body.textContent).not.toMatch(/Lv\.|points|Reserved MCR|Joined “/)
 act(()=>h.auth.privacy.changePolicy(h.auth.privacy.capture(),'consent-revoked'))
 expect(screen.queryByRole('link',{name:'Open feed'})).toBeNull()
 expect(screen.queryByRole('heading',{name:participant().profile.display_name})).toBeNull()
})
it('keeps an authorized empty page distinct from unavailable or complete activity history',async()=>{
 const h=await fixture(),input=activity(h.auth.privacy.capture(),participant().user_id)
 render(<MemoryRouter><MeHub controller={h.controller} auth={h.auth} activity={{...input,page:{...input.page,items:[]}}}/></MemoryRouter>)
 expect(screen.getByText('No posts by this profile in the loaded feed page.')).toBeVisible()
 expect(screen.getByText(/This is a partial view of activity/)).toBeVisible()
 expect(screen.queryByRole('link',{name:'Open feed'})).toBeNull()
})
it('leaves the shared public-profile presentation free of the own-profile browse action',async()=>{
 const h=await fixture()
 render(<MemoryRouter><ProfileActivity subject={participant().user_id} privacy={h.auth.privacy}/></MemoryRouter>)
 expect(screen.getByText('Recent activity is unavailable in this connection.')).toBeVisible()
 expect(screen.queryByRole('link',{name:'Open feed'})).toBeNull()
})
