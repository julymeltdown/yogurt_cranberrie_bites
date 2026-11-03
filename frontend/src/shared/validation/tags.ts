// Audited parseTagInput from Bigyeting social-intro-model.ts at 5eecdcaffd1a9d505b9ab2a974f8d5623217e90a.
/** Audited comma-input utility. Validate the result with tagArray before use. Not a C04 intro field. */
export function parseTagInput(value: string, maximum: number): readonly string[] {
  return value.split(',').map(tag => tag.trim().replace(/^#/, ''))
    .filter((tag, index, tags) => tag.length > 0 && tags.indexOf(tag) === index).slice(0, maximum)
}
