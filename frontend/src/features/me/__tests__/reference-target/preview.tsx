import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { createMemoryRouter, RouterProvider } from 'react-router-dom'
import { MeHub, ProfileForm } from '../../ui/MePages'
import { PublicProfile } from '../../../profile/PublicProfile'
import type { Participant } from '../../model/contract'
import { participant, setup } from '../fixtures'
import { harness, publicIntro } from '../../../people/__tests__/fixtures/harness'
import { activity } from './data'
import './preview.css'
const params = new URLSearchParams(location.search), mode=params.get('mode')??'loaded', route=params.get('route')??'me'
let saved: Participant = participant(), patchMode='success', requests=0, finishSave: (()=>void)|null=null
saved={...saved,profile:{...saved.profile,display_name:'Synthetic listener',bio:'mcr / live music / the black parade\nSee you at the show.'}}
if(mode==='long') saved={...saved,profile:{...saved.profile,display_name:'A'.repeat(40),bio:'B'.repeat(160)}}
if(mode==='empty') saved={...saved,profile:{...saved.profile,bio:null}}
const h=setup(async input=>{
  const request=input as Request;requests++
  if(mode==='loading') return new Promise<Response>(()=>{})
  if(mode==='error') return new Response(null,{status:503})
  if(request.method==='PATCH') {
    const body=await request.json() as {display_name:string;bio:string|null}
    if(patchMode==='pending') await new Promise<void>(resolve=>{finishSave=resolve})
    if(patchMode==='conflict') return Response.json({code:'CONFLICT',message:'PRIVATE_RAW_ERROR',details:{reason:'VERSION_CONFLICT',current_revision:2}},{status:409})
    saved={...saved,revision:saved.revision+1,profile:{...saved.profile,display_name:body.display_name,bio:body.bio}}
  }
  return Response.json(saved)
})
if(mode!=='idle') {const load=h.controller.load();if(mode!=='loading') await load}
if(mode==='expired') h.auth.privacy.changePolicy(h.auth.privacy.capture(),'consent-revoked')
const input=mode==='no-activity'?undefined:activity(h.auth.privacy.capture(),saved.user_id)
const social=harness(),author=publicIntro().author
const publicActivity=mode==='no-activity'?undefined:activity(social.privacy.capture(),author.user_id)
let sharing=0,blocking=0
const router=createMemoryRouter([
  {path:'/me',element:<><MeHub controller={h.controller} auth={h.auth} {...(input?{activity:input}:{})}/><div className="fixture-root-footer"><p>64px reserved for root-owned footer · no duplicate feature artwork</p></div></>},
  {path:'/me/profile',element:<ProfileForm controller={h.controller}/>},
  {path:'/people/fixture',element:<PublicProfile controller={social.controller} userId={author.user_id} {...(mode==='actions'?{sharing:()=>{sharing++},blocking:()=>{blocking++}}:{})} {...(publicActivity?{activity:publicActivity,activityPrivacy:social.privacy}:{})}/>},
  {path:'*',element:<p role="status">Fixture destination</p>},
],{initialEntries:[route==='public'?'/people/fixture':route==='edit'?'/me/profile':'/me']})
createRoot(document.getElementById('root')!).render(<StrictMode><RouterProvider router={router}/></StrictMode>)
window.__REF_PROFILE__={controller:h.controller,requests:()=>requests,mode:(value:string)=>{patchMode=value},settle:()=>finishSave?.(),revoke:()=>h.auth.privacy.changePolicy(h.auth.privacy.capture(),'consent-revoked'),revokePublic:()=>social.privacy.changePolicy(social.privacy.capture(),'block'),publicCounts:()=>({sharing,blocking,requests:social.requests.length}),path:()=>router.state.location.pathname,dispose:()=>{router.dispose();h.dispose();social.dispose()}}
declare global {interface Window {__REF_PROFILE__:{controller:typeof h.controller;requests():number;mode(value:string):void;settle():void;revoke():void;revokePublic():void;publicCounts():{sharing:number;blocking:number;requests:number};path():string;dispose():void}}}
