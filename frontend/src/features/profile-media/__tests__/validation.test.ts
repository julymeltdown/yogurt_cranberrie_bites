// WP-06 T-508: alt text and description validation.
//
// The acceptance statement is "a 161-character alt text is blocked client-side
// and rejected server-side". The server half lives in
// `backend/services/concert-service/tests/profile_posts.rs`
// (`the_description_is_bounded_at_five_hundred_code_points`) and in the frozen
// `social.ProfilePostMedia` alt bound; this file proves the client half, and that
// the client counts exactly the way the server counts.
import { describe, expect, it } from 'vitest'
import {
  MAX_ALT_TEXT_CODE_POINTS,
  MAX_ATTACHMENTS,
  MAX_DESCRIPTION_CODE_POINTS,
  MAX_DIMENSION,
  MAX_PIXELS,
  MAX_UPLOAD_BYTES,
  countedLength,
  draftIsPublishable,
  remaining,
  validateAltText,
  validateAttachmentCount,
  validateDescription,
  validateDimensions,
  validateDraft,
  validateFile,
  validateVisibility,
  type ProfilePostDraft,
} from '../validation'

const draft = (overrides: Partial<ProfilePostDraft> = {}): ProfilePostDraft => ({
  description: '',
  visibility: 'PUBLIC',
  attachments: [],
  ...overrides,
})

const attachment = (overrides: Partial<ProfilePostDraft['attachments'][number]> = {}) => ({
  localId: 'local-1',
  altText: '',
  contentType: 'image/png' as const,
  size: 1024,
  width: 1200,
  height: 800,
  ...overrides,
})

describe('the limits are the server limits', () => {
  it('uses the frozen bounds', () => {
    expect(MAX_DESCRIPTION_CODE_POINTS).toBe(500)
    expect(MAX_ALT_TEXT_CODE_POINTS).toBe(160)
    expect(MAX_ATTACHMENTS).toBe(4)
    expect(MAX_UPLOAD_BYTES).toBe(10 * 1024 * 1024)
    expect(MAX_DIMENSION).toBe(4096)
    expect(MAX_PIXELS).toBe(16_000_000)
  })
})

describe('alt text', () => {
  it('accepts an empty alt text and exactly 160 code points', () => {
    expect(validateAltText('')).toBeNull()
    expect(validateAltText('a'.repeat(160))).toBeNull()
  })

  it('blocks a 161-character alt text client-side', () => {
    const issue = validateAltText('a'.repeat(161))
    expect(issue).not.toBeNull()
    expect(issue?.code).toBe('TOO_LONG')
    expect(issue?.field).toBe('alt_text')
    expect(issue?.message).toContain('160')
  })

  it('counts code points, so an emoji counts once', () => {
    // 160 emoji are 160 code points but 320 UTF-16 units. A client counting
    // `.length` would refuse this valid alt text.
    const emoji = '🎨'.repeat(160)
    expect(emoji.length).toBe(320)
    expect(countedLength(emoji)).toBe(160)
    expect(validateAltText(emoji)).toBeNull()
    expect(validateAltText('🎨'.repeat(161))?.code).toBe('TOO_LONG')
  })

  it('counts a combining mark as its own code point, like the server', () => {
    // "e" + combining acute is two code points. The server's `char_length` and
    // the Rust `chars().count()` both say 2.
    expect(countedLength('e\u0301')).toBe(2)
    expect(validateAltText('e\u0301'.repeat(80))).toBeNull()
    expect(validateAltText('e\u0301'.repeat(81))?.code).toBe('TOO_LONG')
  })

  it('reports which attachment the issue is about', () => {
    const issue = validateAltText('a'.repeat(161), 2)
    expect(issue?.attachment).toBe(2)
  })

  it('refuses a NUL', () => {
    expect(validateAltText('a\u0000b')?.code).toBe('EMPTY')
  })
})

