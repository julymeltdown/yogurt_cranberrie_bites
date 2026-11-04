import { test } from 'node:test'
import assert from 'node:assert/strict'
import { distribution, summarize, growth } from './metrics.mjs'
test('nearest-rank tails preserve outliers without mutating raw samples', () => {
  const values = [200, 16, 17, 18, 19, 20, 21, 22, 23, 24]
  const result = distribution(values)
  assert.equal(result.p50, 20); assert.equal(result.p95, 200); assert.equal(result.max, 200)
  assert.equal(values[0], 200)
})
test('growth excludes declared warmup and preserves negative or positive observed trends', () => {
  assert.equal(growth([10000, 100, 110, 120, 130]).slopeBytesPerCycle, 10)
  assert.equal(growth([10000, 130, 120, 110, 100]).firstToLastBytes, -30)
  assert.equal(growth([100]).slopeBytesPerCycle, null)
})
test('zero renders in a covered world differ from RAF activity and unknown intervals', () => {
  const result = summarize({ start: 0, end: 1000, raf: [0, 500, 1000], renders: [] })
  assert.equal(result.raf.ratePerSecond, 3); assert.equal(result.raf.intervalsMs.mean, 500)
  assert.equal(result.phaserPostRender.ratePerSecond, 0); assert.equal(result.phaserPostRender.intervalsMs.mean, null)
})
test('30 FPS reference uses the original budget and measured window, not requested duration', () => {
  const result = summarize({ start: 0, end: 2000, requestedMs: 1000, raf: [10, 30, 70], renders: [10, 30, 70] })
  assert.equal(result.phaserPostRender.ratePerSecond, 1.5)
  assert.equal(result.phaserPostRender.intervalsAboveOriginal30FpsBudget, 1)
})
