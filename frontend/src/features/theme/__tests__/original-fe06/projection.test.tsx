// @vitest-environment jsdom
import { StrictMode, type ReactNode } from 'react'
import { act, cleanup, renderHook } from '@testing-library/react'
import { afterEach, expect, it, vi } from 'vitest'
import type { ThemeState } from '../../model/controller'
import { createThemeController } from '../../model/controller'
import { projectShellTheme } from '../../original-fe06/shellProjection'
import { useShellTheme } from '../../original-fe06/useShellTheme'
import { MapError } from '../../../../world/map/json'

function source(selected: ThemeState['selected'] = 'REVENGE') {
  let state: ThemeState = { selected, pending: null, phase: 'ready', saved: false, message: '' }
  const listeners = new Set<() => void>()
  return {
    read: () => state,
    subscribe: (listener: () => void) => { listeners.add(listener); return () => { listeners.delete(listener) } },
    publish(patch: Partial<ThemeState>) { state = { ...state, ...patch }; for (const listener of listeners) listener() },
    listeners: () => listeners.size,
  }
}
const shell = () => document.createElement('div')
afterEach(() => { cleanup(); document.body.replaceChildren() })

it('projects only committed eras through loading, fallback error and successful selection', () => {
  const theme = source(), host = shell(), release = new AbortController()
  const stop = projectShellTheme(theme, release.signal, host)
  expect(host.getAttribute('data-mcr-theme')).toBe('REVENGE')
  theme.publish({ pending: 'PARADE', phase: 'loading' })
  expect(host.getAttribute('data-mcr-theme')).toBe('REVENGE')
  theme.publish({ pending: null, phase: 'error' })
  expect(host.getAttribute('data-mcr-theme')).toBe('REVENGE')
  for (const selected of ['PARADE', 'DANGER'] as const) {
    theme.publish({ selected, phase: 'ready' })
    expect(host.getAttribute('data-mcr-theme')).toBe(selected)
  }
  stop(); expect(host.hasAttribute('data-mcr-theme')).toBe(false); expect(theme.listeners()).toBe(0)
})

it.each([null, 'preexisting'])('restores the exact original attribute %s on abort without changing motion or DOM', previous => {
  const theme = source('PARADE'), host = shell(), release = new AbortController()
  if (previous !== null) host.setAttribute('data-mcr-theme', previous)
  host.setAttribute('data-mcr-reduced-motion', 'true')
  host.append(document.createElement('canvas')); const canvas = host.firstChild
  const stop = projectShellTheme(theme, release.signal, host)
  release.abort(); theme.publish({ selected: 'DANGER' }); stop()
  expect(host.getAttribute('data-mcr-theme')).toBe(previous)
  expect(host.getAttribute('data-mcr-reduced-motion')).toBe('true')
  expect(host.firstChild).toBe(canvas); expect(theme.listeners()).toBe(0)
})

it('old emissions and cleanup cannot erase a successor, whose cleanup restores the original baseline', () => {
  const host = shell(), old = source(), next = source('DANGER')
  host.setAttribute('data-mcr-theme', 'baseline')
  const stopOld = projectShellTheme(old, new AbortController().signal, host)
  const stopNext = projectShellTheme(next, new AbortController().signal, host)
  old.publish({ selected: 'PARADE' }); stopOld()
  expect(host.getAttribute('data-mcr-theme')).toBe('DANGER')
  stopNext(); expect(host.getAttribute('data-mcr-theme')).toBe('baseline')
})

it('leaves a later external attribute writer intact during cleanup', () => {
  const host = shell(), stop = projectShellTheme(source(), new AbortController().signal, host)
  host.setAttribute('data-mcr-theme', 'external-owner'); stop()
  expect(host.getAttribute('data-mcr-theme')).toBe('external-owner')
})

it('disposed sources detach synchronously and preaborted releases never subscribe', () => {
  const theme = source(), host = shell(), release = new AbortController()
  projectShellTheme(theme, release.signal, host)
  theme.publish({ phase: 'disposed' })
  expect(theme.listeners()).toBe(0); expect(host.hasAttribute('data-mcr-theme')).toBe(false)
  release.abort(); const subscribe = vi.spyOn(theme, 'subscribe')
  projectShellTheme(theme, release.signal, host); expect(subscribe).not.toHaveBeenCalled()
})

it('cleans up a source that disposes synchronously while subscribing', () => {
  const host = shell(), unsubscribe = vi.fn(), theme = source()
  theme.publish({ phase: 'disposed' })
  const stop = projectShellTheme({ read: theme.read, subscribe: listener => { listener(); return unsubscribe } }, new AbortController().signal, host)
  expect(unsubscribe).toHaveBeenCalledOnce(); stop(); expect(unsubscribe).toHaveBeenCalledOnce()
})

it('retains the palette while the actual controller rejects an unavailable skin', async () => {
  let reject!: (reason: unknown) => void
  const write = vi.fn(), host = shell(), theme = createThemeController({ initial: 'REVENGE',
    scope: { userId: 'user', eventId: 'event' }, preferences: { read: () => null, write },
    load: () => new Promise((_resolve, fail) => { reject = fail }),
    runtime: { readSkin: () => 'REVENGE', replaceSkin: read => read(new AbortController().signal).then(() => {}) } })
  const stop = projectShellTheme(theme, new AbortController().signal, host)
  const selection = theme.select('DANGER')
  expect(theme.read().phase).toBe('loading'); expect(host.getAttribute('data-mcr-theme')).toBe('REVENGE')
  reject(new MapError('unavailable')); await selection
  expect(theme.read().phase).toBe('error'); expect(host.getAttribute('data-mcr-theme')).toBe('REVENGE')
  expect(write).not.toHaveBeenCalled(); stop(); theme.dispose()
})

it('binds one existing shell in StrictMode and replaces only its borrowed release', () => {
  const theme = source(), release = new AbortController(), successor = new AbortController()
  const wrapper = ({ children }: { children: ReactNode }) => <StrictMode><div data-mcr-shell="" data-mcr-reduced-motion="true"><canvas />{children}</div></StrictMode>
  const hook = renderHook(({ signal }) => useShellTheme(theme, signal), { wrapper, initialProps: { signal: release.signal } })
  const host = document.querySelector('[data-mcr-shell]')!, canvas = host.firstChild
  expect(theme.listeners()).toBe(1); expect(host.getAttribute('data-mcr-theme')).toBe('REVENGE')
  hook.rerender({ signal: successor.signal }); act(() => { release.abort(); theme.publish({ selected: 'DANGER' }) })
  expect(host.getAttribute('data-mcr-theme')).toBe('DANGER'); expect(host.firstChild).toBe(canvas)
  expect(host.getAttribute('data-mcr-reduced-motion')).toBe('true')
  act(() => successor.abort()); expect(host.hasAttribute('data-mcr-theme')).toBe(false)
  hook.unmount(); expect(theme.listeners()).toBe(0)
})

it('refuses missing or ambiguous shells and does not subscribe without a presentation', () => {
  const theme = source(), release = new AbortController()
  const absent = renderHook(() => useShellTheme(theme, release.signal))
  expect(theme.listeners()).toBe(0); absent.unmount()
  const wrapper = ({ children }: { children: ReactNode }) => <><div data-mcr-shell="" /><div data-mcr-shell="" />{children}</>
  const ambiguous = renderHook(() => useShellTheme(theme, release.signal), { wrapper })
  expect(theme.listeners()).toBe(0); ambiguous.unmount()
  const inert = renderHook(() => useShellTheme(null, undefined))
  expect(theme.listeners()).toBe(0); inert.unmount()
})
