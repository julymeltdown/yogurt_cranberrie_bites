// @vitest-environment jsdom
import '@testing-library/jest-dom/vitest'
import { afterEach, expect, it, vi } from 'vitest'
import { act, cleanup, render, screen } from '@testing-library/react'
import { FramedWholeCharacterView } from '../../../../app/character-entry/FramedWholeCharacterView'
import type { WholeCharacterRuntime } from '../../../../entities/avatar/whole-character'

afterEach(() => { cleanup(); vi.restoreAllMocks() })
const config = { contract_version: '1.1.0', kind: 'avatar_config', manifest_id: 'mcr_selected_static_20261103',
  manifest_version: 1, representation: 'whole_character', preset_id: 'photo01_front_01' } as const

// The real unchanged renderer is mounted. Only its runtime port and canvas drawing are controlled.
// This verifies presentation/lifetime, not approved artwork or production authorization.
function runtimePort() {
  const listeners = new Set<() => void>()
  const dispose = vi.fn(), unsubscribe = vi.fn()
  const acquire = vi.fn(async (_input: unknown, _options: { signal: AbortSignal; detail?: boolean }) => ({
    read: () => ({ image: {}, source: { x: 0, y: 0, width: 64, height: 64 } }),
    subscribe: (listener: () => void) => { listeners.add(listener); return unsubscribe },
    dispose,
  }))
  vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockReturnValue({
    clearRect: vi.fn(), drawImage: vi.fn(), imageSmoothingEnabled: false,
  } as unknown as CanvasRenderingContext2D)
  return { runtime: { acquire } as unknown as WholeCharacterRuntime, acquire, dispose, unsubscribe,
    withdraw: () => act(() => { for (const listener of listeners) listener() }) }
}

it.each([64, 128] as const)('frames size %s without replacing the renderer or restarting its lease', async size => {
  const f = runtimePort()
  const view = render(<FramedWholeCharacterView runtime={f.runtime} config={config} label="Current portrait" size={size} className="supplied-child" />)
  await act(async () => { await Promise.resolve() })
  const canvas = screen.getByRole('img', { name: 'Current portrait' })
  expect(canvas).toHaveAttribute('width', String(size))
  expect(canvas.parentElement).toHaveClass('supplied-child')
  expect(screen.getByRole('group', { name: 'Current portrait' })).toHaveAttribute('data-portrait-size', String(size))
  expect(f.acquire).toHaveBeenCalledExactlyOnceWith(config, expect.objectContaining({ detail: false }))
  view.rerender(<FramedWholeCharacterView runtime={f.runtime} config={config} label="Updated portrait" size={size} className="supplied-child" />)
  expect(screen.getByRole('img', { name: 'Updated portrait' })).toBe(canvas)
  expect(f.acquire).toHaveBeenCalledTimes(1)
  f.withdraw()
  expect(screen.queryByRole('img')).toBeNull()
  expect(screen.getByRole('status')).toHaveTextContent('Character unavailable')
  view.unmount()
  expect(f.acquire.mock.calls[0]![1].signal.aborted).toBe(true)
  expect(f.unsubscribe).toHaveBeenCalledOnce()
  expect(f.dispose).toHaveBeenCalledOnce()
})

it('preserves the unframed 256px detail request', async () => {
  const f = runtimePort()
  const view = render(<FramedWholeCharacterView runtime={f.runtime} config={config} label="Detail" size={256} detail />)
  await act(async () => { await Promise.resolve() })
  expect(screen.queryByRole('group')).toBeNull()
  expect(screen.getByRole('img', { name: 'Detail' })).toHaveAttribute('width', '256')
  expect(f.acquire).toHaveBeenCalledExactlyOnceWith(config, expect.objectContaining({ detail: true }))
  view.unmount()
  expect(f.dispose).toHaveBeenCalledOnce()
})
