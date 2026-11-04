import type { RuntimePreferencesAdapter } from '.'

const attribute = 'data-mcr-reduced-motion'
interface Projection { readonly previous: string | null; last: string }
const owners = new WeakMap<HTMLElement, Projection>()

/** Borrow one existing shell element; no CSS, wrapper, settings storage or runtime.
 * Project before the pending theme commit, and restore only this borrower's write. */
export function projectRuntimeMotion(adapter: Pick<RuntimePreferencesAdapter, 'read' | 'subscribe'>, shell: HTMLElement) {
  let closed = false, owned: Projection | null = null
  function restore() {
    const previous = owned; owned = null
    if (!previous || owners.get(shell) !== previous) return
    owners.delete(shell)
    // A later external writer owns its value. Never erase it during old cleanup.
    if (shell.getAttribute(attribute) !== previous.last) return
    if (previous.previous === null) shell.removeAttribute(attribute)
    else shell.setAttribute(attribute, previous.previous)
  }
  function render() {
    if (closed) return
    const state = adapter.read()
    if (state.phase !== 'applying' && state.phase !== 'applied') { restore(); return }
    if (!owned) {
      const incumbent = owners.get(shell)
      owned = { previous: incumbent ? incumbent.previous : shell.getAttribute(attribute), last: '' }
      owners.set(shell, owned)
    }
    if (owners.get(shell) !== owned) return
    owned.last = state.reducedMotion ? 'true' : 'false'
    shell.setAttribute(attribute, owned.last)
  }
  const unsubscribe = adapter.subscribe(render)
  render()
  return () => { if (closed) return; closed = true; unsubscribe(); restore() }
}
