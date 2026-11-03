import { createContext, useContext, useMemo, type ReactNode } from 'react'
const Ownership = createContext({ places: true, mural: true })
/** Explicit route composition retains its matching caller-owned provider; defaults own theirs. */
export function WorldPanelOwnership({ places, mural, children }: { places: boolean; mural: boolean; children: ReactNode }) {
  const value = useMemo(() => ({ places, mural }), [places, mural])
  return <Ownership value={value}>{children}</Ownership>
}
export const useWorldPanelOwnership = () => useContext(Ownership)
