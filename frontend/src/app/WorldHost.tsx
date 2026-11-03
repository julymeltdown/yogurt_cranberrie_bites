import { useEffect, useLayoutEffect, useRef, useState } from 'react'
import { Button } from '../shared/ui'
import { scopeKey, usePorts } from './access'
import type { ActiveScope, ShellBootstrap, WorldLease } from './ports'

export function WorldHost({ scope, bootstrap, blocked, covered }: {
  scope: ActiveScope; bootstrap: ShellBootstrap; blocked: boolean; covered: boolean
}) {
  const ports = usePorts()
  const host = useRef<HTMLDivElement>(null)
  const lease = useRef<WorldLease | null>(null)
  const [failed, setFailed] = useState(false)
  const [attempt, setAttempt] = useState(0)
  const blockedRef = useRef(blocked)
  const coveredRef = useRef(covered)
  const available = bootstrap.worldAvailable && !['DRAFT', 'ARCHIVED'].includes(bootstrap.event.phase)
  blockedRef.current = blocked
  coveredRef.current = covered
  const key = scopeKey(scope)
  useLayoutEffect(() => {
    if (!ports.world || !available || !host.current) return
    const controller = new AbortController()
    const sessionSignal = ports.session.read().signal
    const element = host.current
    let owned: WorldLease | null = null
    let disposed = false
    const dispose = () => {
      if (disposed) return
      disposed = true
      controller.abort()
      lease.current = null
      owned?.dispose()
      element.replaceChildren()
    }
    sessionSignal.addEventListener('abort', dispose, { once: true })
    try {
      if (!ports.session.isCurrent(scope) || sessionSignal.aborted) dispose()
      else {
        owned = ports.world.mount(element, { scope, bootstrap, signal: controller.signal })
        // A port may synchronously trigger session invalidation during allocation.
        if (disposed) { owned.dispose(); element.replaceChildren() }
        else {
          lease.current = owned
          owned.setInputBlocked(blockedRef.current || document.hidden)
          owned.setVisible(!coveredRef.current && !document.hidden)
        }
      }
    } catch {
      dispose()
      setFailed(true)
    }
    return () => { sessionSignal.removeEventListener('abort', dispose); dispose() }
    // Scope generation owns the lifetime; panel navigation and bearer refresh do not.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [ports, key, attempt, available])
  useEffect(() => {
    const update = () => {
      lease.current?.setInputBlocked(blocked || document.hidden)
      lease.current?.setVisible(!covered && !document.hidden)
    }
    update()
    document.addEventListener('visibilitychange', update)
    return () => document.removeEventListener('visibilitychange', update)
  }, [blocked, covered])
  return <section className="shell-world" aria-label="World viewport">
    <div ref={host} className="shell-world-host" data-world-host="" inert={blocked} />
    {(!ports.world || !available || failed) && <div className="shell-world-message" role={failed ? 'alert' : undefined}>
      <p className="shell-kicker">You in world</p>
      <h2>{failed ? 'The world could not open' : 'World entry is being prepared'}</h2>
      <p>{failed ? 'You can keep browsing or try opening the world again.'
        : 'Movement and live presence are not available yet.'}</p>
      {failed && <Button variant="secondary" onClick={() => { setFailed(false); setAttempt(n => n + 1) }}>Try again</Button>}
    </div>}
  </section>
}
