import { createContext, useContext, useLayoutEffect, useState, type ReactNode } from 'react'
import type { WholeCharacterRuntime } from '../../entities/avatar/whole-character'
import type { PrivacyScope } from '../../shared/query/privacy'
import type { ShellBootstrap } from '../ports'
import { usePorts } from '../access'

const Context = createContext<WholeCharacterRuntime | null>(null)
/** Views borrow the application registry's runtime; route effects never destroy it. */
export default function CharacterProvider({ bootstrap, scope, children }: {
  readonly bootstrap: ShellBootstrap; readonly scope: PrivacyScope | null; readonly children: ReactNode
}) {
  const { auth, characters } = usePorts()
  const [mounted, setMounted] = useState<{ auth: typeof auth; characters: typeof characters; bootstrap: ShellBootstrap; scope: PrivacyScope; runtime: WholeCharacterRuntime | null } | null>(null)
  useLayoutEffect(() => {
    if (!auth || !characters || !scope || !auth.privacy.isCurrent(scope)) return
    const runtime = characters.get(scope, bootstrap)
    setMounted({ auth, characters, bootstrap, scope, runtime })
  }, [auth, characters, scope, bootstrap])
  const aligned = mounted?.auth === auth && mounted?.characters === characters && mounted?.bootstrap === bootstrap && mounted.scope === scope
  const runtime = aligned && scope && auth?.privacy.isCurrent(scope) && mounted?.runtime?.isCurrent() ? mounted.runtime : null
  // Admit before mounting identity consumers or scheduling the default World load.
  // A rejected art release still opens the shell with honest unavailable portraits.
  if (bootstrap.document?.avatar_manifest && (bootstrap.document.avatar_manifest as Record<string, unknown>).representation === 'whole_character'
    && auth && characters && scope && auth.privacy.isCurrent(scope) && !aligned) return <p role="status">Opening your character…</p>
  return <Context value={runtime}>{children}</Context>
}
export function useCharacterRuntime() {
  const runtime = useContext(Context)
  return runtime?.isCurrent() ? runtime : null
}
