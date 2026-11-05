import type { SessionController } from '../../features/auth/model/session'
import { DOMAIN_OPERATIONS, type DomainOperationId } from '../../shared/api/domain-operations.generated'
import { domainOperation } from '../../shared/api/domain'
import { TransportError } from '../../shared/api/errors'
import { ShellLoadError } from '../ports'
import { servicePolicyDecoder, servicePolicyOperation, servicePolicyPath } from './contract'
import type { PolicyApplicationRelease, PreWorldPolicyPort } from './model'

function registered(value: string): value is DomainOperationId {
  return Object.hasOwn(DOMAIN_OPERATIONS, value)
}
/** Uses only the actual Root-generated operation. Missing old148 registration fails closed;
 * this module never patches a registry or creates an HTTP/Auth/session owner. */
export function createPolicyTransport(auth: SessionController,
  release: () => PolicyApplicationRelease | null): PreWorldPolicyPort {
  return {
    release,
    async read(binding, signal) {
      const current = () => !signal.aborted && auth.session.isCurrent(binding.scope) && auth.privacy.isCurrent(binding.scope)
      const operation: string = servicePolicyOperation
      if (!current() || !registered(operation)) throw new ShellLoadError('unavailable')
      const descriptor = domainOperation(operation)
      if (!descriptor || descriptor.method !== 'GET' || descriptor.path !== '/api/concert/events/{eventId}/service-policy'
        || descriptor.idempotency || !descriptor.success.includes(200)) throw new ShellLoadError('unavailable')
      try {
        const result = await auth.privacy.run(binding.scope, context => auth.request({
          method: 'GET', path: servicePolicyPath(binding.scope.eventId), access: 'protected', errorFamily: 'domain',
          domainOperation: operation, query: { client_version: binding.release.applicationVersion, client_release: binding.release.buildId },
          decode: servicePolicyDecoder(binding, current), signal: context.signal, timeoutMs: 8_000,
        }), signal)
        if (!current()) throw new ShellLoadError('unavailable')
        return result.data
      } catch (error) {
        // Only closed transport categories affect access UI. No body text or status-derived policy.
        const status = error instanceof TransportError ? error.status : null
        throw new ShellLoadError(status === 401 ? 'unauthenticated' : status === 403 ? 'denied'
          : status === 404 ? 'not-found' : status === 429 ? 'rate-limited' : 'unavailable')
      }
    },
  }
}
