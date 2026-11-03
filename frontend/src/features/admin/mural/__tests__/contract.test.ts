import { describe, expect, it } from 'vitest'
import mural from '../../../../../../contracts/domain/v1/mural.schema.json'
import admin from '../../../../../../contracts/domain/v1/admin.schema.json'
import operations from '../../../../../../contracts/domain/v1/operations.json'
import policy from '../../../../../../contracts/domain/v1/policy.json'
import complete from '../../../../../../contracts/fixtures/domain/closure-complete.json'
import falseSuccess from '../../../../../../contracts/fixtures/domain/closure-false-success.json'
import emptyRegion from '../../../../../../contracts/fixtures/domain/empty-mural-region.json'
import { boundsSchema, controlSchema, lockSchema, muralStateSchema, restoreSchema } from '../contract'
import { closeEventSchema, closureJobSchema } from '../../closure/contract'
describe('pinned C04 FE39 DTO parity', () => {
  it('closed DTO keys match owning definitions; authored schemas add declared policy invariants', () => {
    for (const [schema, definition] of [[controlSchema, mural.$defs.MuralControl], [restoreSchema, mural.$defs.RestoreMural], [lockSchema, mural.$defs.RegionLock],
      [boundsSchema, mural.$defs.RegionBounds], [muralStateSchema, mural.$defs.MuralAdminState], [closeEventSchema, admin.$defs.CloseEvent.oneOf[0]!], [closureJobSchema, admin.$defs.ClosureJob]] as const) {
      expect(Object.keys(schema.shape).sort()).toEqual([...definition.required].sort()); expect(definition.additionalProperties).toBe(false)
    }
  })
  it('source fixtures accept complete progress and reject false completion and empty region', () => {
    expect(closureJobSchema.safeParse(complete).success).toBe(true); expect(closureJobSchema.safeParse(falseSuccess).success).toBe(false); expect(boundsSchema.safeParse(emptyRegion).success).toBe(false)
  })
  it('pins operations to exact OPERATOR role and registered requests without inventing a role endpoint', () => {
    for (const id of ['getConcertAdminMural','controlConcertMural','lockConcertMuralRegion','restoreConcertMural','closeConcertEvent','getConcertClosure']) {
      const op = operations.find(o => o.operationId === id)!
      expect(op.actor).toBe('OPERATOR'); expect(op.cache_control).toBe('no-store')
      if (op.method === 'post') { expect(op.idempotency_key).toBe(true); expect(op.success_status).toBe(200); expect(op.pending_status).toBe(202) }
    }
    expect(policy.rules.mural_restore.preconditions).toContain('Mural PAUSED')
  })
})
