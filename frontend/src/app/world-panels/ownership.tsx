import { createContext, useContext, useMemo, type ReactNode } from 'react'
const Ownership = createContext({ places: true, mural: true, social: true })
/** Explicit route composition retains its matching caller-owned provider; defaults own theirs. */
export function WorldPanelOwnership({ places, mural, social, children }: { places: boolean; mural: boolean; social: boolean; children: ReactNode }) {
  const value = useMemo(() => ({ places, mural, social }), [places, mural, social])
  return <Ownership value={value}>{children}</Ownership>
}
export const useWorldPanelOwnership = () => useContext(Ownership)
