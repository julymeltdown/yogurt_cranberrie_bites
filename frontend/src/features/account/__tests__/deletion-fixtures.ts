/** Synthetic fixtures pinned independently to the published d3b7c382 contract. */
export const requiredTargets = [
  'PROFILE', 'FRIEND', 'POST', 'NOTIFICATION',
  'CONCERT_TICKETS', 'CONCERT_SESSIONS', 'CONCERT_SHARING', 'CONCERT_PRIVATE_DATA',
] as const
export const deletionIds = {
  actor: '11111111-1111-4111-8111-111111111111', event: '22222222-2222-4222-8222-222222222222',
  other: '33333333-3333-4333-8333-333333333333', saga: '88888888-8888-4888-8888-888888888888',
}
export const deletionNow = Date.parse('2026-11-04T06:00:00Z')
export function deletionSaga(complete = false) {
  return {
    id: deletionIds.saga, userId: deletionIds.actor,
    status: complete ? 'COMPLETED' : 'IN_PROGRESS',
    retentionUntil: '2026-12-04T05:00:00Z', createdAt: '2026-11-04T05:00:00Z',
    updatedAt: '2026-11-04T05:59:00Z', completedAt: complete ? '2026-11-04T05:58:00Z' : null,
    steps: requiredTargets.map(target => ({
      target: target as string, status: complete ? 'SUCCEEDED' : 'PENDING', attemptCount: complete ? 1 : 0,
      nextAttemptAt: '2026-11-04T06:01:00Z', lastError: null as string | null,
      completedAt: complete ? '2026-11-04T05:57:00Z' : null as string | null,
    })),
  }
}
export function deletionAccepted(saga = deletionSaga()) {
  return { userId: deletionIds.actor, deleted: true, sagaId: deletionIds.saga, saga }
}
