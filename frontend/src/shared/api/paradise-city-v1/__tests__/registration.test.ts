import { execFileSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import { fileURLToPath } from 'node:url'
import { describe, expect, it } from 'vitest'
import { DOMAIN_OPERATIONS, DOMAIN_QUERIES, DOMAIN_ERROR_RULES } from '../../domain-operations.generated'
import { domainOperation, matchesDomainRequest } from '../../domain'
import { notesOperations } from '../notes.generated'

describe('additive venue registry without historical rollback', () => {
  it('preserves every published operation, query and error rule from the exact immutable baseline', () => {
    const cwd = fileURLToPath(new URL('../../../../../..', import.meta.url))
    const raw = execFileSync('/usr/bin/git', ['show', '217c52c25180217b9fd1264af9b8cae52ec88a1a:frontend/src/shared/api/domain-operations.generated.ts'], {cwd})
    expect(createHash('sha256').update(raw).digest('hex')).toBe('3610456a0925aee9c4e3a32564016ea6acc26b7ab01ce536659b7333a5ddd447')
    const text = raw.toString()
    for (const [name, actual] of Object.entries({DOMAIN_OPERATIONS,DOMAIN_QUERIES,DOMAIN_ERROR_RULES})) {
      const block = text.split('export const '+name+' = {\n')[1]!.split('\n} as const')[0]!
      const original = JSON.parse('{'+block.trimEnd().replace(/,$/,'')+'}') as Record<string, unknown>
      for (const [key, value] of Object.entries(original)) expect(actual).toHaveProperty([key], value)
    }
  })
  it('registers exactly the six frozen Notes routes with required CAS query and command keys', () => {
    for (const [id, source] of Object.entries(notesOperations)) {
      const op = domainOperation(id as keyof typeof notesOperations)!
      expect(op.method).toBe(source.method); expect(op.path).toBe(source.path)
      expect(op.idempotency).toBe(source.method !== 'GET')
    }
    const op = domainOperation('withdrawVenuePlaceNote')!
    const path = '/api/concert/events/00000000-0000-4000-8000-000000000201/venue/landmarks/gate.~/notes/00000000-0000-4000-8000-000000000301'
    expect(matchesDomainRequest(op,'DELETE',path,{expected_revision:1})).toBe(true)
    expect(matchesDomainRequest(op,'DELETE',path)).toBe(false)
    expect(matchesDomainRequest(op,'DELETE',path,{expected_revision:0})).toBe(false)
    expect(matchesDomainRequest(op,'DELETE',path.replace('gate.~','..'),{expected_revision:1})).toBe(false)
  })
})
