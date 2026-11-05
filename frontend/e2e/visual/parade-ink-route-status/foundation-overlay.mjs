// Readonly PI01 consumer overlay. Never changes production or supplied originals.
import { readFile } from 'node:fs/promises'
import { createHash } from 'node:crypto'
import { dirname, relative, resolve } from 'node:path'
import assert from 'node:assert/strict'

export async function foundationOverlay(root) {
  const evidence = resolve(root, 'docs/mcr/evidence/parade-ink-report-error-network')
  const manifest = JSON.parse(await readFile(resolve(evidence, 'producer-inputs.json'), 'utf8'))
  const pins = JSON.parse(await readFile(resolve(evidence, 'readset.json'), 'utf8')).files
  const tuples = manifest.entries.map(e => Object.fromEntries(Object.entries(e).sort(([a], [b]) => a.localeCompare(b))))
  assert.equal(createHash('sha256').update(JSON.stringify(tuples)).digest('hex'), 'a9b89a8bee524066009722f020cccc869ff506f65a79b35e5d7de289a6f24fd7')
  assert.equal(tuples.length, 18)
  const map = new Map()
  for (const entry of tuples) {
    const copy = resolve(evidence, 'fixture-overlay', entry.path)
    const bytes = await readFile(copy)
    assert.equal(bytes.length, entry.bytes)
    assert.equal(createHash('sha256').update(bytes).digest('hex'), entry.sha256)
    map.set(resolve(root, entry.path), { copy, bytes })
  }
  async function css(source, bytes) {
    let text = bytes.toString('utf8')
    for (const match of text.matchAll(/@import\s+["']([^"']+)["'];/g)) {
      const path = resolve(dirname(source), match[1]), pinned = map.get(path)
      let input = pinned?.bytes
      if (!input) {
        assert(['cozy-identity-backgrounds-v1.css', 'cozy-community-backgrounds-v1.css'].includes(match[1].split('/').at(-1)))
        input = await readFile(path)
        assert.equal(createHash('sha256').update(input).digest('hex'), pins[relative(root, path)])
      }
      text = text.replace(match[0], await css(path, input))
    }
    return text.replace(/url\(["']?([^"')]+)["']?\)/g, (original, url) => {
      const pinned = map.get(resolve(dirname(source), url))
      return pinned ? `url("/@fs${pinned.copy}")` : original
    })
  }
  return { name: 'pi12-immutable-pi01-consumer', enforce: 'pre',
    resolveId(source, importer) {
      const clean = source.split('?')[0]
      const candidate = clean.startsWith('/src/') ? resolve(root, 'frontend', clean.slice(1))
        : clean.startsWith('.') && importer ? resolve(dirname(importer.split('?')[0]), clean)
          : clean.startsWith('/') ? clean : null
      const entry = candidate && map.get(candidate)
      return entry ? candidate.endsWith('.ttf') ? entry.copy : candidate : null
    },
    async load(id) {
      const source = id.split('?')[0], entry = map.get(source)
      if (!entry || source.endsWith('.ttf')) return null
      return source.endsWith('.css') ? css(source, entry.bytes) : entry.bytes.toString('utf8')
    },
  }
}
