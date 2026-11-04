import { readFileSync } from 'node:fs'
import { expect, it } from 'vitest'

const css = readFileSync(new URL('../../shell-chrome/shell-chrome.css', import.meta.url), 'utf8')
type RGB = readonly [number, number, number]
function luminance(rgb: RGB) {
  const channel = (n: number) => n / 255 <= .04045 ? n / 255 / 12.92 : ((n / 255 + .055) / 1.055) ** 2.4
  return channel(rgb[0]) * .2126 + channel(rgb[1]) * .7152 + channel(rgb[2]) * .0722
}
function contrast(a: RGB, b: RGB) {
  const x = luminance(a), y = luminance(b)
  return (Math.max(x, y) + .05) / (Math.min(x, y) + .05)
}
function background(selector: string): RGB {
  const block = css.split('}').find(rule => rule.trim().startsWith(selector + ' {'))
  const value = block?.match(/background:\s*#([0-9a-f]{6});/i)?.[1]
  if (!value) throw new Error('Missing solid header background for ' + selector)
  return [parseInt(value.slice(0, 2), 16), parseInt(value.slice(2, 4), 16), parseInt(value.slice(4, 6), 16)]
}

// Historical 7c25078f / actual c8a731e5 ME screenshot samples. This preserves
// existing foregrounds; it does not certify full WCAG focus compliance.
const original: RGB = [25, 24, 23], focus: RGB = [162, 35, 43], text: RGB = [242, 234, 223]
for (const selector of ['.mcr-chrome-header', '.mcr-chrome-panel-header']) {
  it(selector + ' paint preserves contrast for both frozen focus and text', () => {
    const paint = background(selector)
    expect(contrast(focus, paint)).toBeGreaterThanOrEqual(contrast(focus, original))
    expect(contrast(text, paint)).toBeGreaterThanOrEqual(contrast(text, original))
  })
}
it('detects the actual first-pass berry regression despite adequate text contrast', () => {
  const firstPass: RGB = [97, 66, 82]
  expect(contrast(text, firstPass)).toBeGreaterThan(7)
  expect(contrast(focus, firstPass)).toBeLessThan(contrast(focus, original))
})
