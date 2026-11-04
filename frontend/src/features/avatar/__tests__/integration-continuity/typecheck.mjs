// Full application roots and retained dependency paths, without tracked config/cache writes.
import assert from 'node:assert/strict'
import { createHash } from 'node:crypto'
import { readFileSync } from 'node:fs'
import { dirname, resolve } from 'node:path'
const helper = resolve(import.meta.dirname, '../../../../world/avatar/__tests__/motion-adoption/typecheck.mjs')
const source = readFileSync(helper, 'utf8')
assert.equal(createHash('sha256').update(source).digest('hex'), '4cbcb57e175101a3fabbcbd67604c223d2604e24732f9d6e3331583a0d624aa1')
assert(source.includes('...parsed.options, incremental:'))
// Existing test configs use .ts imports. Retain the prior no-emit invocation option,
// every application file, and all diagnostics; no exclusion or config change.
const invocation = source.replace('...parsed.options, incremental:', '...parsed.options, allowImportingTsExtensions: true, incremental:')
  .replaceAll('import.meta.dirname', JSON.stringify(dirname(helper)))
process.argv.push('--application')
await import('data:text/javascript;base64,' + Buffer.from(invocation).toString('base64'))
