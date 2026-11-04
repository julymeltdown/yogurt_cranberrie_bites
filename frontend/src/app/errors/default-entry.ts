import type { SessionController } from '../../features/auth/model/session'
import type { WorldAccessBindingPort } from '../ports'
import { createBrowserConnectivity } from './connectivity'
import { createConnectionObserver, type ConnectionObserverOptions, type RecoveryAction, type ServiceDecision, type ValuePort } from './observer'

/** One application-lifetime presentation owner. No connection or policy producer. */
export function createDefaultConnectionStatus(options: {
  auth: Pick<SessionController, 'session' | 'privacy'>
  world?: WorldAccessBindingPort | undefined
  browser: Parameters<typeof createBrowserConnectivity>[0]
  reload(): void
  scheduler?: ConnectionObserverOptions['scheduler']
  pendingMs?: number
}) {
  const connectivity = createBrowserConnectivity(options.browser)
  const world: WorldAccessBindingPort = options.world ?? { read: () => null, subscribe: () => () => {} }
  const recovery: ValuePort<RecoveryAction | null> = {
    read() {
      const captured = world.read()
      return captured?.reconnect ? { release: captured.release, reconnect: () => captured.reconnect!() } : null
    },
    // The observer already subscribes to this immutable binding's lifetime.
    subscribe: () => () => {},
  }
  const service: ValuePort<ServiceDecision | null> = {
    read() {
      const captured = world.read(), status = captured?.lifecycle?.read()
      return captured && status?.release === captured.release && (status.kind === 'version' || status.kind === 'unavailable')
        ? { release: captured.release, kind: status.kind } : null
    },
    // The observer subscribes to the captured binding's lifecycle alongside replication.
    subscribe: () => () => {},
  }
  const controller = createConnectionObserver({ auth: options.auth, world, connectivity, recovery, service,
    ...(options.scheduler ? { scheduler: options.scheduler } : {}),
    ...(options.pendingMs === undefined ? {} : { pendingMs: options.pendingMs }),
  })
  let started = false, disposed = false
  return { controller, reload: options.reload,
    start() { if (started || disposed) return; started = true; connectivity.start(); controller.start() },
    dispose() { if (disposed) return; disposed = true; controller.dispose(); connectivity.dispose() },
  }
}
