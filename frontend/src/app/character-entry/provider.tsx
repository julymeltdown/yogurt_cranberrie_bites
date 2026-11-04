import { createContext, useContext, useLayoutEffect, useState, type ReactNode } from 'react'
import type { WholeCharacterRuntime } from '../../entities/avatar/whole-character'
import type { PrivacyScope } from '../../shared/query/privacy'
import type { ShellBootstrap } from '../ports'
import { usePorts } from '../access'
import type { ReviewedCharacterOwner } from './reviewed-registry'

const Context = createContext<WholeCharacterRuntime | null>(null)
const ReviewedContext = createContext<ReviewedCharacterOwner | null>(null)
/** Views borrow the application registry's runtime; route effects never destroy it. */
export default function CharacterProvider({ bootstrap, scope, children }: {
  readonly bootstrap: ShellBootstrap; readonly scope: PrivacyScope | null; readonly children: ReactNode
}) {
  const { auth, characters } = usePorts()
  const [mounted, setMounted] = useState<{ auth: typeof auth; characters: typeof characters; bootstrap: ShellBootstrap; scope: PrivacyScope; runtime: WholeCharacterRuntime | null; reviewed: ReviewedCharacterOwner | null } | null>(null)
  useLayoutEffect(() => {
    if (!auth || !characters || !scope || !auth.privacy.isCurrent(scope)) return
    let detached = false
    const manifest = bootstrap.document?.avatar_manifest as Record<string, unknown> | undefined
    if (manifest?.representation === 'reviewed_selected_source_motion') {
      void characters.admitReviewed(scope, bootstrap).then(reviewed => {
        if (!detached && auth.privacy.isCurrent(scope)) setMounted({ auth, characters, bootstrap, scope, runtime: null, reviewed })
      })
    } else {
      const runtime = characters.get(scope, bootstrap)
      setMounted({ auth, characters, bootstrap, scope, runtime, reviewed: null })
    }
    return () => { detached = true }
  }, [auth, characters, scope, bootstrap])
  const aligned = mounted?.auth === auth && mounted?.characters === characters && mounted?.bootstrap === bootstrap && mounted.scope === scope
  const runtime = aligned && scope && auth?.privacy.isCurrent(scope) && mounted?.runtime?.isCurrent() ? mounted.runtime : null
  const reviewed = aligned && scope && auth?.privacy.isCurrent(scope) && mounted?.reviewed?.isCurrent() ? mounted.reviewed : null
  // Admit before mounting identity consumers or scheduling the default World load.
  // A rejected art release still opens the shell with honest unavailable portraits.
  if (bootstrap.document?.avatar_manifest && ['whole_character', 'reviewed_selected_source_motion'].includes(String((bootstrap.document.avatar_manifest as Record<string, unknown>).representation))
    && auth && characters && scope && auth.privacy.isCurrent(scope) && !aligned) return <p role="status">Opening your character…</p>
  return <Context value={runtime}><ReviewedContext value={reviewed}>{children}</ReviewedContext></Context>
}
export function useCharacterRuntime() {
  const runtime = useContext(Context)
  return runtime?.isCurrent() ? runtime : null
}

export function useReviewedCharacterOwner() {
  const owner = useContext(ReviewedContext)
  return owner?.isCurrent() ? owner : null
}
