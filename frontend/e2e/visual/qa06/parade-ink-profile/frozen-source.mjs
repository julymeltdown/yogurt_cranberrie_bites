// Read-only source composition: PI06 application candidate plus explicit PI01 presentation postimages.
import assert from 'node:assert/strict'
import { execFileSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import { dirname, extname, relative, resolve } from 'node:path'
export function frozenSource(repo, revision) {
  const foundation = '056a8c48ff667134dee8e5e99710185294e4b898'
  const staticFrame = 'a2b6aaed44318ca3b38670a0fd3d09eb3c29df2b'
  const staticPaths = new Set(['frontend/src/app/character-entry/FramedWholeCharacterView.tsx', 'frontend/src/features/avatar/parade-ink/PortraitFrame.tsx', 'frontend/src/features/avatar/parade-ink/parade-ink-avatar.css'])
  const importPaths = new Set(['frontend/src/app/account-entry/panels.tsx', 'frontend/src/app/social-composition/panels.tsx'])
  const portrait = 'd181bfe4bc487984a3e15f2d3ab07ed7d4727301'
  const portraitPaths = new Set(['frontend/src/app/character-entry/ReviewedCharacterView.tsx', 'frontend/src/features/avatar/parade-ink/PortraitFrame.tsx', 'frontend/src/features/avatar/parade-ink/parade-ink-avatar.css'])
  const shellCorrection = 'b5b44e47531e3b75febc5ee5cdd6a11b1648779c'
  const shellPath = 'frontend/src/app/shell.css'
  const foundationBase = 'f025bdb6fb409f3b7706e475dfabb5f3d21fe168'
  assert(/^[a-f0-9]{40}$/.test(revision))
  const git = (...args) => execFileSync('git', args, { cwd: repo, maxBuffer: 64 * 1024 ** 2 })
  const sharedPaths = new Set(git('diff', '--name-only', foundationBase, foundation, '--', 'frontend/src').toString().trim().split('\n'))
  assert([...sharedPaths].every(path => path.startsWith('frontend/src/shared/') || path.startsWith('frontend/src/app/')))
  const files = new Set([...git('ls-tree', '-r', '--name-only', revision, 'frontend', 'contracts').toString().trim().split('\n'), ...sharedPaths, ...portraitPaths, ...staticPaths])
  const receipts = new Map(), cache = new Map(), transforms = new Set(); let size = 0
  const frontend = resolve(repo, 'frontend'), own = 'frontend/e2e/visual/qa06/parade-ink-profile/'
  function bytes(path) {
    assert(files.has(path), 'Missing frozen source: ' + path)
    if (!cache.has(path)) {
      const sourceRevision = staticPaths.has(path) ? staticFrame : path === shellPath ? shellCorrection : portraitPaths.has(path) ? portrait : sharedPaths.has(path) ? foundation : revision
      let value = git('show', `${sourceRevision}:${path}`)
      const beforeSha256 = createHash('sha256').update(value).digest('hex')
      if (importPaths.has(path)) {
        const before = "import { WholeCharacterView } from '../../entities/avatar/whole-character'"
        const after = "import { FramedWholeCharacterView as WholeCharacterView } from '../character-entry/FramedWholeCharacterView'"
        const text = value.toString(); assert.equal(text.split(before).length, 2, 'Exactly one authorized import hunk')
        value = Buffer.from(text.replace(before, after)); transforms.add(path + ': PI07 ' + staticFrame + ' import-only hunk over ' + sourceRevision + ' SHA ' + beforeSha256)
      }
      size += value.length
      assert(size <= 64 * 1024 ** 2, 'Bounded source/public memory')
      receipts.set(path, { revision: sourceRevision, path, sha256: createHash('sha256').update(value).digest('hex'), bytes: value.length }); cache.set(path, value)
    }
    return cache.get(path)
  }
  const pathOf = id => relative(repo, id.split('?')[0]).replaceAll('\\', '/')
  function css(path, chain = []) {
    assert(!chain.includes(path), 'CSS import cycle')
    let text = bytes(path).toString()
    text = text.replace(/url\(\s*(['"]?)(\.[^)'"\s]+)\1\s*\)/g, (_all, _quote, source) => {
      const asset = pathOf(resolve(repo, dirname(path), source)); assert(files.has(asset)); transforms.add(path + ': asset URL rebasing')
      return `url('/__pi06_frozen_asset__/${asset}')`
    })
    return text.replace(/@import\s+['"](\.[^'"]+\.css)['"]\s*;/g, (_all, source) => {
      const dependency = pathOf(resolve(repo, dirname(path), source)); transforms.add(path + ': relative CSS import flattening')
      return css(dependency, [...chain, path])
    })
  }
  const plugin = { name: 'pi06-source-pinned-application-and-pi01', enforce: 'pre',
    resolveId(source, importer) {
      if (!source.startsWith('.') && !source.startsWith('/')) return null
      const query = source.includes('?') ? source.slice(source.indexOf('?')) : '', clean = source.split('?')[0]
      const absolute = clean.startsWith('/src/') ? resolve(frontend, '.' + clean) : clean.startsWith('/') ? clean : importer ? resolve(dirname(importer.split('?')[0]), clean) : null
      if (!absolute) return null
      const path = pathOf(absolute); if (path.startsWith(own)) return null
      for (const suffix of ['', '.ts', '.tsx', '.mjs', '.js', '.json', '/index.ts', '/index.tsx']) if (files.has(path + suffix)) return resolve(repo, path + suffix) + query
      return null
    },
    load(id) {
      const path = pathOf(id)
      if (path.startsWith(own) || !files.has(path) || !/\.(?:tsx?|m?js|json|css)$/.test(path)) return null
      return extname(path) === '.css' ? css(path) : bytes(path).toString()
    },
  }
  return { revision, staticFrame, foundation, shellCorrection, portrait, portraitPaths: [...portraitPaths], files, bytes, plugin, receipts: () => [...receipts.values()], transformations: () => [...transforms], sharedPaths: [...sharedPaths] }
}
