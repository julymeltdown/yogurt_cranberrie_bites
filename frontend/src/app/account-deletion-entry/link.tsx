import { useLocation } from 'react-router-dom'
import { ShellLink } from '../navigation'
import { useAccountDeletionEntry } from './context'
export function AccountControlsLink() {
  const owner = useAccountDeletionEntry(), location = useLocation()
  return <ShellLink id="settings-account-controls" className="shell-link" to="/me/account" onClick={event => {
    if (!event.defaultPrevented && event.button === 0 && !event.metaKey && !event.ctrlKey && !event.shiftKey && !event.altKey) owner?.rememberSettings(location.key)
  }}>Account controls</ShellLink>
}
