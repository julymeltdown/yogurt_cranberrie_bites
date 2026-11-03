import { useSyncExternalStore, type ReactNode } from 'react'
import type { ThemeController } from '../model/controller'
/** Wrap the existing shell; this component never creates a world or network owner. */
export function ThemeScope({ controller, reducedMotion = false, children }: { controller: ThemeController; reducedMotion?: boolean; children: ReactNode }) {
  const state = useSyncExternalStore(controller.subscribe, controller.read, controller.read)
  return <div className="mcr-theme-scope" data-mcr-theme={state.selected} data-mcr-reduced-motion={reducedMotion ? 'true' : 'false'}>{children}</div>
}
