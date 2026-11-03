// @vitest-environment jsdom
import '@testing-library/jest-dom/vitest'
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react'
import { afterEach, expect, it, vi } from 'vitest'
import { MemoryRouter } from 'react-router-dom'
import { PublicProfile } from '../../PublicProfile'
import { harness, publicIntro } from '../../../people/__tests__/fixtures/harness'
import { activity } from '../../../me/__tests__/reference-target/data'
const disposers:(()=>void)[]=[]
afterEach(()=>{cleanup();disposers.splice(0).forEach(f=>f())})
it('keeps consent callbacks explicit and private activity disabled; clears public content on revoke',async()=>{
  const h=harness();disposers.push(h.dispose);const author=publicIntro().author, sharing=vi.fn(), blocking=vi.fn()
  const view=render(<MemoryRouter><PublicProfile controller={h.controller} userId={author.user_id} sharing={sharing} blocking={blocking} activity={activity(h.privacy.capture(),author.user_id)} activityPrivacy={h.privacy}/></MemoryRouter>)
  await screen.findByRole('heading',{name:author.display_name})
  expect(screen.getAllByRole('link',{name:/Posted to the feed/})).toHaveLength(2)
  expect(screen.getByRole('button',{name:'Parties'})).toBeDisabled();expect(screen.getByRole('button',{name:'Gifts'})).toBeDisabled()
  expect(screen.queryByText('Instagram')).toBeNull();expect(view.container.querySelector('img,canvas')).toBeNull()
  fireEvent.click(screen.getByRole('button',{name:'Request sharing'}));expect(sharing).toHaveBeenCalledExactlyOnceWith(author.user_id)
  fireEvent.click(screen.getByRole('button',{name:'Block user'}));expect(blocking).toHaveBeenCalledExactlyOnceWith(author.user_id)
  expect(screen.getByRole('link',{name:'Report profile'})).toHaveAttribute('href',`/report?targetType=USER&targetId=${author.user_id}`)
  act(()=>h.privacy.changePolicy(h.privacy.capture(),'block'))
  expect(screen.queryByRole('heading',{name:author.display_name})).toBeNull();expect(screen.queryByRole('link',{name:/Posted to the feed/})).toBeNull()
})
it('never enables callbacks or invents portrait/activity when dependencies are missing',async()=>{
  const h=harness();disposers.push(h.dispose);const author=publicIntro().author
  render(<MemoryRouter><PublicProfile controller={h.controller} userId={author.user_id}/></MemoryRouter>)
  await screen.findByRole('heading',{name:author.display_name})
  expect(screen.getByRole('button',{name:'Request sharing'})).toBeDisabled();expect(screen.getByRole('button',{name:'Block user'})).toBeDisabled()
  expect(screen.getByText('Recent activity is unavailable in this connection.')).toBeVisible()
  expect(screen.getByRole('img',{name:`${author.display_name}'s avatar: avatar unavailable`})).toBeVisible()
})
