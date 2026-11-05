import { createRequire } from 'node:module'
import { resolve, dirname } from 'node:path'
import { readFileSync } from 'node:fs'
import { createHash } from 'node:crypto'
export const frontend = resolve(import.meta.dirname, '../../../../..')
export const dependencies = '/Users/lhs/Desktop/mcr/frontend/node_modules'
export const retained = createRequire('/Users/lhs/Desktop/mcr/frontend/package.json')
export const aliases = ['react/jsx-runtime', 'react/jsx-dev-runtime', 'react-dom/client', 'react-dom/server', 'react-dom', 'react',
  'react-router-dom', 'react-hook-form', '@hookform/resolvers/zod', 'browser-image-compression', '@tanstack/react-query', 'zod', 'zustand', 'ky', 'phaser',
  '@testing-library/jest-dom/vitest', '@testing-library/react'].map(name => ({ find: new RegExp('^' + name + '$'), replacement: name === '@testing-library/jest-dom/vitest' ? retained.resolve(name).replace(/\.js$/, '.mjs') : retained.resolve(name) }))
export function compositePlugin() {
  const folder = process.env.PI10_OVERLAY
  if (!folder) throw new Error('PI10_OVERLAY must name the admitted immutable source fixture')
  const readset = JSON.parse(readFileSync(resolve(folder, 'readset.json'), 'utf8'))
  if (readset.commit !== '77cfe94af06cac6b292ad7528352e4750e929e71') throw new Error('Wrong final shared checkpoint')
  if (readset.qualificationSHA !== 'eb80d722f513ce794fe35317a87702dd2878321a2d76f313d36265b3d593d5ef') throw new Error('Wrong final shared readset')
  const files = new Map(readset.files.map(file => {
    const bytes = readFileSync(resolve(folder, file.path))
    if (createHash('sha256').update(bytes).digest('hex') !== file.sha256) throw new Error(`Changed fixture: ${file.path}`)
    return [resolve(frontend, '..', file.path), { file, bytes }]
  }))
  return { name: 'pi10-pinned-pi01-composite', enforce: 'pre',
    resolveId(source, importer) {
      const clean = source.split('?')[0]
      const candidate = clean.startsWith('/src/') ? resolve(frontend, `.${clean}`)
        : clean.startsWith('.') && importer ? resolve(dirname(importer.split('?')[0]), clean) : clean
      for (const file of [candidate, `${candidate}.tsx`, `${candidate}.ts`, resolve(candidate, 'index.tsx')]) {
        const entry = files.get(file)
        if (entry) return /\.(ttf|woff2?|png|svg)$/.test(file) ? resolve(folder, entry.file.path) : file
      }
    },
    load(id) {
      const entry = files.get(id.split('?')[0])
      if (!entry || /\.(ttf|woff2?|png|svg)$/.test(id)) return
      let text = entry.bytes.toString('utf8')
      if (entry.file.path === 'frontend/src/shared/styles/base.css') {
        for (const [name, expected] of [
          ['cozy-identity-backgrounds-v1.css', '77b25626d6c94f3f226e1ccc079a8b96d11d6a4873dfc6e20fcef174c49f5f6d'],
          ['cozy-community-backgrounds-v1.css', 'd26dd8fbee928539f2689ccd9bb6269e4cb964c58b78e8cc86afbd894f228504'],
        ]) {
          const css = readFileSync(resolve(frontend, 'src/shared/styles', name))
          if (createHash('sha256').update(css).digest('hex') !== expected) throw new Error(`Changed read-only CSS dependency: ${name}`)
          text = text.replace(`@import "./${name}" layer(mcr-foundation-legacy);`, `@layer mcr-foundation-legacy {${css.toString('utf8')}}`)
        }
        for (const name of ['tokens.css', 'primitives.css']) {
          const path = `frontend/src/shared/styles/parade-ink/${name}`
          const pinned = files.get(resolve(frontend, '..', path))
          let css = pinned.bytes.toString('utf8')
          if (name === 'tokens.css') css = css.replaceAll('./fonts/', `/@fs/${resolve(folder, 'frontend/src/shared/styles/parade-ink/fonts')}/`)
          text = text.replace(`@import "./parade-ink/${name}";`, css)
        }
      }
      return text
    },
  }
}
