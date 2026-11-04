// Measurement arithmetic; no invented pass/fail performance thresholds.
export function distribution(values) {
  if (!values.length) return { count: 0, min: null, mean: null, p50: null, p95: null, p99: null, max: null }
  const sorted = [...values].sort((a, b) => a - b)
  const q = p => sorted[Math.max(0, Math.ceil(sorted.length * p) - 1)]
  return { count: values.length, min: sorted[0], mean: values.reduce((s, x) => s + x, 0) / values.length, p50: q(.5), p95: q(.95), p99: q(.99), max: sorted.at(-1) }
}
export function summarize(sample) {
  const windowMs = sample.end - sample.start
  const frame = points => {
    const intervals = points.slice(1).map((x, i) => x - points[i])
    return { callbacks: points.length, ratePerSecond: points.length * 1000 / windowMs, intervalsMs: distribution(intervals), intervalsAboveOriginal30FpsBudget: intervals.filter(x => x > 1000 / 30).length }
  }
  return { windowMs, raf: frame(sample.raf), phaserPostRender: frame(sample.renders) }
}
/** Observed finite-sample growth, not a leak verdict or an invented memory budget. */
export function growth(samples, warmupCycles = 1) {
  const retained = samples.slice(warmupCycles)
  if (retained.length < 2) return { samples: retained.length, slopeBytesPerCycle: null, firstToLastBytes: null }
  const meanX = (retained.length - 1) / 2, meanY = retained.reduce((n, x) => n + x, 0) / retained.length
  const slope = retained.reduce((n, y, x) => n + (x - meanX) * (y - meanY), 0) / retained.reduce((n, _, x) => n + (x - meanX) ** 2, 0)
  return { samples: retained.length, warmupCycles, slopeBytesPerCycle: slope, firstToLastBytes: retained.at(-1) - retained[0], minBytes: Math.min(...retained), maxBytes: Math.max(...retained) }
}
