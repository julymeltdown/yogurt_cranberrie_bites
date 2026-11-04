import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import ts from 'typescript'
import type { Page } from '@playwright/test'
import type { WorldObserver } from '../real/world-gate/observer'
import { captureReconnectBefore, reconnectWorldOnce, requireReplacement } from './reconnect.ts'

// Source-only boundary/driver stubs. No app, DOM, socket, authority or runtime
// is fabricated for the real journey; these checks cannot certify that journey.
type Observation = Pick<WorldObserver, 'baseline' | 'opens' | 'active' | 'peak' | 'invalidFrames'>
const initial = { roomEpoch: 'old-epoch', streamId: 'old-stream', eventId: 'event', selfUserId: 'member' } as NonNullable<WorldObserver['baseline']>
const connected = (): Observation => ({ baseline: { ...initial }, opens: 2, active: 1, peak: 1, invalidFrames: 0 })
let checks = 0
const reject = (work: () => unknown) => { assert.throws(work); checks++ }
const before = captureReconnectBefore(connected())
assert(Object.isFrozen(before)); checks++
const prior = connected(), captured = captureReconnectBefore(prior)
prior.baseline = { ...initial, streamId: 'changed-later' }
assert.equal(captured.streamId, 'old-stream'); checks++
for (const patch of [{ baseline: null }, { active: 0 }, { peak: 2 }, { invalidFrames: 1 }]) reject(() => captureReconnectBefore({ ...connected(), ...patch }))
const replacement = (restarted: boolean): Observation => ({ ...connected(), opens: 3, baseline: { ...initial, streamId: 'new-stream', roomEpoch: restarted ? 'new-epoch' : initial.roomEpoch } })
for (const restarted of [false, true]) { requireReplacement(before, replacement(restarted), restarted); checks++ }
for (const patch of [{ active: 0 }, { peak: 2 }, { invalidFrames: 1 }, { opens: 4 }, { opens: 2 }, { baseline: null },
  { baseline: { ...initial, streamId: 'new-stream', roomEpoch: 'new-epoch', selfUserId: 'foreign' } },
  { baseline: { ...initial, streamId: 'new-stream', roomEpoch: 'new-epoch', eventId: 'foreign' } },
  { baseline: { ...initial, roomEpoch: 'new-epoch' } }, { baseline: { ...initial, streamId: 'new-stream' } }]) {
  reject(() => requireReplacement(before, { ...replacement(true), ...patch }, true))
}
reject(() => requireReplacement(before, replacement(true), false))

function driver(restarted: boolean, fault = '') {
  const observer: Observation = { ...connected(), active: 0, baseline: null }
  const log: string[] = []; let path = '/wall', clicks = 0, disposed = false, original = fault !== 'replaced-before'
  const handle = { evaluate: async () => original, dispose: async () => { disposed = true } }
  const page = {
    url: () => 'http://127.0.0.1:4000' + path,
    locator(selector: string) {
      assert(['[data-world-host] canvas', '[data-world-host]', '.world-shell'].includes(selector))
      return { elementHandle: async () => handle, count: async () => 1,
        getAttribute: async (name: string) => name === 'inert' ? path === '/world' && fault !== 'inert' ? null : '' : path === '/world' ? 'closed' : 'full' }
    },
    getByRole(role: string, opts: { name: string; exact: boolean }) {
      assert.equal(opts.exact, true)
      const region = role === 'region'
      assert(region ? ['World scene and controls', 'Wall editor'].includes(opts.name) : role === 'navigation' && opts.name === 'Main navigation')
      return {
        waitFor: async () => { assert(path === (opts.name === 'Wall editor' ? '/wall' : '/world')); if (fault === 'hidden-region') throw Error('HIDDEN_CONTROLS') },
        getByRole(child: string, choice: { name: string; exact: boolean }) {
          assert.equal(choice.exact, true)
          assert(region ? child === 'button' && choice.name === 'Reconnect world' : child === 'link' && ['World', 'Wall'].includes(choice.name))
          return {
            waitFor: async () => { if (fault === 'missing-link' && !region || fault === 'hidden-reconnect' && region) throw Error('MISSING_REACHABLE_CONTROL') },
            isEnabled: async () => !(fault === 'disabled-reconnect' && region),
            click: async () => {
              log.push(choice.name)
              if (!region) { path = choice.name === 'World' ? '/world' : '/wall'; return }
              assert.equal(path, '/world'); clicks++
              if (fault === 'click-failure') throw Error('CLICK_FAILED_NO_RETRY')
              Object.assign(observer, replacement(restarted))
              if (fault === 'replaced-after') original = false
            },
          }
        },
      }
    },
  } as unknown as Page
  return { page, observer, log, clicks: () => clicks, disposed: () => disposed }
}
for (const restarted of [false, true]) {
  const d = driver(restarted)
  await reconnectWorldOnce(d.page, d.observer, before, restarted, Date.now() + 1000)
  assert.deepEqual(d.log, ['World', 'Reconnect world', 'Wall']); assert.equal(d.clicks(), 1); assert(d.disposed()); checks++
}
for (const fault of ['missing-link', 'hidden-region', 'hidden-reconnect', 'disabled-reconnect', 'inert', 'replaced-before', 'replaced-after', 'click-failure']) {
  const d = driver(true, fault)
  await assert.rejects(reconnectWorldOnce(d.page, d.observer, before, true, Date.now() + 50))
  assert.equal(d.clicks(), ['replaced-after', 'click-failure'].includes(fault) ? 1 : 0)
  assert(d.disposed()); checks++
}

const text = await readFile(new URL('./wall-b.spec.ts', import.meta.url), 'utf8')
const source = ts.createSourceFile('wall-b.spec.ts', text, ts.ScriptTarget.Latest, true)
const calls: ts.CallExpression[] = []
function visit(node: ts.Node) { if (ts.isCallExpression(node)) calls.push(node); ts.forEachChild(node, visit) }
visit(source)
for (const stage of ['reconnect', 'restartEpoch']) {
  const block = calls.find(c => c.expression.getText(source) === 'step' && c.arguments[0] && ts.isStringLiteral(c.arguments[0]) && c.arguments[0].text === stage)!
  assert(block)
  const nested = calls.filter(c => c.pos > block.pos && c.end < block.end)
  assert.equal(nested.filter(c => c.expression.getText(source) === 'reconnectWorldOnce').length, 1)
  assert(!block.getText(source).includes('.isVisible()')); checks++
  if (stage === 'restartEpoch') {
    const retry = nested.find(c => c.expression.getText(source) === 'reconnectWorldOnce')!
    const observe = nested.find(c => c.expression.getText(source) === 'port.observeRestart')!
    assert(observe.end < retry.pos)
    assert.equal(retry.arguments[3]?.kind, ts.SyntaxKind.TrueKeyword)
    assert(ts.isForStatement(retry.parent.parent.parent))
    assert.equal(retry.parent.parent.parent.condition?.getText(source), 'n < 2'); checks++
  }
}
process.stdout.write(JSON.stringify({ result: 'PASS', checks, scope: 'focused reconnect boundaries and runner wiring only', old84Replayed: false, realIN04: 'NOT_RUN' }) + '\n')
