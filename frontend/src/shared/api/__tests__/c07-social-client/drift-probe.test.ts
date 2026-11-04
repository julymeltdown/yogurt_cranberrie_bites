import { it } from 'vitest'
import { domainOperation } from '../../domain'
import type { DomainOperationId } from '../../domain-operations.generated'
import { assertOperations, readJSON } from '../../../../../e2e/real/social/cohort.mjs'
it('external drift sentinel: an injected public method mismatch must exit nonzero', () => {
  const operations = readJSON('contracts/domain/v1/operations.json')
  if (process.env.MCR_C07_DRIFT_PROBE === 'method') operations.find((row: { operationId: string }) => row.operationId === 'createConcertSocialRequest').method = 'delete'
  assertOperations(id => domainOperation(id as DomainOperationId), operations)
})
