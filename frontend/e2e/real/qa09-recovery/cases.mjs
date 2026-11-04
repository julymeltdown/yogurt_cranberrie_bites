// Operational labels for the six clauses of original QA09, not added acceptance gates.
export const cases = Object.freeze({
  'QA09-DB-RESTART': Object.freeze({ boundary: 'database-restart', action: 'readPickup', criterion: 'Restored state is consistent after an actual database restart.' }),
  'QA09-BACKUP-RESTORE': Object.freeze({ boundary: 'physical-backup-restore', action: 'readPickup', criterion: 'The actual restored backup retains committed pickup and consistent inventory.' }),
  'QA09-OUTBOX-REPLAY': Object.freeze({ boundary: 'outbox-replay', action: 'refreshInbox', criterion: 'A real relay restart/replay preserves committed effects without duplicate delivery.' }),
  'QA09-PENDING-PICKUP': Object.freeze({ boundary: 'pending-pickup', action: 'checkPickup', criterion: 'An uncertain pickup resolves the original command without a fresh collection intent.' }),
  'QA09-DELETION': Object.freeze({ boundary: 'deletion-recovery', action: 'checkDeletion', criterion: 'Deletion remains truthful while incomplete and converges with private state withdrawn.' }),
  'QA09-PARTIAL-CLOSURE': Object.freeze({ boundary: 'partial-closure', action: 'resumeClosure', criterion: 'The same partial job resumes explicitly and preserves completed shutdown/cleanup steps.' }),
})

export function caseDefinition(id) {
  if (!Object.hasOwn(cases, id)) throw new Error('QA09_UNKNOWN_CASE')
  return cases[id]
}

export function initialDisposition() {
  return Object.fromEntries(Object.entries(cases).map(([id, definition]) => [id, {
    status: 'NOT_RUN', criterion: definition.criterion,
    limitation: 'Requires a source-qualified actual producer, real App binding and a fresh accepted resource lease.',
  }]))
}
