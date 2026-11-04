export function inspectInputReadiness(input: unknown): Readonly<{
  status: 'INPUT_NOT_READY' | 'INPUT_CANDIDATE'
  productionAdmitted: false
  reasons: readonly string[]
  worldReason: string | null
  limitation: string
}>
