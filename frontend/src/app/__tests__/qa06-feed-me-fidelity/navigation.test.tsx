// @vitest-environment jsdom
import '@testing-library/jest-dom/vitest'
import {cleanup,fireEvent,render,screen} from '@testing-library/react'
import {afterEach,expect,it,vi} from 'vitest'
import {MemoryRouter} from 'react-router-dom'
import {PanelScreen} from '../../WorldShell'
import {panelRoutes} from '../../routes'
import {EventFeed} from '../../../features/event-feed'
import {setup as feedFixture} from '../../../features/event-feed/__tests__/fixtures'
import {MeHub} from '../../../features/me'
import {participant,setup as meFixture} from '../../../features/me/__tests__/fixtures'

const disposers:(()=>void)[]=[]
afterEach(()=>{cleanup();disposers.splice(0).forEach(f=>f())})
const route=(path:string)=>panelRoutes.find(r=>r.path===path)!
it('real Feed owns one compose action while its shell fallback remains available without a feature',()=>{
 const h=feedFixture(async()=>Response.json({items:[],next_cursor:null}));disposers.push(h.dispose)
 h.controller.start();const compose=vi.fn()
 render(<MemoryRouter initialEntries={['/feed']}><PanelScreen route={route('/feed')}><EventFeed controller={h.controller} onPost={()=>{}} onLink={()=>{}} onCompose={compose}/></PanelScreen></MemoryRouter>)
 expect(screen.queryByRole('link',{name:'New post'})).toBeNull()
 fireEvent.click(screen.getByRole('button',{name:'Write a post'}));expect(compose).toHaveBeenCalledTimes(1)
})
it('real Me owns every account destination once, with secondary destinations in compact options',async()=>{
 const h=meFixture(async()=>Response.json(participant()));disposers.push(h.dispose);await h.controller.load()
 render(<MemoryRouter initialEntries={['/me']}><PanelScreen route={route('/me')}><MeHub controller={h.controller} auth={h.auth}/></PanelScreen></MemoryRouter>)
 expect(screen.queryByRole('navigation',{name:'Me sections'})).toBeNull()
 fireEvent.click(screen.getByText('Profile options',{exact:true}))
 for(const path of ['/me/avatar','/me/profile','/me/social','/me/requests','/me/settings','/me/age','/me/blocks','/me/account'])expect(document.querySelectorAll(`a[href="${path}"]`)).toHaveLength(1)
 expect(screen.getByRole('navigation',{name:'More account options'})).toBeVisible()
})
it.each(['/feed','/me'])('keeps all registered fallback destinations on %s without supplied feature content',path=>{
 render(<MemoryRouter initialEntries={[path]}><PanelScreen route={route(path)}/></MemoryRouter>)
 expect(screen.getByRole('navigation',{name:route(path).title+' sections'})).toBeVisible()
 for(const link of route(path).links!)expect(document.querySelector(`a[href="${link.path}"]`)).toBeInTheDocument()
})
it.each(['/meet','/gifts'])('keeps other feature supplementary navigation on %s',path=>{
 render(<MemoryRouter initialEntries={[path]}><PanelScreen route={route(path)}><p>Existing feature content</p></PanelScreen></MemoryRouter>)
 expect(screen.getByText('Existing feature content')).toBeVisible()
 fireEvent.click(screen.getByText('More actions'))
 expect(screen.getByRole('navigation',{name:route(path).title+' sections'})).toBeVisible()
})
