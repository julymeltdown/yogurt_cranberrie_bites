"""Apply screenshot budget only inside an accepted dual-guard browser clock."""
from pathlib import Path
p = Path(__file__).resolve().parents[4] / 'frontend/e2e/visual/parade-ink-route-status/native.mjs'
s = p.read_text()
a = "  await page.screenshot({ path: resolve(output, `${result.width}-${result.height}-${label}.png`), fullPage: true })"
b = """  // All frozen cases retain geometry/state proof; select 39 PNGs across both groups.
  const selected = group === 'network'
    ? result.width === 390 && result.height === 844 || ['offline', 'version'].includes(label)
    : result.width === 390 && result.height === 844 || ['report-ready', 'report-pending', 'http-401', 'http-503'].includes(label)
  const screenshot = selected ? `${result.width}-${result.height}-${label}.png` : null
  if (screenshot) await page.screenshot({ path: resolve(output, screenshot), fullPage: true })"""
if a in s:
    assert s.count(a) == 1
    s = s.replace(a, b).replace('result.captures.push({ label, geometry, proof })', 'result.captures.push({ label, geometry, proof, screenshot })')
    p.write_text(s)
else:
    assert b in s, 'Unexpected native driver postimage'
print('Applied bounded screenshot selection; every frozen scenario and viewport remains exercised.')
