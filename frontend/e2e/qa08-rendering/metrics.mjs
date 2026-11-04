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
