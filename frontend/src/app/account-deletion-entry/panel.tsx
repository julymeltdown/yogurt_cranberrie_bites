import { useLayoutEffect } from 'react'
import { useLocation } from 'react-router-dom'
import { AccountPanel } from '../../features/account/panel'
import { useRetainedAccount } from './context'
export default function AccountControlsPanel() {
  const { owner, controller } = useRetainedAccount(), location = useLocation()
  useLayoutEffect(() => { owner?.acquire(); return () => { owner?.read().controller?.editConfirmation('') } }, [owner, location.key])
  if (!owner || !controller) return <p role="status">Account controls require your current canonical session.</p>
  return <AccountPanel auth={owner.auth} controller={controller} onBack={() => owner.back(location.state)} />
}
