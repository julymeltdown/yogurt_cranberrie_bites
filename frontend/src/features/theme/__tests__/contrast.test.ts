import { readFileSync } from 'node:fs'
import { expect, it } from 'vitest'
const css = readFileSync(new URL('../../../shared/styles/themes/themes.css', import.meta.url), 'utf8')
function luminance(hex: string) {
  const channels = hex.slice(1).match(/../g)!.map(channel => {
    const s = Number.parseInt(channel, 16) / 255
    return s <= .04045 ? s / 12.92 : ((s + .055) / 1.055) ** 2.4
  })
  return channels[0]! * .2126 + channels[1]! * .7152 + channels[2]! * .0722
}
function contrast(a: string, b: string) { const x = luminance(a), y = luminance(b); return (Math.max(x, y) + .05) / (Math.min(x, y) + .05) }
it.each(['REVENGE', 'PARADE', 'DANGER'])('%s supports readable semantic text and visible control focus', theme => {
  const body = new RegExp(`\\[data-mcr-theme="${theme}"\\] \\{([^}]+)`).exec(css)![1]!
  const tokens = Object.fromEntries([...body.matchAll(/--mcr-([a-z]+): (#[0-9a-f]{6})/g)].map(match => [match[1]!, match[2]!]))
  for (const color of ['ink', 'muted', 'red', 'success', 'warning']) expect(contrast(tokens[color]!, tokens.paper!), `${color} on paper`).toBeGreaterThanOrEqual(4.5)
  expect(contrast(tokens.ink!, tokens.cream!), 'ink on cream').toBeGreaterThanOrEqual(4.5)
  expect(contrast(tokens.focus!, tokens.paper!), 'focus on paper').toBeGreaterThanOrEqual(3)
})