describe('description', () => {
  it('accepts an empty description and exactly 500 code points', () => {
    expect(validateDescription('')).toBeNull()
    expect(validateDescription('가'.repeat(500))).toBeNull()
  })

  it('blocks a 501-code-point description', () => {
    const issue = validateDescription('가'.repeat(501))
    expect(issue?.code).toBe('TOO_LONG')
    expect(issue?.field).toBe('description')
  })

  it('counts a multi-byte character once', () => {
    // 500 Korean characters are 1500 UTF-8 bytes and 500 UTF-16 units; the
    // server counts 500 code points.
    expect(countedLength('가'.repeat(500))).toBe(500)
    expect(validateDescription('가'.repeat(500))).toBeNull()
  })

  it('reports the remaining budget and never goes negative', () => {
    expect(remaining('', 500)).toBe(500)
    expect(remaining('가'.repeat(500), 500)).toBe(0)
    expect(remaining('가'.repeat(501), 500)).toBe(0)
  })
})

describe('file and dimension limits', () => {
  it('accepts only JPEG, PNG and WebP', () => {
    for (const type of ['image/jpeg', 'image/png', 'image/webp']) {
      expect(validateFile({ type, size: 1024 })).toBeNull()
    }
    for (const type of ['image/gif', 'video/mp4', 'image/avif', 'text/plain']) {
      expect(validateFile({ type, size: 1024 })?.code).toBe('UNSUPPORTED_TYPE')
    }
  })

  it('accepts 10 MiB and refuses one byte more', () => {
    expect(validateFile({ type: 'image/png', size: MAX_UPLOAD_BYTES })).toBeNull()
    expect(validateFile({ type: 'image/png', size: MAX_UPLOAD_BYTES + 1 })?.code).toBe('TOO_LARGE')
    expect(validateFile({ type: 'image/png', size: 0 })?.code).toBe('TOO_LARGE')
  })

  it('accepts 4096 px per side and refuses 4097', () => {
    expect(validateDimensions(4096, 1)).toBeNull()
    expect(validateDimensions(1, 4096)).toBeNull()
    expect(validateDimensions(4097, 1)?.code).toBe('TOO_MANY_PIXELS')
  })

  it('refuses more than 16 megapixels even within 4096 per side', () => {
    // 4096 x 4096 is 16,777,216 pixels, over the 16 M limit.
    expect(validateDimensions(4096, 4096)?.code).toBe('TOO_MANY_PIXELS')
    // 4000 x 4000 is exactly 16,000,000 and is accepted.
    expect(validateDimensions(4000, 4000)).toBeNull()
  })

  it('bounds the attachment count at four', () => {
    expect(validateAttachmentCount(0)).toBeNull()
    expect(validateAttachmentCount(4)).toBeNull()
    expect(validateAttachmentCount(5)?.code).toBe('TOO_MANY')
  })

  it('accepts only the two visibilities', () => {
    expect(validateVisibility('PUBLIC')).toBeNull()
    expect(validateVisibility('PRIVATE')).toBeNull()
    expect(validateVisibility('FRIENDS')?.field).toBe('visibility')
  })
})

describe('a whole draft', () => {
  it('is publishable with no attachments and no description', () => {
    expect(validateDraft(draft())).toEqual([])
    expect(draftIsPublishable(draft())).toBe(true)
  })

  it('reports every issue, in field order', () => {
    const issues = validateDraft(draft({
      description: 'a'.repeat(501),
      attachments: [
        attachment({ altText: 'a'.repeat(161) }),
        attachment({ localId: 'local-2', contentType: 'image/gif' as never }),
      ],
    }))
    // The description is reported first, then each attachment in order.
    expect(issues.map(issue => issue.code)).toEqual(['TOO_LONG', 'TOO_LONG', 'UNSUPPORTED_TYPE'])
    expect(issues[0]?.field).toBe('description')
    expect(issues[1]?.attachment).toBe(0)
    expect(issues[2]?.attachment).toBe(1)
  })

  it('refuses a fifth attachment', () => {
    const issues = validateDraft(draft({
      attachments: Array.from({ length: 5 }, (_, index) => attachment({ localId: `local-${index}` })),
    }))
    expect(issues.some(issue => issue.code === 'TOO_MANY')).toBe(true)
    expect(draftIsPublishable(draft({
      attachments: Array.from({ length: 5 }, (_, index) => attachment({ localId: `local-${index}` })),
    }))).toBe(false)
  })
})
