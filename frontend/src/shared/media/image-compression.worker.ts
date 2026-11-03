import imageCompression from 'browser-image-compression'
/** Bundled same-origin module worker. The owner terminates it on cancel/dispose. */
self.onmessage = async (event: MessageEvent<File>) => {
  try {
    const blob = await imageCompression(event.data, {
      maxSizeMB: 2, maxWidthOrHeight: 2048, fileType: 'image/webp', initialQuality: 0.86,
      preserveExif: false, useWebWorker: false, maxIteration: 8,
      onProgress: percent => self.postMessage({ progress: percent }),
    })
    self.postMessage({ blob })
  } catch { self.postMessage({ error: 'decode-failed' }) }
}
