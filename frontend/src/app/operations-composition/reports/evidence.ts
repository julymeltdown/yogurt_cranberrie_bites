import type { SessionController } from '../../../features/auth/model/session'
import type { EvidencePort } from '../../../features/admin/reports'
import { canonicalUuid } from '../../../features/auth/model/contract'
import type { PrivacyScope } from '../../../shared/query/privacy'
import { DOMAIN_OPERATIONS, type DomainOperationId } from '../../../shared/api/domain-operations.generated'
import { domainOperation } from '../../../shared/api/domain'
import { inspectImage, IMAGE_TYPES, MAX_IMAGE_BYTES } from '../../../shared/media/image-policy'
import { sameOperationsScope } from '../../operations-entry/contract'

// Actual producer ABI: Report03de / Media213 source142a8d648c73a014f78071897ad1cb7b499af531.
// Root owns registration. Its absence disables this byte path, never the report queue.
export const REPORT_EVIDENCE_OPERATION = 'readConcertReportEvidence'
export const reportEvidencePath = (event: string, report: string, media: string) =>
  `/api/concert/events/${event}/admin/reports/${report}/evidence/${media}/content`
export function registeredReportEvidenceOperation(): DomainOperationId | null {
  const operation = Object.keys(DOMAIN_OPERATIONS).find((id): id is DomainOperationId => id === REPORT_EVIDENCE_OPERATION)
  const registered = domainOperation(operation)
  return operation && registered?.method === 'GET'
    && registered.path === reportEvidencePath('{eventId}', '{reportId}', '{mediaId}')
    && registered.query === 'common.EmptyRequest' && !registered.idempotency
    && registered.success.length === 1 && registered.success[0] === 200 ? operation : null
}

export function createReportEvidence(options: {
  auth: Pick<SessionController, 'session' | 'privacy' | 'request'>
  scope: PrivacyScope
  signal: AbortSignal
  current(): boolean
}): EvidencePort {
  const { auth, scope } = options
  function check(signal: AbortSignal) {
    if (signal.aborted || !options.current() || !auth.privacy.isCurrent(scope) || !auth.session.isCurrent(scope)) {
      throw Error('Report evidence unavailable')
    }
  }
  return Object.freeze({ async open(input: Parameters<EvidencePort['open']>[0]) {
    const signal = AbortSignal.any([options.signal, input.signal])
    check(signal)
    if (!sameOperationsScope(input.scope, scope) || !canonicalUuid(input.reportId) || !canonicalUuid(input.mediaId)) {
      throw Error('Report evidence context unavailable')
    }
    // Discover the actual generated registration without inventing a member of an older union.
    const operation = registeredReportEvidenceOperation()
    if (!operation) throw Error('Report evidence producer unavailable')
    const path = reportEvidencePath(scope.eventId!, input.reportId, input.mediaId)
    return auth.privacy.run(scope, async ({ signal: privateSignal }) => {
      check(privateSignal)
      const result = await auth.request({ method: 'GET', path, access: 'protected', errorFamily: 'domain',
        domainOperation: operation, signal: privateSignal,
        binaryResponse: { maxBytes: MAX_IMAGE_BYTES, contentTypes: [...IMAGE_TYPES] },
        decode(value, context) {
          check(privateSignal)
          if (context.status !== 200 || context.path !== path || context.scope.generation !== scope.generation
            || context.scope.userId !== scope.userId || context.scope.eventId !== scope.eventId
            || !(value instanceof Blob)) throw Error('Invalid report evidence')
          return value
        } })
      check(privateSignal)
      await inspectImage(result.data, privateSignal)
      check(privateSignal)
      // The retained feature controller allocates/revokes its own URL after another role check.
      return Object.freeze({ mediaId: input.mediaId, blob: result.data })
    }, signal)
  } })
}
