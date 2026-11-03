// SYNTHETIC GEOMETRIC TEST ATLAS. Never a production artwork derivative.
import { deflateSync } from 'node:zlib'
const crc32 = bytes => { let crc = 0xffffffff; for (const byte of bytes) { crc ^= byte; for (let bit = 0; bit < 8; bit++) crc = crc >>> 1 ^ (crc & 1 ? 0xedb88320 : 0) } return (crc ^ 0xffffffff) >>> 0 }
const chunk = (type, payload) => { const bytes = Buffer.concat([Buffer.from(type), payload]), size = Buffer.alloc(4), crc = Buffer.alloc(4); size.writeUInt32BE(payload.length); crc.writeUInt32BE(crc32(bytes)); return Buffer.concat([size, bytes, crc]) }
export function syntheticAvatarPng() {
  const width = 320, height = 1280, pixels = new Uint8Array(width * height * 4)
  const rectangles = [[20, 16, 24, 44, 220, 70, 60], [28, 22, 8, 4, 40, 100, 220], [20, 12, 24, 16, 50, 180, 70], [20, 36, 24, 22, 230, 190, 40], [36, 30, 8, 8, 180, 70, 210]]
  const fill = (x, y, w, h, color) => { for (let yy = y; yy < y + h; yy++) for (let xx = x; xx < x + w; xx++) pixels.set([...color, 255], (yy * width + xx) * 4) }
  for (let layer = 0; layer < 5; layer++) for (let row = 0; row < 4; row++) for (let col = 0; col < 5; col++) {
    const [x, y, w, h, r, g, b] = rectangles[layer], ox = col * 64, oy = layer * 256 + row * 64
    fill(ox + x, oy + y, w, h, [r, g, b]); fill(ox, oy, 4, 4, [20 + row * 40, 20 + col * 40, 30 + layer * 30])
  }
  const ihdr = Buffer.alloc(13); ihdr.writeUInt32BE(width); ihdr.writeUInt32BE(height, 4); ihdr[8] = 8; ihdr[9] = 6
  const scan = Buffer.alloc((width * 4 + 1) * height)
  for (let row = 0; row < height; row++) scan.set(pixels.subarray(row * width * 4, (row + 1) * width * 4), row * (width * 4 + 1) + 1)
  return new Uint8Array(Buffer.concat([Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]), chunk('IHDR', ihdr), chunk('IDAT', deflateSync(scan)), chunk('IEND', Buffer.alloc(0))]))
}
