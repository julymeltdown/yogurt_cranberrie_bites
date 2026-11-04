// Synthetic source tests with a deterministic clock. No actual relay or runtime.
import { test } from 'node:test'
import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import { createFrameFaultController } from './frame-faults.mjs'

function harness(overrides = {}) {
  let at = 0, next = 0
  const timers = new Map(), calls = [], sent = []
  const controller = createFrameFaultController({
    send: (bytes, metadata) => { sent.push({ bytes: Buffer.from(bytes), ...metadata }); calls.push('send') },
    pauseUpstream: () => { calls.push('pause') }, resumeUpstream: () => { calls.push('resume') },
    close: () => { calls.push('close') }, now: () => at,
    setTimer: (fn, ms) => { const id = ++next; timers.set(id, { fn, at: at + ms }); return id },
    clearTimer: id => { timers.delete(id) }, ...overrides,
  })
  return { controller, calls, sent, timers,
    setTime: value => { at = value },
    fire: (value = Math.min(...[...timers.values()].map(t => t.at))) => {
      at = value
      for (const [id, timer] of timers) if (timer.at <= at) { timers.delete(id); timer.fn() }
    } }
}
const classify = { isBinary: false, isDelta: true }
const request = action => ({ caseId: action === 'drop-next-delta' ? 'packet-gap' : action === 'delay-downstream' ? 'packet-delay' : 'slow-client',
  requestId: `window:${action}`, action, maximumFrames: action === 'drop-next-delta' ? 1 : action === 'delay-downstream' ? 8 : 64,
  ...(action === 'drop-next-delta' ? {} : { maximumBytes: 262144 }), maximumMs: action === 'delay-downstream' ? 250 : 1000 })

