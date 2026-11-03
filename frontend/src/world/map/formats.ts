/** C06 uses format:uuid plus its own lowercase shape pattern, without UUID version/variant policy. */
export const validUuid = (value: string) => value.length === 36 && /^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$/.test(value)
// Matches pinned C06 rfc3339-validator0.1.4, which excludes leap seconds.
export function validDateTime(value: string) {
  const match = /^(\d{4})-(\d{2})-(\d{2})[Tt](\d{2}):(\d{2}):(\d{2})(?:\.\d+)?([Zz]|[+-]\d{2}:\d{2})$/.exec(value)
  if (!match || match[0] !== value) return false
  const [, year, month, day, hour, minute, second, zone] = match
  const y = Number(year), m = Number(month), days = [31, y % 4 === 0 && (y % 100 !== 0 || y % 400 === 0) ? 29 : 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
  return y >= 1 && m >= 1 && m <= 12 && Number(day) >= 1 && Number(day) <= days[m - 1]! && Number(hour) <= 23 && Number(minute) <= 59
    && Number(second) <= 59 && (zone!.toUpperCase() === 'Z' || Number(zone!.slice(1, 3)) <= 23 && Number(zone!.slice(4)) <= 59)
}
/** Pinned C06 URI-checker syntax, not WHATWG normalization or permission to fetch provenance. */
export function validUri(value: string) {
  const match = /^[A-Za-z][A-Za-z0-9+.-]*:(.*)$/.exec(value)
  if (!match || match[0] !== value) return false
  const rest = match[1]!, hash = rest.indexOf('#'), fragment = hash < 0 ? '' : rest.slice(hash + 1), withoutFragment = hash < 0 ? rest : rest.slice(0, hash)
  const question = withoutFragment.indexOf('?'), query = question < 0 ? '' : withoutFragment.slice(question + 1), hier = question < 0 ? withoutFragment : withoutFragment.slice(0, question)
  const pathChars = /^(?:[A-Za-z0-9._~!$&'()*+,;=:@/-]|%[0-9A-Fa-f]{2})*$/
  const queryChars = /^(?:[A-Za-z0-9._~!$&'()*+,;=:@/?-]|%[0-9A-Fa-f]{2})*$/
  if (!queryChars.test(query) || !queryChars.test(fragment)) return false
  if (!hier.startsWith('//')) return pathChars.test(hier)
  const slash = hier.indexOf('/', 2), authority = slash < 0 ? hier.slice(2) : hier.slice(2, slash), path = slash < 0 ? '' : hier.slice(slash)
  if (!pathChars.test(path)) return false
  const at = authority.indexOf('@'), user = at < 0 ? '' : authority.slice(0, at), hostPort = at < 0 ? authority : authority.slice(at + 1)
  if (!/^(?:[A-Za-z0-9._~!$&'()*+,;=:-]|%[0-9A-Fa-f]{2})*$/.test(user)) return false
  if (hostPort.startsWith('[')) {
    const close = hostPort.indexOf(']'); if (close < 0 || !/^(?::\d*)?$/.test(hostPort.slice(close + 1))) return false
    const host = hostPort.slice(1, close)
    if (/^v[0-9A-Fa-f]+\.[A-Za-z0-9._~!$&'()*+,;=:-]+$/.test(host)) return true
    return validIpv6(host)
  }
  const parts = hostPort.split(':')
  return parts.length <= 2 && /^(?:[A-Za-z0-9._~!$&'()*+,;=-]|%[0-9A-Fa-f]{2})*$/.test(parts[0]!) && (parts.length === 1 || /^\d*$/.test(parts[1]!))
}

/** Independently authored group counter matching the pinned URI checker's IPv6 alternatives.
 * Its dotted tail accepts 1..3 decimal digits per octet, including leading zeros.
 * Provenance syntax only: never normalize this into, or use it as, a fetch URL.
 */
function validIpv6(host: string) {
  if (host.length > 45) return false
  const halves = host.split('::')
  if (halves.length > 2) return false
  const groups = halves.flatMap(half => half === '' ? [] : half.split(':'))
  let units = 0
  for (const [index, group] of groups.entries()) {
    if (group.includes('.')) {
      const octets = group.split('.')
      if (index !== groups.length - 1 || !host.endsWith(group) || octets.length !== 4 || !octets.every(o => /^\d{1,3}$/.test(o) && Number(o) <= 255)) return false
      units += 2
    } else {
      if (!/^[0-9a-fA-F]{1,4}$/.test(group)) return false
      units++
    }
  }
  return halves.length === 2 ? units < 8 : units === 8
}
