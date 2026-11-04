import type { ThemeController } from '../model/controller'
import { isThemeId } from '../model/preferences'

export type ShellTheme = Pick<ThemeController, 'read' | 'subscribe'>
const attribute = 'data-mcr-theme'
interface Projection { readonly previous: string | null; last: string }
const owners = new WeakMap<HTMLElement, Projection>()

/** Borrow the existing shell. Pending artwork never changes its committed palette.
 * This lease owns only its attribute, never the controller or world lifetime. */
export function projectShellTheme(theme: ShellTheme, signal: AbortSignal, shell: HTMLElement) {
  let closed = false, owned: Projection | null = null, unsubscribe = () => {}
  function restore() {
    const previous = owned; owned = null
    if (!previous || owners.get(shell) !== previous) return
    owners.delete(shell)
    if (shell.getAttribute(attribute) !== previous.last) return
    if (previous.previous === null) shell.removeAttribute(attribute)
    else shell.setAttribute(attribute, previous.previous)
  }
  function stop() {
    if (closed) return
    closed = true; unsubscribe(); signal.removeEventListener('abort', stop); restore()
  }
  function render() {
    if (closed) return
    const state = theme.read()
    if (state.phase === 'disposed') { stop(); return }
    if (!isThemeId(state.selected)) return
    if (!owned) {
      const incumbent = owners.get(shell)
      owned = { previous: incumbent ? incumbent.previous : shell.getAttribute(attribute), last: '' }
      owners.set(shell, owned)
    }
    if (owners.get(shell) !== owned) return
    owned.last = state.selected
    shell.setAttribute(attribute, owned.last)
  }
  if (signal.aborted) return stop
  signal.addEventListener('abort', stop, { once: true })
  unsubscribe = theme.subscribe(render)
  // A synchronous subscriber may close the lease before returning its cleanup.
  if (closed) unsubscribe()
  else render()
  return stop
}
