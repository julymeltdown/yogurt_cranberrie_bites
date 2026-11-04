// Actual existing product DOM only. No preview/test entry, Auth override, raw HTTP,
// injected commands, offline queue, backend process control or request interception.
const timeout = 10_000
export const actions = Object.freeze({
  async readPickup(page) { await page.getByRole('button', { name: 'Read current reservation', exact: true }).click({ timeout }) },
  async checkPickup(page) { await page.getByRole('button', { name: 'Check original command', exact: true }).click({ timeout }) },
  async checkDeletion(page) { await page.getByRole('button', { name: 'Check deletion progress', exact: true }).click({ timeout }) },
  async refreshInbox(page) {
    await page.getByRole('button', { name: 'Refresh inbox', exact: true }).click({ timeout })
  },
  async resumeClosure(page, { reason, jobId, expectedRevision }) {
    if (typeof reason !== 'string' || !reason.trim() || reason.length > 500) throw new Error('QA09_RESUME_REASON_REQUIRED')
    if (!/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/.test(jobId)
      || !Number.isSafeInteger(expectedRevision) || expectedRevision < 1) throw new Error('QA09_RESUME_BINDING_REQUIRED')
    const form = page.getByRole('form', { name: 'Prepare closure resume', exact: true })
    await form.getByRole('textbox', { name: 'Resume reason', exact: true }).fill(reason, { timeout })
    await form.getByRole('button', { name: 'Retry failed closure steps', exact: true }).click({ timeout })
    const review = page.getByRole('group', { name: 'Confirm closure resume', exact: true })
    if (await review.locator('code').innerText({ timeout }) !== jobId
      || !await review.getByText(`expected job revision ${expectedRevision}.`, { exact: false }).isVisible()) throw new Error('QA09_RESUME_REVIEW_MISMATCH')
    await review.getByRole('button', { name: 'Confirm resume closure', exact: true }).click({ timeout })
  },
})

/** Only booleans/counts escape the browser. Never capture raw DOM text or storage. */
export async function observeDom(page) {
  const visible = async locator => (await locator.count()) === 1 && await locator.isVisible()
  const enabled = async locator => (await locator.count()) === 1 && await locator.isVisible() && await locator.isEnabled()
  return {
    canvasCount: await page.locator('[data-world-host] canvas').count(),
    pickupRecoveryVisible: await visible(page.getByRole('button', { name: 'Check original command', exact: true })),
    collectionReceiptVisible: await visible(page.getByRole('heading', { name: 'Collection receipt', exact: true })),
    closurePartialFailureVisible: await visible(page.getByLabel('Closure job state', { exact: true }).filter({ hasText: /^PARTIAL_FAILURE · revision \d+$/ })),
    closureCompletedVisible: await visible(page.getByLabel('Closure job state', { exact: true }).filter({ hasText: /^COMPLETED · revision \d+$/ })),
    closureResumeEnabled: await enabled(page.getByRole('button', { name: 'Retry failed closure steps', exact: true })),
    deletionProgressVisible: await visible(page.getByRole('list', { name: 'Deletion progress', exact: true })),
    deletionCheckEnabled: await enabled(page.getByRole('button', { name: 'Check deletion progress', exact: true })),
    deletionCompletedVisible: await visible(page.getByRole('status').filter({ hasText: /^Account deletion confirmed by the Auth owner\.$/ })),
  }
}
