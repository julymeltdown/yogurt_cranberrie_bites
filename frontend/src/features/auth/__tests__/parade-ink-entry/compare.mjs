import { writeFile } from 'node:fs/promises'
import { resolve } from 'node:path'
/** Fixed unmasked header crop, never a claim that Auth copy equals Feed copy. */
export async function compareHeaders(browser, reference, candidates, output) {
  const page = await browser.newPage({ viewport: { width: 900, height: 200 } })
  const records = [], panels = []
  try {
    for (const candidate of candidates) {
      const result = await page.evaluate(async ({ reference, actual }) => {
        const image = async source => { const value = new Image(); value.src = source; await value.decode(); return value }
        const ref = await image(`data:image/jpeg;base64,${reference}`), app = await image(`data:image/png;base64,${actual}`)
        const make = () => { const canvas = document.createElement('canvas'); canvas.width = 180; canvas.height = 48; return canvas }
        const r = make(), a = make(), overlay = make(), diff = make()
        const rc = r.getContext('2d'), ac = a.getContext('2d')
        rc.fillStyle = '#f9f9f8'; rc.fillRect(0, 0, 180, 48)
        // PI01 sourceAnchors.header, normalized from395 source pixels to390CSSpx.
        rc.drawImage(ref, 29, 65, 385, 47, 0, 0, 385 * 390 / 395, 47 * 390 / 395)
        ac.drawImage(app, 0, 0)
        const rb = rc.getImageData(0, 0, 180, 48), ab = ac.getImageData(0, 0, 180, 48), dc = diff.getContext('2d'), db = dc.createImageData(180, 48)
        let total = 0, changed = 0
        for (let i = 0; i < rb.data.length; i += 4) {
          let pixel = 0
          for (let c = 0; c < 3; c++) { const d = Math.abs(rb.data[i + c] - ab.data[i + c]); total += d; pixel += d; db.data[i + c] = Math.min(255, d * 2) }
          if (pixel > 30) changed++
          db.data[i + 3] = 255
        }
        dc.putImageData(db, 0, 0)
        const oc = overlay.getContext('2d'); oc.drawImage(r, 0, 0); oc.globalAlpha = .5; oc.drawImage(a, 0, 0)
        return { mae: total / (180 * 48 * 3), changedFraction: changed / (180 * 48),
          reference: r.toDataURL(), actual: a.toDataURL(), overlay: overlay.toDataURL(), diff: diff.toDataURL() }
      }, { reference: reference.toString('base64'), actual: candidate.bytes.toString('base64') })
      const files = { comparison: 'header-comparison.png', row: records.length }
      panels.push(['reference', 'actual', 'overlay', 'diff'].map(kind => result[kind]))
      records.push({ name: candidate.name, width: candidate.width, crop: [0, 0, 180, 48], referenceCrop: [29, 65, 414, 112],
        referenceScale: 390 / 395, mask: null, mae: result.mae, changedFraction: result.changedFraction, files,
        interpretation: 'Unmasked visual evidence. Feed title/control labels differ from public/Auth; the metric is descriptive, not a pixel-acceptance threshold.' })
    }
    const sheet = await page.evaluate(async rows => {
      const canvas = document.createElement('canvas'); canvas.width = 720; canvas.height = rows.length * 48
      const context = canvas.getContext('2d')
      for (let row = 0; row < rows.length; row++) for (let column = 0; column < 4; column++) {
        const value = new Image(); value.src = rows[row][column]; await value.decode(); context.drawImage(value, column * 180, row * 48)
      }
      return canvas.toDataURL()
    }, panels)
    await writeFile(resolve(output, 'header-comparison.png'), Buffer.from(sheet.split(',')[1], 'base64'))
  } finally { await page.close() }
  return records
}
