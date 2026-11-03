// @vitest-environment jsdom
import '@testing-library/jest-dom/vitest'
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react'
import { afterEach, expect, it, vi } from 'vitest'
import { createMemoryRouter, RouterProvider } from 'react-router-dom'
import { createAppRoutes } from '../../router'
import { createFixturePorts } from '../fixtures/ports'
import { ConcertWindow, ConcertWindowEmbedding } from '../../../shared/ui/concert-window'
const owned: (()=>void)[]=[]
afterEach(()=>{cleanup();owned.splice(0).forEach(f=>f());vi.unstubAllGlobals()})
function mobile(){vi.stubGlobal('matchMedia',()=>({matches:true,addEventListener(){},removeEventListener(){}}))}
it('mobile default routes fill the window, retain one world through sizing and restore Back focus',async()=>{
 mobile();const fixture=createFixturePorts(),router=createMemoryRouter(createAppRoutes(fixture.ports),{initialEntries:['/world']});owned.push(()=>router.dispose());render(<RouterProvider router={router}/>)
 const opener=await screen.findByRole('link',{name:/^Feed$/}),host=document.querySelector('[data-world-host]');opener.focus();fireEvent.click(opener)
 await screen.findByRole('heading',{name:'Feed',level:1});expect(document.querySelector('.world-shell')).toHaveAttribute('data-panel-size','full');expect(fixture.stats.visible).toBe(false)
 fireEvent.click(screen.getByRole('button',{name:'Reduce panel'}));expect(document.querySelector('.world-shell')).toHaveAttribute('data-panel-size','half');expect(fixture.stats.visible).toBe(true)
 fireEvent.click(screen.getByRole('button',{name:'Minimize panel'}));expect(document.querySelector('.shell-panel-body')).not.toBeVisible()
 fireEvent.click(screen.getByRole('button',{name:'Show panel content'}));expect(document.querySelector('.shell-panel-body')).toBeVisible()
 fireEvent.click(screen.getByRole('button',{name:'Back to previous panel'}));await screen.findByRole('link',{name:/^Map$/});expect(opener).toHaveFocus();expect(document.querySelector('[data-world-host]')).toBe(host);expect(fixture.stats.mounts).toBe(1)
})
it('a new mobile route restores full content after a deliberate reduction or minimized panel',async()=>{
 mobile();const fixture=createFixturePorts(),router=createMemoryRouter(createAppRoutes(fixture.ports),{initialEntries:['/feed']});owned.push(()=>router.dispose());render(<RouterProvider router={router}/>)
 await screen.findByRole('heading',{name:'Feed',level:1});fireEvent.click(screen.getByRole('button',{name:'Reduce panel'}));await act(()=>router.navigate('/gifts'))
 expect(document.querySelector('.world-shell')).toHaveAttribute('data-panel-size','full');fireEvent.click(screen.getByRole('button',{name:'Minimize panel'}));await act(()=>router.navigate('/meet'))
 expect(document.querySelector('.world-shell')).toHaveAttribute('data-panel-size','full');expect(document.querySelector('.shell-panel-body')).toBeVisible();expect(fixture.stats.mounts).toBe(1)
})
it('embedding suppresses only the first frame; nested windows and actionable close headers remain owned by features',()=>{
 const close=vi.fn();render(<ConcertWindowEmbedding><ConcertWindow title="Feed"><ConcertWindow title="Nested content">Details</ConcertWindow></ConcertWindow><ConcertWindow title="Closable detail" onClose={close}>Detail</ConcertWindow></ConcertWindowEmbedding>)
 expect(screen.getByRole('region',{name:'Feed'})).toHaveClass('concert-window--embedded');expect(screen.getByRole('region',{name:'Nested content'})).not.toHaveClass('concert-window--embedded');expect(screen.getByRole('region',{name:'Closable detail'})).not.toHaveClass('concert-window--embedded')
 fireEvent.click(screen.getByRole('button',{name:'Close Closable detail'}));expect(close).toHaveBeenCalledOnce()
})
