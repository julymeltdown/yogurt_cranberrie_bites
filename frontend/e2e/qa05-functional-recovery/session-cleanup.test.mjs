// Synthetic resource failures only. Actual browser/owner cleanup still needs
// the fresh managed window; no fake I/O is product acceptance.
import { test } from 'node:test'
import assert from 'node:assert/strict'
import { createSessionDisposer } from './session.mjs'

test('all cleanup phases execute after private handle/page/observer failures and report only fixed labels', async () => {
  const calls = []
  const fail = label => () => { calls.push(label); throw Error('PRIVATE-TICKET-DO-NOT-LOG') }
  const cover = { close: fail('cover') }, extraPages = new Set([cover])
  let handle = { evaluate: fail('visibility-listener'), dispose: fail('visibility-handle') }
  const dispose = createSessionDisposer({ pages: [{ close: fail('page-a') }, { close: async () => { calls.push('page-b') } }], extraPages,
    observers: [{ dispose: fail('observer-a') }, { dispose: async () => { calls.push('observer-b') } }],
    contexts: [{ close: async () => { calls.push('context') } }],
    takeVisibilityHandle: () => { const value = handle; handle = null; return value }, retire: () => { calls.push('retire') } })
  await assert.rejects(dispose(), error => {
    assert.equal(error.message, 'QA05_SESSION_CLEANUP_FAILED:VISIBILITY_LISTENER,VISIBILITY_HANDLE,PAGE,OBSERVER')
    assert(!String(error).includes('PRIVATE')); return true
  })
  assert.deepEqual(calls, ['retire', 'visibility-listener', 'visibility-handle', 'page-a', 'page-b', 'cover', 'observer-a', 'observer-b', 'context'])
  assert.equal(handle, null); assert.equal(extraPages.size, 0)
})
test('concurrent and later cleanup calls share one result and never skip pending context closure', async () => {
  let finish, closed = 0, retired = 0
  const closing = new Promise(resolve => { finish = resolve })
  const dispose = createSessionDisposer({ pages: [{ close: () => closing }], extraPages: new Set(), observers: [],
    contexts: [{ close: async () => { closed++ } }], takeVisibilityHandle: () => null, retire: () => { retired++ } })
  const first = dispose(), second = dispose()
  assert.equal(first, second); assert.equal(retired, 1); assert.equal(closed, 0)
  finish(); await first
  assert.equal(closed, 1); assert.equal(dispose(), first); await dispose(); assert.equal(closed, 1)
})
test('cleanup of a partially initialized App still closes its acquired context', async () => {
  let contexts = 0
  const dispose = createSessionDisposer({ pages: [], extraPages: new Set(), observers: [],
    contexts: [{ close() { contexts++ } }], takeVisibilityHandle: () => undefined, retire() {} })
  await dispose(); assert.equal(contexts, 1)
})
test('a synchronous close event can reenter disposal without creating another cleanup', async () => {
  let reentered, closes = 0, dispose
  dispose = createSessionDisposer({ pages: [{ close() { closes++; reentered = dispose() } }], extraPages: new Set(), observers: [], contexts: [],
    takeVisibilityHandle: () => undefined, retire() {} })
  const first = dispose(); await first
  assert.equal(reentered, first); assert.equal(closes, 1)
})
test('a failed owned context is reported after all contexts have been attempted', async () => {
  const closed = []
  const dispose = createSessionDisposer({ pages: [], extraPages: new Set(), observers: [],
    contexts: [{ close() { closed.push(1); throw Error('PRIVATE') } }, { close() { closed.push(2) } }],
    takeVisibilityHandle: () => undefined, retire() {} })
  await assert.rejects(dispose(), /QA05_SESSION_CLEANUP_FAILED:CONTEXT/)
  assert.deepEqual(closed, [1, 2])
})
test('the same owned page registered as a cover is closed once, before listeners and contexts', async () => {
  const calls = [], page = { close() { calls.push('page') } }
  const dispose = createSessionDisposer({ pages: [page], extraPages: new Set([page]),
    observers: [{ dispose() { calls.push('observer') } }], contexts: [{ close() { calls.push('context') } }],
    takeVisibilityHandle: () => undefined, retire() {} })
  await dispose(); assert.deepEqual(calls, ['page', 'observer', 'context'])
})
