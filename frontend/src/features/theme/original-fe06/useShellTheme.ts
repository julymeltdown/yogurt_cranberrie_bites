import { useLayoutEffect } from 'react'
import { projectShellTheme, type ShellTheme } from './shellProjection'
import '../../../shared/styles/themes/themes.css'

/** Subscribe to the incumbent presentation; retain the existing shell DOM tree. */
export function useShellTheme(theme: ShellTheme | null | undefined, signal: AbortSignal | undefined) {
  useLayoutEffect(() => {
    if (!theme || !signal || signal.aborted) return
    const shells = document.querySelectorAll<HTMLElement>('[data-mcr-shell]')
    if (shells.length !== 1) return
    return projectShellTheme(theme, signal, shells[0]!)
  }, [theme, signal])
}
