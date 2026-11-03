import { useLayoutEffect, useRef, useState } from 'react'
import { createRoot } from 'react-dom/client'
import { ConcertAction, ConcertIcon, ConcertNavigation, ConcertRow, ConcertState, ConcertTabs, ConcertWindow } from '../index'
import type { ConcertIconName } from '../index'
import './preview.css'

const destinations = ['World', 'Feed', 'Meet', 'Gifts', 'Wall', 'Me'] as const

function Preview() {
  const [open, setOpen] = useState(true)
  const [tab, setTab] = useState('parties')
  const [state, setState] = useState('loaded')
  const [notes, setNotes] = useState(false)
  const reopen = useRef<HTMLButtonElement>(null)
  useLayoutEffect(() => { if (!open) reopen.current?.focus() }, [open])
  function close() { setOpen(false) }
  const rows = <ul className="concert-rows">
    <ConcertRow title="Area A queue group" metadata="Example party · 4 of 8 places" visual={<ConcertIcon name="meet" />}
      action={<ConcertAction tone="paper" onClick={() => setNotes(true)}>Details</ConcertAction>}>Meeting details require current permission.</ConcertRow>
    <ConcertRow title="After-show hangout" metadata="Example party · applications paused" visual={<ConcertIcon name="star" />}>
      <span>Read-only example</span>
    </ConcertRow>
    <ConcertRow title="Photo walk · Parade era" metadata="Example party · 3 of 6 places" visual={<ConcertIcon name="world" />}>
      Virtual presence does not confirm arrival.
    </ConcertRow>
  </ul>
  const content = state === 'empty'
    ? <ConcertState title="No parties here yet">Try another filter when the owning feature is available.</ConcertState>
    : state === 'unavailable'
      ? <ConcertState kind="unavailable" title="Party service unavailable">This example cannot load current parties. No action was sent.</ConcertState>
      : state === 'pending'
        ? <ConcertState kind="pending" title="Waiting for confirmation">An owner must supply the actual outcome. Pending does not mean joined.</ConcertState>
        : rows

  return <main className="preview">
    <div className="preview__label">UIV01 / COMPONENT PREVIEW</div>
    <p className="preview__intro">Synthetic examples. No event or account connection.</p>
    <div className="preview__tools">
      <label htmlFor="preview-state">Example state</label>
      <select id="preview-state" value={state} onChange={e => { setState(e.target.value); setNotes(false) }}>
        <option value="loaded">Loaded</option><option value="empty">Empty</option>
        <option value="unavailable">Unavailable</option><option value="pending">Pending action</option>
      </select>
    </div>
    <ConcertAction ref={reopen} tone="paper" disabled={open} onClick={() => setOpen(true)}>Open concert window</ConcertAction>
    {open && <ConcertWindow title="Meet" icon="meet" headingLevel={1} onClose={close}
      footer={<><ConcertAction pending={state === 'pending'} disabled={state === 'unavailable'} onClick={() => setNotes(value => !value)}
        aria-expanded={notes} aria-controls="layout-notes">{state === 'pending' ? 'Awaiting owner response' : 'Layout notes'}</ConcertAction>
        <p id="layout-notes" role="status" hidden={!notes}>Local presentation only. Details and layout notes do not apply, join, reserve or send a request.</p></>}>
      <ConcertTabs label="Meet sections" value={tab} onValueChange={setTab} items={[
        { value: 'parties', label: 'Parties', content },
        { value: 'people', label: 'People', content: <ConcertState title="No example introductions">Only authorized people would appear here.</ConcertState> },
        { value: 'plan', label: 'My plan', content: <ConcertState kind="unavailable" title="Account connection required">This component preview has no private plan data.</ConcertState> },
      ]} />
    </ConcertWindow>}
    <ConcertNavigation label="Preview destinations" current="meet" items={destinations.map(label => ({
      id: label.toLowerCase(), label, icon: label.toLowerCase() as ConcertIconName, href: `#example-${label.toLowerCase()}`,
    }))} />
    <aside className="preview__notes" aria-label="Presentation examples">
      <h2>Same frame. Different content.</h2>
      <p>No art substitutes, invented providers or decorative account stats.</p>
      <section id="example-world"><h3>World</h3><p>Shell adoption preserves one existing game and connection.</p></section>
      <section id="example-feed"><h3>Feed</h3><p>Compact author and content rows use authorized projections.</p></section>
      <section id="example-meet"><h3>Meet</h3><p>Application and approval remain owning-feature actions.</p></section>
      <section id="example-gifts"><h3>Gifts</h3><p>Stock and pickup credentials remain server-authorized.</p></section>
      <section id="example-wall"><h3>Wall</h3><p>Preview and confirmed pixel placement stay distinct.</p></section>
      <section id="example-me"><h3>Me</h3><p>Only approved character derivatives may fill the visual slot.</p></section>
    </aside>
  </main>
}

createRoot(document.getElementById('root')!).render(<Preview />)
