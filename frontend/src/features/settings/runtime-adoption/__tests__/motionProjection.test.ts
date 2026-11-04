// @vitest-environment jsdom
import { afterEach, expect, it, vi } from 'vitest'
import { fixture } from './fixture'
import { projectRuntimeMotion } from '../motionProjection'
import { defaults } from '../../../me/model/contract'

const cleanups: (() => void)[] = []
afterEach(() => { cleanups.splice(0).reverse().forEach(fn => fn()) })
function setup(baseline: string | null = null) {
  const callbacks = new Set<() => void>(); let matches = false
  const f = fixture({ get matches() { return matches }, addEventListener: (_type, fn) => { callbacks.add(fn) },
    removeEventListener: (_type, fn) => { callbacks.delete(fn) } })
  const shell = document.createElement('div'), signal = new AbortController(), scope = f.auth.privacy.capture()
  if (baseline !== null) shell.setAttribute('data-mcr-reduced-motion', baseline)
  const stop = projectRuntimeMotion(f.adapter, shell)
  cleanups.push(() => { stop(); f.dispose() })
  return { ...f, shell, signal, scope, stop,
    os(value: boolean) { matches = value; [...callbacks].forEach(fn => fn()) },
    attr: () => shell.getAttribute('data-mcr-reduced-motion') }
}
it('projects explicit motion before the actual theme selection and restores the exact baseline after caller abort', async () => {
  const f = setup('false'), select = vi.spyOn(f.theme, 'select')
  select.mockImplementation(async () => { expect(f.attr()).toBe('true') })
  expect(f.attr()).toBe('false') // Constructing/subscribing does not apply.
  await f.adapter.apply({ ...defaults, reduced_motion: true }, f.scope, f.signal.signal)
  expect(select).toHaveBeenCalledOnce(); expect(f.attr()).toBe('true')
  f.signal.abort(); expect(f.attr()).toBe('false')
})
it('ORs OS and explicit motion and updates without restarting audio', async () => {
  const f = setup()
  await f.adapter.apply(defaults, f.scope, f.signal.signal)
  expect(f.attr()).toBe('false')
  f.os(true); await vi.waitFor(() => expect(f.attr()).toBe('true'))
  f.os(false); await vi.waitFor(() => expect(f.attr()).toBe('false'))
  expect(f.read().soundCalls).toBe(0)
  await f.adapter.apply({ ...defaults, reduced_motion: true }, f.scope, f.signal.signal)
  f.os(false); await vi.waitFor(() => expect(f.attr()).toBe('true'))
  f.adapter.dispose(); expect(f.attr()).toBeNull()
})
it('restores missing/baseline attributes immediately on privacy disposal and failed theme application', async () => {
  const f = setup()
  await f.adapter.apply({ ...defaults, reduced_motion: true }, f.scope, f.signal.signal)
  f.revoke(); expect(f.attr()).toBeNull()
  const failed = setup('baseline')
  await expect(failed.adapter.apply({ ...defaults, theme: 'PARADE', reduced_motion: true }, failed.scope, failed.signal.signal)).rejects.toMatchObject({ reason: 'failed' })
  expect(failed.attr()).toBe('baseline')
})
it('does not erase another DOM writer during old borrower cleanup', async () => {
  const f = setup('baseline')
  await f.adapter.apply({ ...defaults, reduced_motion: true }, f.scope, f.signal.signal)
  f.shell.setAttribute('data-mcr-reduced-motion', 'external')
  f.stop(); f.adapter.dispose()
  expect(f.attr()).toBe('external')
})
it('fences a reentrant abort during applied publication and restores DOM without reporting success', async () => {
  const f = setup()
  f.adapter.subscribe(() => { if (f.adapter.read().phase === 'applied') f.signal.abort() })
  await expect(f.adapter.apply({ ...defaults, reduced_motion: true }, f.scope, f.signal.signal)).rejects.toMatchObject({ reason: 'cancelled' })
  expect(f.attr()).toBeNull(); expect(f.read().activeEffects).toBe(0)
})
