import assert from 'node:assert/strict'
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs'
import { join } from 'node:path'
import { tmpdir } from 'node:os'
import test from 'node:test'
import { admitOriginal, readRunConfig } from './admission.mjs'

test('missing original inputs remain NOT_RUN without requiring an unrelated approved request', () => {
  const result = admitOriginal(undefined)
  assert.equal(result.status, 'NOT_RUN')
  assert.equal(result.IN03, 'NOT_RUN')
  assert.ok(result.missing.some(value => value.input === 'server-expired request identity'))
  assert.ok(!result.missing.some(value => value.input === 'admitted approvedRequestId'))
})

test('dependency outage cannot be admitted as authorization-denial evidence; diagnostics stay redacted', () => {
  const privateValue = 'unit-redaction-canary'
  const result = admitOriginal({ principals: { requester: { email: privateValue, password: privateValue } },
    denials: { unrelated: [503], expired: [200], revoked: [] } })
  assert.equal(result.status, 'NOT_RUN')
  for (const kind of ['unrelated', 'expired', 'revoked'])
    assert.ok(result.missing.some(value => value.input === `contract-qualified ${kind} denial statuses`))
  assert.ok(!JSON.stringify(result).includes(privateValue))
})

test('malformed private JSON cannot expose its content in the thrown diagnostic', () => {
  const directory = mkdtempSync(join(tmpdir(), 'in03-admission-'))
  try {
    const path = join(directory, 'private.json')
    writeFileSync(path, '{"password":"unit-redaction-canary",', { mode: 0o600 })
    assert.throws(() => readRunConfig(path), { message: 'NOT_RUN: private real configuration unreadable' })
  } finally { rmSync(directory, { recursive: true }) }
})
