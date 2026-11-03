import { privateIntroSchema } from './contract'
import { introContactBypass } from './publication-contact'

/** Advisory BE25 parity only. The server owns publication, identity and event authorization. */
export function introPolicyValid(input: unknown, context: { now: number; eventCutoff: number; publishing: boolean }): boolean {
  const result = privateIntroSchema.safeParse(input)
  if (!result.success) return false
  const row = result.data, { now, eventCutoff, publishing } = context
  const nil = '00000000-0000-0000-0000-000000000000'
  return Number.isFinite(now) && Number.isFinite(eventCutoff) && eventCutoff > now
    && ![row.intro_id, row.event_id, row.owner_id].includes(nil) && Date.parse(row.created_at) <= now
    && !row.text.includes('\0')
    && (row.expires_at === null || Date.parse(row.expires_at) > now && Date.parse(row.expires_at) <= eventCutoff)
    && (!(publishing || row.state !== 'DRAFT') || row.text.replace(/^\p{White_Space}+|\p{White_Space}+$/gu, '').length > 0)
    && (!(publishing || row.state === 'PUBLISHED') || !introContactBypass(row.text))
}
