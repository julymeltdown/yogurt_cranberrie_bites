import '../../features/account/parade-ink-private/private.css'
import { useLayoutEffect } from 'react'
import { useLocation } from 'react-router-dom'
import { AccountPanel } from '../../features/account/panel'
import { useRetainedAccount } from './context'
export default function AccountControlsPanel() {
  const { owner, controller } = useRetainedAccount(), location = useLocation()
  useLayoutEffect(() => { owner?.acquire(); return () => { owner?.read().controller?.editConfirmation('') } }, [owner, location.key])
  if (!owner || !controller) return <div className="pi-private-account"><p role="status">Account controls require your current canonical session.</p></div>
  return <div className="pi-private-account-entry"><AccountPanel auth={owner.auth} controller={controller} onBack={() => owner.back(location.state)} /></div>
}
