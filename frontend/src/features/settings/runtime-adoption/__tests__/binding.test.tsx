// @vitest-environment jsdom
import { StrictMode, type ReactNode } from 'react'
import { act, cleanup, renderHook } from '@testing-library/react'
import { afterEach, expect, it, vi } from 'vitest'
import { useRuntimePreferences, type PreferencePresentation } from '../useRuntimePreferences'
import { fixture } from './fixture'
import { defaults } from '../../../me/model/contract'

let f: ReturnType<typeof fixture>
afterEach(() => { cleanup(); f?.dispose() })
const wrapper = ({ children }: { children: ReactNode }) => <StrictMode><div data-mcr-shell="">{children}</div></StrictMode>
function setup() {
  f = fixture()
  const scope = f.auth.privacy.capture(), release = new AbortController(), dispose = vi.fn(), acquire = vi.fn(() => ({ dispose }))
  const presentation: PreferencePresentation = { request: { privacyScope: scope, signal: release.signal }, theme: f.theme,
    runtimePreferences: { effects: { motion: true, lowPower: true, acquire } } }
  return { scope, release, dispose, acquire, presentation }
}
it('Root binding borrows exact original scope/release/theme and survives StrictMode without automatic effects', async () => {
  const x = setup(), hook = renderHook(() => useRuntimePreferences(f.auth, x.scope, x.presentation), { wrapper })
  expect(hook.result.current).toBeDefined(); expect(x.acquire).not.toHaveBeenCalled()
  await act(async () => { await hook.result.current!.apply(defaults, x.scope, new AbortController().signal) })
  expect(x.acquire).toHaveBeenCalledOnce()
  act(() => x.release.abort())
  expect(x.dispose).toHaveBeenCalledOnce(); expect(hook.result.current?.read().phase).toBe('disposed')
  expect(f.theme.read().phase).toBe('ready')
})
it('Root binding rejects missing capabilities, mismatched lease, and re-labelled expired policy', () => {
  const x = setup(), { runtimePreferences: _effects, ...missing } = x.presentation
  const hook = renderHook(({ value }) => useRuntimePreferences(f.auth, x.scope, value), { wrapper, initialProps: { value: missing as PreferencePresentation } })
  expect(hook.result.current).toBeUndefined()
  hook.rerender({ value: { ...x.presentation, request: { ...x.presentation.request, privacyScope: { ...x.scope, generation: x.scope.generation + 1 } } } })
  expect(hook.result.current).toBeUndefined()
  act(() => { f.auth.privacy.changePolicy(x.scope, 'block') })
  hook.rerender({ value: x.presentation }); expect(hook.result.current).toBeUndefined(); expect(x.acquire).not.toHaveBeenCalled()
})
it('Root binding disposes a replaced release without aborting or disposing the borrowed shell owners', async () => {
  const x = setup(), hook = renderHook(({ value }) => useRuntimePreferences(f.auth, x.scope, value), { wrapper, initialProps: { value: x.presentation } })
  const first = hook.result.current!
  await act(async () => { await first.apply(defaults, x.scope, new AbortController().signal) })
  hook.rerender({ value: { ...x.presentation, request: { ...x.presentation.request, signal: new AbortController().signal } } })
  expect(first.read().phase).toBe('disposed'); expect(x.dispose).toHaveBeenCalledOnce()
  expect(x.release.signal.aborted).toBe(false); expect(hook.result.current).not.toBe(first)
  hook.unmount(); expect(f.theme.read().phase).toBe('ready')
})
it('projects explicit motion on the existing shell in StrictMode and restores its baseline on caller/privacy abort', async () => {
  const x = setup(), hook = renderHook(() => useRuntimePreferences(f.auth, x.scope, x.presentation), { wrapper })
  const shell = document.querySelector('[data-mcr-shell]')!, caller = new AbortController()
  shell.setAttribute('data-mcr-reduced-motion', 'baseline')
  await act(async () => { await hook.result.current!.apply({ ...defaults, reduced_motion: true }, x.scope, caller.signal) })
  expect(shell.getAttribute('data-mcr-reduced-motion')).toBe('true')
  act(() => caller.abort()); expect(shell.getAttribute('data-mcr-reduced-motion')).toBe('baseline')
  await act(async () => { await hook.result.current!.apply({ ...defaults, reduced_motion: true }, x.scope, new AbortController().signal) })
  act(() => { f.auth.privacy.changePolicy(x.scope, 'block') })
  expect(shell.getAttribute('data-mcr-reduced-motion')).toBe('baseline')
  expect(hook.result.current!.read().phase).toBe('disposed')
})
it('does not expose Root live controls without one unambiguous existing shell', () => {
  const x = setup(), hook = renderHook(() => useRuntimePreferences(f.auth, x.scope, x.presentation))
  expect(hook.result.current).toBeUndefined()
  expect(x.acquire).not.toHaveBeenCalled()
})
