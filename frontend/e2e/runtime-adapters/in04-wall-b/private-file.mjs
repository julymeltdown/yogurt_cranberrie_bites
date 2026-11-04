// Shared Node-only file custody reader; also embedded in the self-contained QA
// module so private live frontend bindings need not exist during source bundling.
import assert from 'node:assert/strict'
import { open } from 'node:fs/promises'
import { constants } from 'node:fs'
import { isAbsolute } from 'node:path'
import { createHash } from 'node:crypto'

export async function ownedBytes(path, { maximum = 32768, privateOnly = false, hash } = {}) {
  assert(typeof path === 'string' && isAbsolute(path), 'Input path must be absolute')
  const file = await open(path, constants.O_RDONLY | constants.O_NOFOLLOW)
  try {
    const before = await file.stat()
    assert(before.isFile() && before.uid === process.getuid() && (before.mode & (privateOnly ? 0o077 : 0o022)) === 0,
      'Input must be an owned regular file with restricted permissions')
    assert(before.size > 0 && before.size <= maximum, 'Input size bound')
    const bytes = await file.readFile()
    const after = await file.stat()
    assert(before.ino === after.ino && before.size === after.size && before.mtimeMs === after.mtimeMs && bytes.length === before.size,
      'Input changed during read')
    if (hash !== undefined) {
      assert.match(hash, /^[a-f0-9]{64}$/)
      assert.equal(createHash('sha256').update(bytes).digest('hex'), hash, 'Input hash mismatch')
    }
    return bytes
  } finally { await file.close() }
}
