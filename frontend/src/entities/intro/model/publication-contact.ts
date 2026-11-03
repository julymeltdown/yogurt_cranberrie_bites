// Audited pure contact detector at Bigyeting 5eecdcaffd1a9d505b9ab2a974f8d5623217e90a.
// Publication policy only; ordinary text/tag helpers do not prohibit contact-like prose.
export const introContactBypass = (value: string) => {
  const normalized = [...value].map((character) => {
    const code = character.codePointAt(0)!
    if (code >= 0xff01 && code <= 0xff5e) return String.fromCodePoint(code - 0xfee0)
    if (character === '\u3002' || character === '\uff61') return '.'
    if (character === '\u2215' || character === '\u2044') return '/'
    return character
  }).join('').toLowerCase()
  const compact = [...normalized].filter((character) => (
    !/\p{White_Space}/u.test(character) && !'[](){}\u200b\u200c\u200d'.includes(character)
  )).join('')
  const alphanumeric = [...normalized].filter((character) => /[\p{Alphabetic}\p{N}]/u.test(character)).join('')
  const domain = (candidate: string) => compact.split(candidate).some((_, index, parts) => {
    if (index === parts.length - 1) return false
    const start = parts.slice(0, index + 1).join(candidate).length
    // BE25 tests complete Unicode characters on each side, not UTF-16 surrogate halves.
    const before = Array.from(compact.slice(0, start)).at(-1)
    const after = Array.from(compact.slice(start + candidate.length))[0]
    return (before === undefined || !/[\p{Alphabetic}\p{N}]/u.test(before))
      && (after === undefined || !/[\p{Alphabetic}\p{N}]/u.test(after))
  })
  const hasLinkOrHandle = normalized.includes('@')
    || compact.includes('http://')
    || compact.includes('https://')
    || domain('instagram.com')
    || domain('twitter.com')
    || domain('x.com')
    || domain('discord.gg')
    || domain('t.me')
    || alphanumeric.includes('instagramdotcom')
    || alphanumeric.includes('discorddotgg')
    || alphanumeric.includes('tdotme')
    || normalized.includes('kakao')
    || normalized.includes('카카오톡')
    || normalized.includes('오픈채팅')
    || compact.includes('line:id')
    || compact.includes('line_id')
    || compact.includes('line-id')
    || normalized.includes('line id')
    || normalized.includes('line 아이디')
    || normalized.includes('라인 id')
    || normalized.includes('라인아이디')
  const digitCount = [...normalized].filter((character) => /[0-9]/.test(character)).length
  return hasLinkOrHandle || digitCount >= 9
}
