import { expect, it } from 'vitest'
import schema from '../../../../../../contracts/domain/v1/common.schema.json'
import { DOMAIN_ERROR_RULES } from '../../domain-operations.generated'
it('matches every generated error rule to the actual canonical properties, required fields and hint bounds', () => {
  const projected: Readonly<Record<string, unknown>> = DOMAIN_ERROR_RULES
  const branches = schema.$defs.DomainError.properties.details.oneOf
  // Standalone venue operations add reasons outside the retained C04 common schema.
  const venueOnly = ['NOTE_CAPACITY', 'REVISION_OR_KEY_CONFLICT', 'LOCATION_SHARING_OFF', 'LOCATION_STALE',
    'LOCATION_ACCURACY_UNAVAILABLE', 'LOCATION_REQUEST_EXPIRED', 'LOCATION_REQUEST_TERMINAL']
  expect(Object.keys(projected).sort()).toEqual([...branches.map(branch => branch.properties.reason.const), ...venueOnly].sort())
  for (const reason of venueOnly) expect(projected[reason], reason).toBeNull()
  for (const branch of branches) {
    const props = branch.properties
    const expected = 'retry_after_ms' in props ? { ...props.retry_after_ms, required: branch.required.includes('retry_after_ms') } : null
    expect(projected[props.reason.const], props.reason.const).toEqual(expected)
  }
})
