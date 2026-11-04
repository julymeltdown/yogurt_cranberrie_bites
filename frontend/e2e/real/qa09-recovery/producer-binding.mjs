import { open } from 'node:fs/promises'
import { constants } from 'node:fs'
import { createHash } from 'node:crypto'
import { dirname, resolve, basename } from 'node:path'
import { cases, caseDefinition } from './cases.mjs'

const sha = /^[0-9a-f]{40}$/, digest = /^[0-9a-f]{64}$/, run = /^[0-9a-f]{32}$/
const utc = /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d{1,6})?(?:Z|\+00:00)$/
const fail = () => { throw new Error('QA09_PRODUCER_BINDING_INVALID') }
const time = value => {
  if (value === null) return null
  if (typeof value !== 'string' || !utc.test(value) || !Number.isFinite(Date.parse(value))
    || new Date(value).toISOString().slice(0, 19) !== value.slice(0, 19)) fail()
  return Date.parse(value)
}

/** Coordination envelope, not a product contract. Opaque backend observations and
 * limitations are never copied into frontend evidence or interpreted as acceptance.
 * A physical-restore segment may have an artifact but no original domain boundary.
 */
export function producerProjection(value, expected) {
  caseDefinition(expected.caseId)
  if (!run.test(expected.runId) || !sha.test(expected.backend)
    || expected.frontend !== null && !sha.test(expected.frontend)) fail()
  if (!value || value.version !== 1 || value.runId !== expected.runId || value.caseId !== expected.caseId
    || !['NOT_RUN', 'IN_PROGRESS'].includes(value.state) || value.sourceRevisions?.backend !== expected.backend) fail()
  const frontend = value.sourceRevisions.frontend ?? null
  if (frontend !== null && !sha.test(frontend) || expected.frontend !== null && frontend !== expected.frontend) fail()
  const phases = value.phases
  if (!phases || !['before', 'boundary', 'recovered'].every(key => Object.hasOwn(phases, key))) fail()
  const before = time(phases.before), boundary = time(phases.boundary), recovered = time(phases.recovered)
  if (boundary !== null && (before === null || boundary < before)
    || recovered !== null && (before === null || recovered < before || boundary !== null && recovered < boundary)) fail()
  let evidence = null
  if (value.producerEvidence !== null && value.producerEvidence !== undefined) {
    const { path, sha256 } = value.producerEvidence
    if (typeof path !== 'string' || path.length > 128 || !/^[A-Za-z0-9][A-Za-z0-9_.-]*$/.test(path)
      || basename(path) !== path || !digest.test(sha256)) fail()
    evidence = { path, sha256 }
  }
  return { version: 1, runId: expected.runId, caseId: expected.caseId, state: value.state,
    sourceRevisions: { backend: expected.backend, frontend },
    phases: { before: phases.before, boundary: phases.boundary, recovered: phases.recovered },
    producerEvidence: evidence, frontendSourceBound: expected.frontend !== null && frontend === expected.frontend,
    hasOriginalBoundary: before !== null && boundary !== null && recovered !== null,
    qualification: 'Producer metadata only. A pinned segment or notification does not qualify original QA09.' }
}

/** The actual backend plan/restore CLI writes one report containing six cases.
 * Select exactly one case only after binding the enclosing run/candidate and
 * checking the complete, unique original case roster. Never treat the outer
 * IN_PROGRESS state or a different case's artifact as this case's execution.
 */
export function producerReportProjection(report, expected) {
  if (!report || report.version !== 1 || report.runId !== expected.runId
    || report.sourceRevisions?.backend !== expected.backend || report.wholeQa09Accepted !== false
    || !['NOT_RUN', 'IN_PROGRESS'].includes(report.state) || !Array.isArray(report.cases)
    || report.cases.length !== Object.keys(cases).length) fail()
  const seen = new Set()
  for (const item of report.cases) {
    if (!item || !Object.hasOwn(cases, item.caseId) || seen.has(item.caseId)) fail()
    seen.add(item.caseId)
    producerProjection(item, { ...expected, caseId: item.caseId })
  }
  return producerProjection(report.cases.find(item => item.caseId === expected.caseId), expected)
}

/** Read only the explicitly provided owner report and its same-directory artifact.
 * Bounded file reads; no directory discovery, URL attachment, database or process control.
 */
export async function readProducerProjection(reportPath, expected) {
  async function boundedFile(path, limit) {
    const file = await open(path, constants.O_RDONLY | constants.O_NOFOLLOW)
    try {
      const metadata = await file.stat()
      if (!metadata.isFile() || metadata.size > limit) fail()
      const buffer = Buffer.alloc(limit + 1)
      let total = 0
      while (total < buffer.length) {
        const { bytesRead } = await file.read(buffer, total, buffer.length - total, null)
        if (!bytesRead) break
        total += bytesRead
      }
      if (total > limit) fail()
      return buffer.subarray(0, total)
    } finally { await file.close() }
  }
  const report = await boundedFile(reportPath, 64 * 1024)
  let decoded
  try { decoded = JSON.parse(report) } catch { fail() }
  const projection = producerReportProjection(decoded, expected)
  let artifactVerified = false
  if (projection.producerEvidence) {
    const bytes = await boundedFile(resolve(dirname(reportPath), projection.producerEvidence.path), 16 * 1024 * 1024)
    if (createHash('sha256').update(bytes).digest('hex') !== projection.producerEvidence.sha256) fail()
    artifactVerified = true
  }
  return { ...projection, reportSha256: createHash('sha256').update(report).digest('hex'), artifactVerified }
}
