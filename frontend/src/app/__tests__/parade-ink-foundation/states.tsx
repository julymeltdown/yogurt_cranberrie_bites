// Supplemental component states only; this is not the default App milestone.
import { useState } from 'react'
import { createRoot } from 'react-dom/client'
import { Button, Field } from '../../../shared/ui/controls'
import { Dialog } from '../../../shared/ui/Dialog'
import { Tabs } from '../../../shared/ui/Tabs'
import { Feedback, Skeleton } from '../../../shared/ui/feedback'
import '../../../shared/styles/base.css'
function States() {
  const [tab, setTab] = useState('all'), [open, setOpen] = useState(false)
  return <main style={{ padding: 12 }}>
    <h1>Shared control states · test only</h1>
    <Tabs label="Fixture tabs" value={tab} onValueChange={setTab} items={[
      {value:'all',label:'ALL',content:<p>All content</p>},
      {value:'next',label:'NEXT',content:<p>Next content</p>},
      {value:'disabled',label:'UNAVAILABLE',disabled:true,content:null},
    ]} />
    <p><Button onClick={() => setOpen(true)}>Open sheet</Button></p>
    <p><Button busy>Saving · pending</Button></p>
    <p><Button disabled>Unavailable action</Button></p>
    <Field label="Draft" hint="Retained locally in this test." error="Enter a title." defaultValue="" />
    <Feedback kind="error" title="Could not save" action={<Button>Try again</Button>}>Your draft remains.</Feedback>
    <Skeleton label="Loading items" />
    <Dialog open={open} presentation="sheet" title="Details" onClose={() => setOpen(false)}>
      <Field label="Sheet draft" /><Button>Save draft</Button>
    </Dialog>
  </main>
}
createRoot(document.getElementById('root')!).render(<States />)
