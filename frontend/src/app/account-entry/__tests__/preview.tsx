import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { createBrowserRouter, RouterProvider } from 'react-router-dom'
import { createSessionController } from '../../../features/auth/model/session'
import { createFixturePorts } from '../../__tests__/fixtures/ports'
import { eventId } from '../../../features/auth/__tests__/fixtures/data'
import { createAppRoutes } from '../../router'
import '../../../shared/styles/base.css'
import '../../shell.css'
if (!import.meta.env.DEV) throw new Error('Synthetic account fixture is development-only')
const auth=createSessionController({eventId}),fixture=createFixturePorts()
auth.start()
await new Promise<void>(resolve=>{if(auth.read().capabilities!=='loading')resolve();else{const stop=auth.subscribe(()=>{if(auth.read().capabilities!=='loading'){stop();resolve()}})}})
if(!await auth.login('synthetic@example.org','fixture-password'))throw new Error('Synthetic login unavailable')
const candidate=new URL(location.href).searchParams.get('route'),initial=['/world','/me','/me/profile','/me/settings','/me/avatar'].includes(candidate??'')?candidate!:'/world'
history.replaceState(null,'',initial)
const router=createBrowserRouter(createAppRoutes({...fixture.ports,auth,session:auth.session,readiness:auth.readiness,bootstrap:{load:auth.loadBootstrap}}))
window.__ACCOUNT__={stats:fixture.stats,navigate:path=>router.navigate(path),revoke:()=>auth.privacy.changePolicy(auth.privacy.capture(),'consent-revoked'),
  changeIdentity:()=>auth.session.replace(auth.session.read().generation,{userId:'00000000-0000-4000-8000-000000000999',eventId,accessToken:'NEXT_SYNTHETIC_ONLY'}),
  dispose(){router.dispose();auth.dispose()}}
declare global{interface Window{__ACCOUNT__:{stats:typeof fixture.stats;navigate(path:string):Promise<void>;revoke():boolean;changeIdentity():unknown;dispose():void}}}
createRoot(document.getElementById('root')!).render(<StrictMode><RouterProvider router={router}/></StrictMode>)
