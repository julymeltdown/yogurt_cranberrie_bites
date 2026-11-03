// Audited Bigyeting extracts at 5eecdcaffd1a9d505b9ab2a974f8d5623217e90a; see form-policy-helpers.md.
import { z } from 'zod'

/** Unicode code points, including combining marks and joiners; not graphemes. */
export const codePointLength = (value: string) => [...value].length
export const boundedText = (minimum: number, maximum: number) => z.string().refine(
  value => codePointLength(value) >= minimum && codePointLength(value) <= maximum,
  `Use between ${minimum} and ${maximum} Unicode code points.`,
)
export const tagSchema = boundedText(1, 24).transform(value => value.trim()).pipe(boundedText(1, 24))
export const tagArray = (maximum: number, schema = tagSchema) => z.array(schema).max(maximum).superRefine((tags, context) => {
  if (new Set(tags).size !== tags.length) context.addIssue({ code: 'custom', message: 'Tags must be unique.' })
})
