// @vitest-environment jsdom
import '@testing-library/jest-dom/vitest'
import { afterEach, expect, it, vi } from 'vitest'
import { act, cleanup, fireEvent, render, screen, within } from '@testing-library/react'
import type { WholeCharacterRuntime } from '../../../../entities/avatar/whole-character'
import type { WholeCharacterEditorController, WholeEditorState } from '../../whole-character/controller'
import { WholeCharacterEditor } from '../../whole-character/WholeCharacterEditor'

// The existing drawing adapter is replaced only in these controlled editor presentation cases.
vi.mock('../../../../entities/avatar/whole-character', async original => ({
  ...await original<object>(),
  WholeCharacterView: ({ label }: { label: string }) => <span role="img" aria-label={label} />,
}))
afterEach(cleanup)
function fixture() {
  const choices = Array.from({ length: 6 }, (_, index) => ({
    id: `photo01_front_0${index + 1}` as WholeEditorState['choices'][number]['id'], label: `Character ${index + 1}`,
    config: { contract_version: '1.1.0', kind: 'avatar_config', manifest_id: 'mcr_selected_static_20261103',
      manifest_version: 1, representation: 'whole_character', preset_id: `photo01_front_0${index + 1}` } as WholeEditorState['choices'][number]['config'],
  }))
  let state: WholeEditorState = { phase: 'ready', choices, draft: choices[0]!.config,
    persisted: { revision: 1, config: choices[1]!.config }, candidate: null, needsReconciliation: false, notice: null }
  const listeners = new Set<() => void>()
  const actions = { start: vi.fn(), stop: vi.fn(), random: vi.fn(), reset: vi.fn(), select: vi.fn(),
    save: vi.fn(async () => true), reload: vi.fn(async () => true), retry: vi.fn(), resolveConflict: vi.fn() }
  const controller = { ...actions, runtime: {}, read: () => state,
    subscribe: (fn: () => void) => { listeners.add(fn); return () => { listeners.delete(fn) } },
  } as unknown as WholeCharacterEditorController<WholeCharacterRuntime>
  return { controller, actions, choices, update: (patch: Partial<WholeEditorState>) => act(() => {
    state = { ...state, ...patch }; for (const listener of listeners) listener()
  }) }
}
it('retains three-choice paging, visible supplied labels, actions and controller lifetime', () => {
  const f = fixture(), view = render(<WholeCharacterEditor controller={f.controller} />)
  const options = () => within(screen.getByRole('group', { name: 'Whole character choices' })).getAllByRole('button')
  expect(options()).toHaveLength(3)
  expect(screen.getByText('Character 1')).toHaveClass('mcr-whole-choice-label')
  fireEvent.click(screen.getByRole('button', { name: 'Next characters' }))
  expect(screen.getByRole('button', { name: 'Choose Character 4' })).toBeEnabled()
  fireEvent.click(screen.getByRole('button', { name: 'Choose Character 4' }))
  expect(f.actions.select).toHaveBeenLastCalledWith('photo01_front_04')
  f.update({ choices: [f.choices[0]!] })
  expect(options()).toHaveLength(1)
  expect(screen.getByRole('button', { name: 'Previous characters' })).toHaveAttribute('aria-disabled', 'true')
  fireEvent.click(screen.getByRole('button', { name: 'Previous characters' }))
  expect(options()).toHaveLength(1)
  f.update({ choices: f.choices })
  fireEvent.click(screen.getByRole('button', { name: 'Random character' }))
  fireEvent.click(screen.getByRole('button', { name: 'Reset draft' }))
  fireEvent.click(screen.getByRole('button', { name: 'Save character' }))
  expect(f.actions.random).toHaveBeenCalledOnce()
  expect(f.actions.reset).toHaveBeenCalledOnce()
  expect(f.actions.save).toHaveBeenCalledOnce()
  view.unmount()
  expect(f.actions.start).toHaveBeenCalledOnce(); expect(f.actions.stop).toHaveBeenCalledOnce()
})
it('retains unknown-save recovery, both reconciliation actions and withdrawal', () => {
  const f = fixture(); render(<WholeCharacterEditor controller={f.controller} />)
  f.update({ needsReconciliation: true, notice: 'uncertain' })
  expect(screen.getByRole('button', { name: 'Save character' })).toBeDisabled()
  fireEvent.click(screen.getByRole('button', { name: 'Check latest saved character' }))
  expect(f.actions.reload).toHaveBeenCalledOnce()
  f.update({ candidate: { revision: 2, config: f.choices[1]!.config }, notice: 'review-server' })
  expect(screen.getByRole('button', { name: 'Reset draft' })).toBeDisabled()
  fireEvent.click(screen.getByRole('button', { name: 'Keep my draft' }))
  expect(f.actions.resolveConflict).toHaveBeenLastCalledWith('keep-draft')
  fireEvent.click(screen.getByRole('button', { name: 'Use saved character' }))
  expect(f.actions.resolveConflict).toHaveBeenLastCalledWith('use-server')
  f.update({ phase: 'unavailable', draft: null, choices: [], candidate: null, notice: null, needsReconciliation: false })
  expect(screen.queryByRole('group', { name: 'Whole character choices' })).toBeNull()
  expect(screen.getByRole('button', { name: 'Save character' })).toBeDisabled()
  expect(screen.getByRole('status')).toHaveTextContent('unavailable')
  expect(f.actions.save).not.toHaveBeenCalled()
})
