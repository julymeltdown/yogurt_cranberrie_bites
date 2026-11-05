// @vitest-environment jsdom
import '@testing-library/jest-dom/vitest'
import { afterEach, expect, it, vi } from 'vitest'
import { act, cleanup, fireEvent, render, screen, within } from '@testing-library/react'
import type { MotionItem, MotionSelection } from '../../../../entities/avatar/runtime/motion-adoption/art'
import type { MotionEditorController, MotionEditorState } from '../../motion-adoption/controller'
import { MotionEditor } from '../../motion-adoption/MotionEditor'

// Presentation tests use a controlled store and replace only unchanged drawing adapters.
// These fixtures are not reviewed artwork or evidence of production persistence/runtime acceptance.
vi.mock('../../motion-adoption/MotionView', () => ({ MotionView: ({ label }: { label: string }) => <span role="img" aria-label={label} /> }))
vi.mock('../../motion-adoption/MotionOption', () => ({ MotionOption: ({ label, pressed, disabled, onChoose, part }: {
  label: string; pressed: boolean; disabled: boolean; onChoose(): void; part?: { kind: string; key: string }
}) => <button type="button" data-part-kind={part?.kind} data-part-key={part?.key} aria-label={`Choose ${label}`} aria-pressed={pressed} disabled={disabled} onClick={onChoose}>{label}</button> }))
afterEach(cleanup)

function fixture() {
  let current = true
  const selection: MotionSelection = { mode: 'wardrobe', body: 'body-0', hair: 'hair-0', eyes: 'eyes-0', outfit: 'outfit-0', accessory: null }
  const items: MotionItem[] = (['body', 'hair', 'eyes', 'outfit', 'accessory'] as const).flatMap(kind =>
    Array.from({ length: kind === 'eyes' ? 1 : kind === 'accessory' ? 0 : 6 }, (_, index) => ({
      key: `${kind}-${index}`, kind, directions: [], skinPalette: { colors: [[0,0,0], [1,1,1], [120 + index,90,60], [255,255,255]] },
    })))
  let state: MotionEditorState<MotionSelection> = {
    phase: 'ready', draft: selection, persisted: { revision: 1, config: { ...selection, hair: 'hair-1' } },
    catalog: { items, presets: [{ id: 'fixture', label: 'Fixture preset', config: selection }],
      layers: ['body', 'eyes', 'hair', 'outfit', 'accessory'], qualification: 'TEST_ONLY' },
    candidate: null, needsReconciliation: false, notice: null,
  }
  const listeners = new Set<() => void>()
  const actions = { start: vi.fn(), stop: vi.fn(), choose: vi.fn(async () => true), random: vi.fn(async () => true),
    reset: vi.fn(), save: vi.fn(async () => true), reload: vi.fn(async () => true), retry: vi.fn(),
    selectPreset: vi.fn(), resolveConflict: vi.fn() }
  const controller = { ...actions, read: () => state, subscribe: (fn: () => void) => { listeners.add(fn); return () => { listeners.delete(fn) } },
    runtime: { isCurrent: () => current, authority: { selection: (config: MotionSelection) => config, qualification: 'TEST_ONLY', canSave: true } },
  } as unknown as MotionEditorController<MotionSelection>
  const update = (patch: Partial<typeof state>) => act(() => { state = { ...state, ...patch }; for (const listener of listeners) listener() })
  return { controller, actions, selection, update, revokeWithoutPublishing() { current = false } }
}

it('pages four real supplied choices and clamps one/zero-choice categories without losing controls', () => {
  const f = fixture(); render(<MotionEditor controller={f.controller} />)
  const options = () => within(screen.getByRole('region', { name: 'Hair' })).getAllByRole('button', { name: /^Choose hair/ })
  expect(options()).toHaveLength(4)
  fireEvent.click(screen.getByRole('button', { name: 'Next Hair choices' })); expect(options()).toHaveLength(2)
  fireEvent.click(screen.getByRole('button', { name: 'Choose hair 6' }))
  expect(f.actions.choose).toHaveBeenLastCalledWith({ ...f.selection, hair: 'hair-5' })
  fireEvent.click(screen.getByRole('button', { name: 'Eyes' }))
  expect(screen.getByRole('button', { name: 'Choose eyes 1' })).toBeEnabled()
  expect(screen.getByRole('button', { name: 'Next Eyes choices' })).toBeDisabled()
  expect(screen.getByRole('button', { name: 'Previous Eyes choices' })).toBeDisabled()
  fireEvent.click(screen.getByRole('button', { name: 'Accessory' }))
  expect(screen.getByRole('button', { name: 'Next Accessory choices' })).toBeDisabled()
  fireEvent.click(screen.getByRole('button', { name: 'None' }))
  expect(f.actions.choose).toHaveBeenLastCalledWith({ ...f.selection, accessory: null })
  expect(screen.getByRole('combobox', { name: 'Choose accessory' })).toBeEnabled()
  expect(f.actions.save).not.toHaveBeenCalled()
})

