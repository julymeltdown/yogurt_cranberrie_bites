import { expect, it } from 'vitest'
import ts from 'typescript'
import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { execFileSync } from 'node:child_process'
const repo = resolve(import.meta.dirname, '../../../../..')
const base = '2b3819fbe686b03769648e698c3741b8f9debe18'
const paths = ['app/account-deletion-entry/panel.tsx', 'app/archive-entry/index.tsx', 'app/safety-entry/panels.tsx', 'app/social-composition/sharing.tsx', 'features/account/panel.tsx', 'features/age-status/AgeStatus.tsx', 'features/archive/ArchivePage.tsx', 'features/inbox/ui/Inbox.tsx', 'features/safety/panels.tsx', 'features/social-sharing/ui/SocialSharing.tsx']
// Remove only the authorized inert wrapper/class/import additions. Everything else,
// including every expression, handler, ref, hook, role and exact copy, must match.
function functional(source: string) {
  const file = ts.createSourceFile('input.tsx', source, ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX)
  const result = ts.transform(file, [context => root => {
    const visit: ts.Visitor = node => {
      if (ts.isImportDeclaration(node) && ts.isStringLiteral(node.moduleSpecifier) && node.moduleSpecifier.text.includes('parade-ink-private/private.css')) return undefined
      if (ts.isJsxElement(node) && ts.isIdentifier(node.openingElement.tagName) && node.openingElement.tagName.text === 'div') {
        const attrs = node.openingElement.attributes.properties
        const cls = attrs.find(a => ts.isJsxAttribute(a) && a.name.getText(file) === 'className')
        if (attrs.length === 1 && cls && ts.isJsxAttribute(cls) && cls.initializer && ts.isStringLiteral(cls.initializer) && ['pi-private-account', 'pi-private-account-entry', 'pi-private-blocks'].includes(cls.initializer.text)) {
          const children = node.children.filter(c => !ts.isJsxText(c) || c.text.trim())
          if (children.length === 1) return ts.visitNode(children[0], visit)
        }
      }
      if (ts.isJsxAttribute(node) && node.name.getText(file) === 'className' && node.initializer && ts.isStringLiteral(node.initializer)) {
        const text = node.initializer.text.split(' ').filter(s => !s.startsWith('pi-private-')).join(' ')
        return ts.factory.updateJsxAttribute(node, node.name, ts.factory.createStringLiteral(text))
      }
      return ts.visitEachChild(node, visit, context)
    }
    return ts.visitNode(root, visit) as ts.SourceFile
  }])
  const printed = ts.createPrinter({ removeComments: false }).printFile(result.transformed[0] as ts.SourceFile)
  result.dispose(); return printed
}
it.each(paths)('preserves all functional syntax and protected hunks in %s', path => {
  const full = 'frontend/src/' + path
  const before = execFileSync('git', ['show', `${base}:${full}`], { cwd: repo, encoding: 'utf8' })
  expect(functional(readFileSync(resolve(repo, full), 'utf8'))).toBe(functional(before))
})
it.each(['features/safety/safety.css','features/people/people.css','features/social-sharing/ui/sharing.css','features/inbox/ui/inbox.css','features/archive/archive.css'])('preserves the readonly stylesheet %s', path => {
  const full = 'frontend/src/' + path
  expect(readFileSync(resolve(repo, full), 'utf8')).toBe(execFileSync('git', ['show', `${base}:${full}`], { cwd: repo, encoding: 'utf8' }))
})
