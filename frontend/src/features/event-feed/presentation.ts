/** Feed metadata only. No timers, requests or inferred author information. */
export function relativePostTime(createdAt: string, now = Date.now()): string {
  const time = Date.parse(createdAt), elapsed = Math.max(0, now - time)
  if (!Number.isFinite(time)) return 'Date unavailable'
  if (elapsed < 60_000) return 'Just now'
  if (elapsed < 3_600_000) return `${Math.floor(elapsed / 60_000)}m ago`
  if (elapsed < 86_400_000) return `${Math.floor(elapsed / 3_600_000)}h ago`
  if (elapsed < 604_800_000) return `${Math.floor(elapsed / 86_400_000)}d ago`
  return new Date(time).toLocaleDateString('en-GB', { day: 'numeric', month: 'short', year: 'numeric', timeZone: 'Asia/Seoul' })
}
