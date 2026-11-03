// @vitest-environment jsdom
import { act } from 'react'
import { afterEach, expect, it, vi } from 'vitest'
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { createThemeController, type ThemeOptions } from '../model/controller'
import { createLocalThemePreferences, type ThemeId } from '../model/preferences'
import { ThemeSelector } from '../ui/ThemeSelector'
import { ThemeScope } from '../ui/ThemeScope'
import { MapError } from '../../../world/map/json'
import type { LoadedMap } from '../../../world/map/resources'
function setup(overrides: Partial<ThemeOptions> = {}) {
  let skin = 'REVENGE'
  const preferences = { read: vi.fn(() => null), write: vi.fn() }
  const runtime = { readSkin: () => skin, replaceSkin: vi.fn(async (load: (signal: AbortSignal) => Promise<LoadedMap>, signal?: AbortSignal) => {
    const map = await load(signal ?? new AbortController().signal)
    if (signal?.aborted) throw new MapError('cancelled')
    skin = map.theme.theme_id
  }) }
  const load = vi.fn(async (theme: ThemeId) => ({ theme: { theme_id: theme }, dispose: vi.fn() }) as unknown as LoadedMap)
  const controller = createThemeController({ initial: 'REVENGE', scope: { userId: 'user-a', eventId: 'event-a' }, preferences, runtime, load, ...overrides })
  return { controller, preferences, runtime, load }
}
afterEach(cleanup)
it('scopes persistent selection by both identity and event and rejects delimiter aliasing', () => {
  const values = new Map<string, string>(), p = createLocalThemePreferences({ getItem: key => values.get(key) ?? null, setItem: (key, value) => { values.set(key, value) } })
  p.write({ userId: 'a:b', eventId: 'c' }, 'DANGER')
  expect(p.read({ userId: 'a', eventId: 'b:c' })).toBeNull()
  expect(p.read({ userId: 'a:b', eventId: 'c' })).toBe('DANGER')
  expect(p.read({ userId: 'b', eventId: 'c' })).toBeNull()
  expect(() => p.read({ userId: '', eventId: 'c' })).toThrow()
})
it('persists only after skin commit and restores once without overriding user intent', async () => {
  const s = setup({ preferences: { read: () => 'PARADE', write: vi.fn() } })
  await s.controller.restore(); expect(s.controller.read().selected).toBe('PARADE')
  await s.controller.select('DANGER'); await s.controller.restore()
  expect(s.controller.read().selected).toBe('DANGER'); expect(s.load).toHaveBeenCalledTimes(2)
})
it('ignores invalid stored values and exposes storage failure with retry', async () => {
  const s = setup({ preferences: { read: () => 'arbitrary_url', write: () => { throw new Error('disabled') } } })
  await s.controller.restore(); expect(s.load).not.toHaveBeenCalled()
  await s.controller.select('PARADE'); expect(s.controller.read()).toMatchObject({ selected: 'PARADE', phase: 'error', saved: false })
  await s.controller.select('PARADE'); expect(s.load).toHaveBeenCalledOnce()
})
it('preserves prior selection and preference on unavailable or failed assets', async () => {
  const s = setup({ load: async () => { throw new MapError('unavailable') } })
  await s.controller.select('DANGER')
  expect(s.controller.read()).toMatchObject({ selected: 'REVENGE', pending: null, phase: 'error' })
  expect(s.controller.read().message).toContain('artwork is unavailable'); expect(s.preferences.write).not.toHaveBeenCalled()
})
it('reports the actual committed fallback after a superseded visual commit and later load failure', async () => {
  let skin = 'REVENGE'
  const s = setup({ runtime: { readSkin: () => skin, replaceSkin: async () => { skin = 'PARADE'; throw new MapError('load-failed') } } })
  await s.controller.select('DANGER')
  expect(s.controller.read()).toMatchObject({ selected: 'PARADE', phase: 'error', saved: false })
  expect(s.controller.read().message).toContain('PARADE remains selected'); expect(s.preferences.write).not.toHaveBeenCalled()
})
it('rejects a loader returning a different era and releases it', async () => {
  const dispose = vi.fn(), s = setup({ load: async () => ({ theme: { theme_id: 'PARADE' }, dispose }) as unknown as LoadedMap })
  await s.controller.select('DANGER'); expect(dispose).toHaveBeenCalledOnce(); expect(s.preferences.write).not.toHaveBeenCalled()
})
it.each(['supersede', 'dispose'] as const)('does not persist ignored-abort results after %s', async action => {
  let resolve!: (v: LoadedMap) => void
  const pending = new Promise<LoadedMap>(r => { resolve = r }), s = setup({ load: async theme => theme === 'PARADE' ? pending : ({ theme: { theme_id: theme }, dispose() {} }) as LoadedMap })
  const a = s.controller.select('PARADE')
  if (action === 'supersede') await s.controller.select('DANGER'); else s.controller.dispose()
  resolve({ theme: { theme_id: 'PARADE' } } as LoadedMap); await a
  expect(s.preferences.write.mock.calls.map(call => call[1])).toEqual(action === 'supersede' ? ['DANGER'] : [])
})
it('renders a native radio group, pending and error feedback; scope follows only committed theme', async () => {
  let reject!: (e: unknown) => void
  const s = setup({ load: () => new Promise((_resolve, r) => { reject = r }) })
  render(<ThemeScope controller={s.controller} reducedMotion><ThemeSelector controller={s.controller} /></ThemeScope>)
  expect(screen.getByRole('radio', { name: 'REVENGE' }).getAttribute('checked')).not.toBeNull()
  fireEvent.click(screen.getByRole('radio', { name: 'DANGER' }))
  expect(screen.getByRole('status').textContent).toContain('Loading DANGER')
  expect((screen.getByRole('radio', { name: 'DANGER' }) as HTMLInputElement).checked).toBe(true)
  expect(document.querySelector('[data-mcr-theme]')?.getAttribute('data-mcr-theme')).toBe('REVENGE')
  await act(async () => { reject(new Error('load failed')) })
  await waitFor(() => expect(screen.getByRole('status').textContent).toContain('REVENGE remains selected'))
  expect(document.querySelector('[data-mcr-reduced-motion]')?.getAttribute('data-mcr-reduced-motion')).toBe('true')
})
it('exposes stored-preference read failures without touching the world', async () => {
  const s = setup({ preferences: { read() { throw new Error('denied') }, write() {} } })
  await s.controller.restore(); expect(s.controller.read().message).toContain('Could not read'); expect(s.load).not.toHaveBeenCalled()
})
