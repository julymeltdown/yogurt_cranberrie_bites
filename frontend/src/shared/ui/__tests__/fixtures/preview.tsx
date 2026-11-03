// Isolated FE05 browser fixture; not registered in the application/router.
import { StrictMode, useRef, useState } from 'react'
import { createRoot } from 'react-dom/client'
import { Button, Dialog, Feedback, Field, Skeleton, Tabs, ToastRegion, type ToastMessage } from '../../index'
import '../../../styles/base.css'

function Preview() {
  const [open, setOpen] = useState(false)
  const [nested, setNested] = useState(false)
  const [sheet, setSheet] = useState(false)
  const [tab, setTab] = useState('plans')
  const [messages, setMessages] = useState<ToastMessage[]>([])
  const input = useRef<HTMLInputElement>(null)
  return <main className="foundation" style={{ padding: '1rem', maxWidth: '44rem' }}>
    <h1 style={{ fontSize: '2rem' }}>MCR primitives</h1>
    <p>Interaction fixture only. No account, network or world simulation.</p>
    <Field label="Draft title" hint="A short name for your plan." required />
    <div style={{ display: 'flex', flexWrap: 'wrap', gap: '0.75rem', marginBlock: '1rem' }}>
      <Button onClick={() => { setSheet(false); setOpen(true) }}>Open dialog</Button>
      <Button variant="secondary" onClick={() => { setSheet(true); setOpen(true) }}>Open sheet</Button>
      <Button variant="secondary" onClick={() => { setSheet(false); setNested(true); setOpen(true) }}>Open stacked dialogs</Button>
      <Button variant="danger" busy>Busy action</Button>
      <Button variant="quiet" onClick={() => setMessages([
        { id: 'draft', message: 'Your draft is still here.' },
        { id: 'error', message: 'Could not send. Try again.', tone: 'error' },
      ])}>Show notifications</Button>
    </div>
    <Tabs label="Fixture views" value={tab} onValueChange={setTab} items={[
      { value: 'plans', label: 'Plans', content: <Feedback kind="empty" title="No plans yet">Add your first plan.</Feedback> },
      { value: 'disabled', label: 'Unavailable', disabled: true, content: null },
      { value: 'updates', label: 'Updates', content: <Feedback kind="status" title="Waiting">Your request is pending.</Feedback> },
    ]} />
    <Feedback kind="error" title="Could not load" action={<Button variant="secondary">Try again</Button>}>
      Your draft has been kept.
    </Feedback>
    <Skeleton label="Loading fixture" />
    <ToastRegion messages={messages} onDismiss={id => setMessages(items => items.filter(item => item.id !== id))} />
    <Dialog open={open} title={sheet ? 'Plan sheet' : 'Plan dialog'} description="Review the plan before continuing."
      presentation={sheet ? 'sheet' : 'dialog'} initialFocusRef={input} onClose={() => setOpen(false)}>
      <Field ref={input} label="Plan name" hint="This stays in this fixture." />
      <Button onClick={() => setNested(true)}>Open confirmation</Button>
      <Dialog open={nested} title="Confirmation" onClose={() => setNested(false)}>
        <p>This is a second modal for focus-stack verification.</p>
        <Button>Confirm draft</Button>
      </Dialog>
      <div hidden><Button>Hidden action</Button></div>
      <Button disabled>Disabled action</Button>
    </Dialog>
  </main>
}

const root = document.getElementById('fixture')
if (!root) throw new Error('Missing fixture root')
createRoot(root).render(<StrictMode><Preview /></StrictMode>)
