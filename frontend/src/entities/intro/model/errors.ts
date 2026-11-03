import { z } from 'zod'
import { DOMAIN_OPERATIONS } from '../../../shared/api/domain-operations.generated'

/** Exact C04 write reasons; no legacy AGE_NOT_ALLOWED/CONTACT_BLOCKED wire enum is invented. */
export const introWriteReasonSchema = z.enum([...new Set(Object.values(DOMAIN_OPERATIONS.setConcertIntro.errors).flat())])