it('keeps pose, Random, Cancel and reconciliation actions attached to their existing controller', () => {
  const f = fixture(); const view = render(<MotionEditor controller={f.controller} />)
  fireEvent.click(screen.getByRole('button', { name: 'Preview walk' }))
  expect(screen.getByRole('button', { name: 'Stop walking' })).toHaveAttribute('aria-pressed', 'true')
  fireEvent.click(screen.getByRole('button', { name: 'Random character' })); expect(f.actions.random).toHaveBeenCalledOnce()
  fireEvent.click(screen.getByRole('button', { name: 'Cancel changes' })); expect(f.actions.reset).toHaveBeenCalledOnce()
  f.update({ needsReconciliation: true, notice: 'uncertain' })
  expect(screen.getByRole('button', { name: 'Save outfit' })).toBeDisabled()
  fireEvent.click(screen.getByRole('button', { name: 'Check latest saved outfit' })); expect(f.actions.reload).toHaveBeenCalledOnce()
  f.update({ candidate: { revision: 2, config: f.selection }, notice: 'review-server' })
  expect(screen.getByRole('button', { name: 'Cancel changes' })).toBeDisabled()
  fireEvent.click(screen.getByRole('button', { name: 'Keep my draft' })); expect(f.actions.resolveConflict).toHaveBeenLastCalledWith('keep-draft')
  fireEvent.click(screen.getByRole('button', { name: 'Use saved outfit' })); expect(f.actions.resolveConflict).toHaveBeenLastCalledWith('use-server')
  expect(f.actions.save).not.toHaveBeenCalled(); view.unmount(); expect(f.actions.stop).toHaveBeenCalledOnce()
})

it('uses the supplied body color and rejects a stale enabled swatch after owner withdrawal', () => {
  const f = fixture(); render(<MotionEditor controller={f.controller} />)
  fireEvent.click(screen.getByRole('button', { name: 'Skin / body' }))
  const swatch = screen.getByRole('button', { name: 'Skin tone 2' })
  expect(swatch.firstElementChild).toHaveStyle({ backgroundColor: 'rgb(121, 90, 60)' })
  fireEvent.click(swatch); expect(f.actions.choose).toHaveBeenLastCalledWith({ ...f.selection, body: 'body-1' })
  f.actions.choose.mockClear(); f.revokeWithoutPublishing(); fireEvent.click(swatch)
  expect(f.actions.choose).not.toHaveBeenCalled()
  f.update({ phase: 'unavailable', draft: null, catalog: null })
  expect(screen.queryByRole('button', { name: 'Skin tone 2' })).toBeNull()
  expect(screen.getByRole('button', { name: 'Save outfit' })).toBeDisabled()
})

it('binds each visible isolated part to its category and item across paging without changing selection actions', () => {
  const f = fixture(); render(<MotionEditor controller={f.controller} />)
  const keys = (kind: string) => within(screen.getByRole('region', { name: kind === 'body' ? 'Skin / body' : kind[0]!.toUpperCase() + kind.slice(1) }))
    .queryAllByRole('button', { name: /^Choose / }).map(button => [button.getAttribute('data-part-kind'), button.getAttribute('data-part-key')])
  expect(keys('hair')).toEqual([0,1,2,3].map(i => ['hair', `hair-${i}`]))
  fireEvent.click(screen.getByRole('button', { name: 'Next Hair choices' }))
  expect(keys('hair')).toEqual([['hair','hair-4'], ['hair','hair-5']])
  fireEvent.click(screen.getByRole('button', { name: 'Choose hair 6' }))
  expect(f.actions.choose).toHaveBeenLastCalledWith({ ...f.selection, hair: 'hair-5' })
  for (const [kind, label] of [['eyes','Eyes'], ['outfit','Outfit'], ['body','Skin / body']] as const) {
    fireEvent.click(screen.getByRole('button', { name: label }))
    expect(keys(kind)).toEqual(Array.from({ length: kind === 'eyes' ? 1 : 4 }, (_, i) => [kind, `${kind}-${i}`]))
  }
  fireEvent.click(screen.getByRole('button', { name: 'Accessory' }))
  expect(keys('accessory')).toEqual([])
  fireEvent.click(screen.getByRole('button', { name: 'None' }))
  expect(f.actions.choose).toHaveBeenLastCalledWith({ ...f.selection, accessory: null })
  expect(f.actions.save).not.toHaveBeenCalled()
})
