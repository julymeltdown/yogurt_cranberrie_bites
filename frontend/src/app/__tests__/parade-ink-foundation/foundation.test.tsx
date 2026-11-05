// @vitest-environment jsdom
import '@testing-library/jest-dom/vitest'
import { useState } from 'react'
import { createMemoryRouter, RouterProvider } from 'react-router-dom'
import { createAppRoutes } from '../../router'
import { createFixturePorts } from '../fixtures/ports'
import { Button, Field } from '../../../shared/ui/controls'
import { Dialog } from '../../../shared/ui/Dialog'
import { Tabs } from '../../../shared/ui/Tabs'
import { afterEach, expect, it, vi } from 'vitest'
import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import type { ThemeState } from '../../../features/theme/model/controller'
import { EraControls, PanelHeader } from '../../shell-chrome'
import { ParadeFooter } from '../../shell-chrome/parade-footer'
import { ConcertWindow, ConcertWindowEmbedding } from '../../../shared/ui/concert-window'

afterEach(cleanup)
it('ink glyphs remain decorative inside the existing real panel actions', () => {
  const back = vi.fn(), toggleSize = vi.fn(), toggleContent = vi.fn()
  render(<PanelHeader title="Feed" size="full" back={back} toggleSize={toggleSize} toggleContent={toggleContent} />)
  expect(screen.getByRole('heading', { name: 'Feed' })).toHaveAttribute('tabindex', '-1')
  for (const [name, callback] of [['Reduce panel', toggleSize], ['Minimize panel', toggleContent], ['Back to previous panel', back]] as const) {
    const button = screen.getByRole('button', { name })
    expect(button.querySelector('svg')).toHaveAttribute('aria-hidden', 'true')
    expect(button.querySelector('svg')).toHaveAttribute('focusable', 'false')
    fireEvent.click(button)
    expect(callback).toHaveBeenCalledOnce()
  }
  expect(screen.queryByRole('img')).not.toBeInTheDocument()
})
it('removing ornamental footer does not remove feature-owned footer actions or nested headings', () => {
  const close = vi.fn()
  render(<ConcertWindowEmbedding><ConcertWindow title="Feed" footer={<button>Load more</button>}>
    <ConcertWindow title="Post details" onClose={close}>Actual post</ConcertWindow><ParadeFooter />
  </ConcertWindow></ConcertWindowEmbedding>)
  expect(screen.getByRole('region', { name: 'Feed' })).toHaveClass('concert-window--embedded')
  expect(screen.getByRole('region', { name: 'Post details' })).not.toHaveClass('concert-window--embedded')
  expect(screen.getByRole('button', { name: 'Load more' })).toBeVisible()
  fireEvent.click(screen.getByRole('button', { name: 'Close Post details' }))
  expect(close).toHaveBeenCalledOnce()
  expect(document.querySelector('.mcr-chrome-parade-footer')).toBeNull()
})

it('shared dialog keeps busy prevention, invalid-field description, tab keyboard and return focus together', () => {
  const save = vi.fn()
  function Example() {
    const [open, setOpen] = useState(false), [tab, setTab] = useState('one')
    return <><Button onClick={() => setOpen(true)}>Open draft</Button>
      <Dialog open={open} title="Draft" onClose={() => setOpen(false)}>
        <Field label="Title" error="Enter a title." />
        <Tabs label="Audience" value={tab} onValueChange={setTab} items={[
          {value:'one',label:'Public',content:'Public audience'},
          {value:'two',label:'Private',content:'Private audience'},
        ]} /><Button busy onClick={save}>Saving</Button>
      </Dialog></>
  }
  render(<Example />)
  const opener = screen.getByRole('button', {name:'Open draft'})
  opener.focus(); fireEvent.click(opener)
  expect(screen.getByRole('textbox', {name:'Title'})).toHaveAccessibleDescription('Enter a title.')
  const first = screen.getByRole('tab', {name:'Public'})
  first.focus(); fireEvent.keyDown(first, {key:'ArrowRight'})
  expect(screen.getByRole('tab', {name:'Private'})).toHaveFocus()
  fireEvent.click(screen.getByRole('button', {name:'Saving'})); expect(save).not.toHaveBeenCalled()
  fireEvent.keyDown(document.activeElement!, {key:'Escape'}); expect(opener).toHaveFocus()
})
it('the actual default route keeps one World owner through changed headers and restores route-trigger focus', async () => {
  const fixture = createFixturePorts(), router = createMemoryRouter(createAppRoutes(fixture.ports), {initialEntries:['/world']})
  const view = render(<RouterProvider router={router} />)
  try {
    const feed = await screen.findByRole('link', {name:'Feed'})
    feed.focus(); fireEvent.click(feed)
    await screen.findByRole('heading', {name:'Feed',level:1})
    fireEvent.click(screen.getByRole('button', {name:'Expand panel'}))
    fireEvent.click(screen.getByRole('button', {name:'Reduce panel'}))
    expect(fixture.stats.mounts).toBe(1); expect(fixture.stats.connections).toBe(1)
    fireEvent.click(screen.getByRole('button', {name:'Back to previous panel'}))
    await screen.findByRole('button', {name:'World tools'})
    expect(feed).toHaveFocus(); expect(document.querySelector('.mcr-chrome-parade-footer')).toBeNull()
    expect(fixture.stats.peakGames).toBe(1); expect(fixture.stats.peakConnections).toBe(1)
  } finally { view.unmount(); router.dispose() }
})

it('Parade Ink presentation observes the selected era without selecting a new era itself', () => {
  const state: ThemeState = {selected:'REVENGE',pending:null,phase:'ready',message:'',saved:true}
  const select = vi.fn(async () => {})
  const controller = {read:() => state,subscribe:() => () => {},select}
  const {rerender} = render(<div data-mcr-theme="REVENGE"><EraControls controller={controller} /></div>)
  expect(screen.getByRole('button', {name:'Revenge'})).toHaveAttribute('aria-pressed','true')
  expect(select).not.toHaveBeenCalled()
  rerender(<div data-mcr-theme="DANGER"><EraControls controller={controller} /></div>)
  expect(screen.getByRole('button', {name:'Revenge'})).toHaveAttribute('aria-pressed','true')
  expect(select).not.toHaveBeenCalled()
  fireEvent.click(screen.getByRole('button', {name:'Parade'}))
  expect(select).toHaveBeenCalledExactlyOnceWith('PARADE')
  expect(screen.getByRole('button', {name:'Revenge'})).toHaveAttribute('aria-pressed','true')
})
