import { describe, expect, it, vi } from 'vitest'
import { invalidateMuralEligibility, subscribeMuralEligibilityRefresh } from '../refresh'

describe('same-Auth eligibility invalidation', () => {
  it('notifies only the exact owner and privacy generation and unregisters cleanly', () => {
    const auth = {}, other = {}, scope = { userId: 'actor', eventId: 'event', generation: 1, policyGeneration: 2, gate: 'open' as const }
    const current = vi.fn(), wrong = vi.fn(), later = vi.fn()
    const stops = [subscribeMuralEligibilityRefresh(auth, scope, current), subscribeMuralEligibilityRefresh(other, scope, wrong),
      subscribeMuralEligibilityRefresh(auth, { ...scope, policyGeneration: 3 }, later)]
    invalidateMuralEligibility(auth, scope)
    expect(current).toHaveBeenCalledOnce(); expect(wrong).not.toHaveBeenCalled(); expect(later).not.toHaveBeenCalled()
    stops.forEach(stop => stop()); invalidateMuralEligibility(auth, scope)
    expect(current).toHaveBeenCalledOnce()
  })
})