test('an idle or completed controller preserves exact bytes and binary flags; only one classified real delta is dropped', () => {
  const h = harness(), bytes = Buffer.from([0, 255, 13, 10])
  h.controller.acceptServerFrame(bytes, { ...classify, isBinary: true })
  assert.deepEqual(h.sent, [{ bytes, isBinary: true }])
  h.controller.arm(request('drop-next-delta'))
  h.controller.acceptServerFrame(bytes, { ...classify, isDelta: false })
  h.controller.acceptServerFrame(bytes, classify)
  h.controller.acceptServerFrame(bytes, classify)
  assert.equal(h.sent.length, 3)
  const p = h.controller.readProof()
  assert.equal(p.state, 'COMPLETE'); assert.equal(p.droppedFrames, 1)
  assert.equal(p.droppedBytes, bytes.length); assert.equal(p.acceptedFrames, 3)
  assert.equal(p.deliveredFrames, 2); assert.equal(h.timers.size, 0)
})
test('delay copies retained opaque traffic, preserves FIFO and does not expose payloads', () => {
  const h = harness(), a = Buffer.from('PRIVATE-TICKET-NOT-LOGGED'), b = Buffer.from([0, 255])
  h.controller.arm(request('delay-downstream'))
  h.controller.acceptServerFrame(a, classify); h.controller.acceptServerFrame(b, { ...classify, isBinary: true })
  a.fill(0); b.fill(1)
  assert.equal(h.sent.length, 0); assert.equal(h.controller.readProof().peakQueuedFrames, 2)
  assert(!JSON.stringify(h.controller.readProof()).includes('PRIVATE-TICKET'))
  h.fire()
  assert.equal(h.sent[0].bytes.toString(), 'PRIVATE-TICKET-NOT-LOGGED')
  assert.deepEqual(h.sent[1], { bytes: Buffer.from([0, 255]), isBinary: true })
  const p = h.controller.readProof()
  assert.equal(p.state, 'COMPLETE'); assert.equal(p.queuedBytes, 0)
  assert(p.completedAtMs > p.armedAtMs && p.completedAtMs <= p.deadlineMs)
})
test('real pause and resume callbacks are measured even when TCP pause admits no additional frames', () => {
  const h = harness(); h.controller.arm(request('pause-downstream'))
  assert.deepEqual(h.calls, ['pause']); h.fire()
  assert.deepEqual(h.calls, ['pause', 'resume'])
  const p = h.controller.readProof()
  assert.equal(p.state, 'COMPLETE'); assert.equal(p.pauseCompleted, 1); assert.equal(p.resumeCompleted, 1)
  assert(p.resumedAtMs > p.pausedAtMs && p.resumedAtMs <= p.deadlineMs)
})
test('pause buffers only already arriving bounded frames and flushes FIFO before resuming upstream', () => {
  const h = harness(); h.controller.arm(request('pause-downstream'))
  h.controller.acceptServerFrame(Buffer.from('one'), classify); h.controller.acceptServerFrame(Buffer.from('two'), classify)
  h.fire()
  assert.deepEqual(h.calls, ['pause', 'send', 'send', 'resume'])
  assert.deepEqual(h.sent.map(frame => frame.bytes.toString()), ['one', 'two'])
})
for (const action of ['delay-downstream', 'pause-downstream']) {
  test(`${action} frame overflow retires before allocating or delivering the excess frame`, () => {
    const h = harness(); h.controller.arm({ ...request(action), maximumFrames: 1 })
    h.controller.acceptServerFrame(Buffer.from('first'), classify)
    const p = h.controller.acceptServerFrame(Buffer.from('excess'), classify)
    assert.equal(p.state, 'RETIRED'); assert.equal(p.failure, 'QA05_QUEUE_BOUND')
    assert.equal(p.acceptedFrames, 1); assert.equal(p.queuedFrames, 0); assert.equal(p.deliveredFrames, 0)
    h.fire(2000); assert.deepEqual(h.calls, action === 'pause-downstream' ? ['pause', 'close'] : ['close'])
  })
  test(`${action} byte overflow cannot pass a frame-count-only budget`, () => {
    const h = harness(); h.controller.arm({ ...request(action), maximumBytes: 5 })
    h.controller.acceptServerFrame(Buffer.from('123'), classify)
    const p = h.controller.acceptServerFrame(Buffer.from('456'), classify)
    assert.equal(p.failure, 'QA05_QUEUE_BOUND'); assert.equal(p.acceptedBytes, 3); assert.equal(h.sent.length, 0)
  })
}
test('missing delta never becomes an applied gap; delay with no frames never claims an effect', () => {
  for (const action of ['drop-next-delta', 'delay-downstream']) {
    const h = harness(); h.controller.arm(request(action)); h.fire()
    assert.equal(h.controller.readProof().failure, 'QA05_NO_FAULT_EFFECT')
    assert.equal(h.controller.readProof().state, 'RETIRED'); assert.deepEqual(h.calls, ['close'])
  }
})
test('event-loop overrun closes without late delivery or resumption', () => {
  const h = harness(); h.controller.arm(request('pause-downstream'))
  h.controller.acceptServerFrame(Buffer.from('queued'), classify); h.fire(1001)
  assert.equal(h.controller.readProof().failure, 'QA05_TIMER_LATE')
  assert.deepEqual(h.calls, ['pause', 'close']); assert.equal(h.sent.length, 0)
})
test('an expired frame callback cannot enqueue past the timer deadline', () => {
  const h = harness(); h.controller.arm(request('delay-downstream')); h.setTime(250)
  h.controller.acceptServerFrame(Buffer.from('late'), classify)
  assert.equal(h.controller.readProof().failure, 'QA05_FAULT_EXPIRED'); assert.equal(h.sent.length, 0)
})
test('a blocking sink that exhausts the hold budget cannot flush the next frame or resume', () => {
  let h
  h = harness({ send: () => { h.setTime(1001) } })
  h.controller.arm(request('pause-downstream'))
  h.controller.acceptServerFrame(Buffer.from('first'), classify); h.controller.acceptServerFrame(Buffer.from('second'), classify)
  h.fire()
  const p = h.controller.readProof()
  assert.equal(p.failure, 'QA05_TIMER_LATE'); assert.equal(p.sendAttempts, 1); assert.equal(p.deliveredFrames, 1)
  assert.equal(p.resumeAttempts, 0); assert.deepEqual(h.calls, ['pause', 'close'])
})
for (const failure of ['send', 'pauseUpstream', 'resumeUpstream']) {
  test(`uncertain ${failure} output retires once without exposing private callback errors`, () => {
    const h = harness({ [failure]: () => { throw Error('SECRET-OWNER-TICKET') } })
    h.controller.arm(request('pause-downstream'))
    h.controller.acceptServerFrame(Buffer.from('private'), classify); h.fire()
    const p = h.controller.readProof()
    assert.equal(p.state, 'RETIRED'); assert(!JSON.stringify(p).includes('SECRET'))
    assert.equal(h.calls.filter(v => v === 'close').length, 1)
    h.controller.dispose(); assert.equal(h.calls.filter(v => v === 'close').length, 1)
  })
}
test('async callbacks are not mistaken for completed effects, and rejected promises are consumed', async () => {
  const h = harness({ send: async () => { throw Error('SECRET-ASYNC') } })
  h.controller.arm(request('drop-next-delta'))
  const p = h.controller.acceptServerFrame(Buffer.from('x'), { ...classify, isDelta: false })
  assert.equal(p.failure, 'QA05_SEND_UNCERTAIN'); assert.equal(p.deliveredFrames, 0)
  await new Promise(resolve => setImmediate(resolve)); assert.deepEqual(h.calls, ['close'])
})
test('duplicate or overlapping arming cannot execute a second fault; dispose discards queued bytes and timers', () => {
  const h = harness(); h.controller.arm(request('delay-downstream'))
  assert.throws(() => h.controller.arm(request('drop-next-delta')), /NOT_ARMABLE/)
  h.controller.acceptServerFrame(Buffer.from('x'), classify); h.fire()
  assert.throws(() => h.controller.arm(request('delay-downstream')), /DUPLICATE/)
  h.controller.arm(request('pause-downstream')); h.controller.acceptServerFrame(Buffer.from('later'), classify)
  h.controller.dispose(); h.fire(2000)
  assert.equal(h.controller.readProof().queuedBytes, 0); assert.equal(h.timers.size, 0)
  assert.deepEqual(h.calls, ['send', 'pause', 'close'])
})
test('oversized or unclassified frames and clock regression fail closed', () => {
  for (const run of [h => h.controller.acceptServerFrame(Buffer.alloc(65547), classify),
    h => h.controller.acceptServerFrame(Buffer.from('x'), { isBinary: false }),
    h => { h.setTime(-1); h.controller.acceptServerFrame(Buffer.from('x'), classify) }]) {
    const h = harness(); h.controller.arm(request('drop-next-delta')); run(h)
    assert.equal(h.controller.readProof().state, 'RETIRED'); assert.deepEqual(h.calls, ['close'])
  }
})
test('the exact self-contained utility imports through a verified data URL', async () => {
  const bytes = await readFile(new URL('./frame-faults.mjs', import.meta.url))
  const module = await import('data:text/javascript;base64,' + bytes.toString('base64'))
  const seen = []
  const c = module.createFrameFaultController({ send: bytes => { seen.push(Buffer.from(bytes)) }, pauseUpstream() {}, resumeUpstream() {}, close() {} })
  c.acceptServerFrame(Buffer.from([0, 255]), { isBinary: true, isDelta: false })
  assert.deepEqual(seen, [Buffer.from([0, 255])]); c.dispose()
})
