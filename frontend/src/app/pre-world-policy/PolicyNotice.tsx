import { Link } from 'react-router-dom'
import { Button, Feedback } from '../../shared/ui'
import type { PolicyView } from './model'
import { PreWorldFrame } from './parade-ink/PreWorldFrame'

export function PolicyNotice({ view, retry, reload }: { view: PolicyView; retry: () => void; reload: () => void }) {
  const pending = view.kind === 'idle' || view.kind === 'checking'
  const maintenance = view.kind === 'maintenance'
  const required = view.kind === 'version'
  const title = pending ? 'Checking event availability' : maintenance ? 'World entry is paused'
    : required ? 'An app update is required' : 'Event availability could not be confirmed'
  return <PreWorldFrame title={title} actions={<>
    {required && <Button onClick={reload}>Reload app</Button>}
    {!pending && <Button variant="secondary" onClick={retry}>Check again</Button>}
    <Link className="shell-link" to="/">Back to home</Link>
  </>}>
    <Feedback kind={pending ? 'status' : 'error'} title="Your destination is saved">
      <p>{pending ? 'Checking the current service policy before opening your event.'
        : maintenance ? 'The event service is undergoing maintenance. Check again before entering.'
          : required ? `This event requires app version ${view.minimumClientVersion} or later. Reload the app, then check again.`
            : 'We could not obtain a current service decision. No World has been opened.'}</p>
    </Feedback>
  </PreWorldFrame>
}
