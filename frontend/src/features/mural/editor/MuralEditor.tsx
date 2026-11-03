import { useEffect, useId, useMemo, useRef, useState, useSyncExternalStore } from 'react'
import type { KeyboardEvent, PointerEvent, ReactNode } from 'react'
import type { MuralEditorController } from './controller'
import { boundView, createPointerInput, fitView, pixelAt, zoomView } from './input'
import type { View } from './input'
import { ConcertAction, ConcertWindow } from '../../../shared/ui/concert-window'
import './editor.css'

export function MuralEditor({ controller, onRefresh, synchronizationStatus }: { controller: MuralEditorController; onRefresh?: () => void; synchronizationStatus?: ReactNode }) {
  const state = useSyncExternalStore(controller.subscribe, controller.read, controller.read)
  const { snapshot, selection } = state
  const id = useId(), canvas = useRef<HTMLCanvasElement>(null), surface = useRef<HTMLDivElement>(null)
  const [view, setView] = useState<View>(fitView), [size, setSize] = useState(256), [, tick] = useState(0)
  const viewRef = useRef(view), sizeRef = useRef(size)
  const updateView = (next: View) => { viewRef.current = next; setView(next) }
  const [x, setX] = useState('0'), [y, setY] = useState('0'), [invalid, setInvalid] = useState(false)
  const input = useMemo(() => createPointerInput({ cancel: () => controller.cancelSelection(),
    pan: delta => {
      const next = boundView({ ...viewRef.current, x: viewRef.current.x + delta.x, y: viewRef.current.y + delta.y }, sizeRef.current)
      viewRef.current = next; setView(next)
    },
    select: point => { const pixel = pixelAt(point, viewRef.current, sizeRef.current); if (pixel) controller.select(pixel.x, pixel.y) },
  }), [controller])
  const hasSnapshot = snapshot !== null
  useEffect(() => {
    const target = surface.current
    if (!target) return
    const observer = new ResizeObserver(([entry]) => {
      const width = entry?.contentRect.width ?? 256
      if (width <= 0) return
      sizeRef.current = width; setSize(width)
      const next = boundView(viewRef.current, width); viewRef.current = next; setView(next)
    })
    observer.observe(target)
    return () => observer.disconnect()
  }, [hasSnapshot])
  useEffect(() => {
    const cancel = () => input.cancel()
    const hidden = () => { if (document.hidden) cancel() }
    window.addEventListener('blur', cancel)
    document.addEventListener('visibilitychange', hidden)
    return () => { window.removeEventListener('blur', cancel); document.removeEventListener('visibilitychange', hidden); input.cancel() }
  }, [input])
  useEffect(() => {
    const timer = window.setInterval(() => tick(n => n + 1), 250)
    return () => window.clearInterval(timer)
  }, [])
  useEffect(() => {
    if (!snapshot || !controller.canEdit()) {
      input.cancel(); setX('0'); setY('0'); setInvalid(false)
      viewRef.current = fitView(); setView(fitView())
    }
  }, [controller, input, hasSnapshot, state.availability, snapshot?.state])
  useEffect(() => { if (selection) { setX(String(selection.x)); setY(String(selection.y)); setInvalid(false) } }, [selection])
  // The offscreen raster is bounded at 128², with no image URLs, bitmaps, workers, or per-pixel DOM.
  const raster = useMemo(() => {
    if (!snapshot) return null
    const image = document.createElement('canvas'); image.width = image.height = 128
    const context = image.getContext('2d')
    if (!context) return null
    const data = context.createImageData(128, 128)
    for (let index = 0; index < 16384; index++) {
      const hex = snapshot.palette[snapshot.pixels[index]!]!
      data.data[index * 4] = Number.parseInt(hex.slice(1, 3), 16)
      data.data[index * 4 + 1] = Number.parseInt(hex.slice(3, 5), 16)
      data.data[index * 4 + 2] = Number.parseInt(hex.slice(5, 7), 16)
      data.data[index * 4 + 3] = 255
    }
    context.putImageData(data, 0, 0); return image
  }, [snapshot])
  useEffect(() => {
    const context = canvas.current?.getContext('2d')
    if (!context || !raster || !snapshot) return
    context.resetTransform(); context.clearRect(0, 0, 512, 512); context.scale(512 / size, 512 / size)
    context.imageSmoothingEnabled = false
    context.drawImage(raster, view.x, view.y, size * view.zoom, size * view.zoom)
    const cell = size * view.zoom / 128
    if (view.zoom >= 4) {
      context.beginPath(); context.strokeStyle = '#808080'; context.lineWidth = 0.5
      for (let i = 0; i <= 128; i++) {
        const a = view.x + i * cell, b = view.y + i * cell
        context.moveTo(a, view.y); context.lineTo(a, view.y + 128 * cell)
        context.moveTo(view.x, b); context.lineTo(view.x + 128 * cell, b)
      }
      context.stroke()
    }
    if (selection) {
      const left = view.x + selection.x * cell, top = view.y + selection.y * cell
      context.fillStyle = snapshot.palette[selection.color_index]!
      context.globalAlpha = 0.6; context.fillRect(left, top, cell, cell); context.globalAlpha = 1
      context.strokeStyle = '#FFFFFF'; context.lineWidth = 4; context.strokeRect(left - 1, top - 1, cell + 2, cell + 2)
      context.strokeStyle = '#000000'; context.lineWidth = 2; context.setLineDash([3, 2]); context.strokeRect(left - 1, top - 1, cell + 2, cell + 2); context.setLineDash([])
    }
  }, [raster, snapshot, view, selection, size])
  const point = (event: PointerEvent<HTMLDivElement>) => {
    const rect = event.currentTarget.getBoundingClientRect()
    return { x: event.clientX - rect.left, y: event.clientY - rect.top }
  }
  const showPixel = (px: number, py: number) => {
    if (!controller.select(px, py)) return
    updateView(boundView({ ...viewRef.current, x: size / 2 - (px + 0.5) * size * view.zoom / 128,
      y: size / 2 - (py + 0.5) * size * view.zoom / 128 }, size))
  }
  const keyboard = (event: KeyboardEvent<HTMLDivElement>) => {
    if (event.key === 'Escape') { event.preventDefault(); input.cancel(); return }
    if (!['ArrowLeft', 'ArrowRight', 'ArrowUp', 'ArrowDown'].includes(event.key)) return
    event.preventDefault()
    const px = selection?.x ?? 0, py = selection?.y ?? 0
    showPixel(Math.min(127, Math.max(0, px + (event.key === 'ArrowRight' ? 1 : event.key === 'ArrowLeft' ? -1 : 0))),
      Math.min(127, Math.max(0, py + (event.key === 'ArrowDown' ? 1 : event.key === 'ArrowUp' ? -1 : 0))))
  }
  const cooldown = Math.ceil(controller.remaining() / 1000)
  const busy = state.phase === 'sending' || state.phase === 'checking'
  return <section className="mcr-mural" aria-label="Wall editor"><ConcertWindow title="Wall" icon="wall">
    <div className="mcr-mural__meta"><span>{snapshot ? `Confirmed revision ${snapshot.revision}` : 'No confirmed wall'}</span>
      <span>{state.command ? 'Placement unresolved' : state.availability !== 'writable' ? state.availability : cooldown > 0 ? `Cooldown: ${cooldown}s` : snapshot?.state}</span></div>
    <div className="mcr-mural__status"><p role="status" aria-label="Wall command status" aria-live="polite">{state.message}</p>{synchronizationStatus}</div>
    <p id={`${id}-help`} className="mcr-mural__help">Tap to preview. Drag to pan. Only Confirm pixel sends a placement.</p>
    {snapshot && <>
      <div className="mcr-mural__toolbar" aria-label="Wall view controls">
        <button type="button" onClick={() => { input.cancel(); updateView(zoomView(view, view.zoom / 2, size)) }} disabled={view.zoom <= 1}>Zoom out</button>
        <output aria-label="Zoom level">{view.zoom}×</output>
        <button type="button" onClick={() => { input.cancel(); updateView(zoomView(view, view.zoom * 2, size)) }} disabled={view.zoom >= 8}>Zoom in</button>
        <button type="button" onClick={() => { input.cancel(); updateView(fitView()) }}>Fit wall</button>
      </div>
      <div ref={surface} className="mcr-mural__surface" role="group" aria-label="Mural pixel preview" aria-describedby={`${id}-help ${id}-keyboard`} tabIndex={0}
        onKeyDown={keyboard} onContextMenu={e => { e.preventDefault(); input.cancel() }}
        onPointerDown={e => { if (e.button !== 0 || e.pointerType === 'mouse' && !e.isPrimary) return; e.currentTarget.focus(); e.currentTarget.setPointerCapture(e.pointerId); input.down(e.pointerId, point(e)) }}
        onPointerMove={e => input.move(e.pointerId, point(e))} onPointerUp={e => input.up(e.pointerId, point(e))}
        onPointerCancel={() => input.cancel()} onLostPointerCapture={e => input.lost(e.pointerId)}>
        <canvas ref={canvas} width={512} height={512} aria-hidden="true" />
      </div>
      <fieldset disabled={!controller.canEdit()}><legend>Palette · color {state.color}</legend>
        <div className="mcr-mural__palette">{snapshot.palette.map((hex, color) => <button key={color} type="button" aria-label={`Color ${color} ${hex}`}
          aria-pressed={state.color === color} onClick={() => controller.chooseColor(color)}><span className="mcr-mural__swatch" style={{ backgroundColor: hex }} aria-hidden="true" /><span>{color}{state.color === color ? ' ✓' : ''}</span></button>)}</div>
      </fieldset>
      <div className="mcr-mural__preview" aria-label="Selection preview">
        <strong>PREVIEW ONLY</strong>
        <p>{selection ? `Pixel (${selection.x}, ${selection.y}) · color ${selection.color_index}. Confirmed color: ${snapshot.pixels[selection.y * 128 + selection.x]}.` : 'No pixel selected.'}</p>
        <p>{cooldown > 0 ? `Cooldown: ${cooldown}s remaining. The server decides when you can place.` : 'Server cooldown and region permissions apply.'}</p>
      </div>
      <div className="mcr-mural__actions">
        <ConcertAction className="mcr-mural__confirm" pending={state.phase === 'sending'} disabled={!controller.canConfirm()} onClick={() => void controller.confirm()}>Confirm pixel</ConcertAction>
        <button type="button" disabled={!selection || busy} onClick={() => controller.cancelSelection()}>Cancel preview</button>
      </div>
      <fieldset disabled={!controller.canEdit()}><legend>Pixel coordinates · 0–127</legend>
        <form className="mcr-mural__coordinates" onSubmit={e => { e.preventDefault();
          const valid = /^\d{1,3}$/.test(x) && /^\d{1,3}$/.test(y) && Number(x) <= 127 && Number(y) <= 127
          setInvalid(!valid); if (valid) showPixel(Number(x), Number(y))
        }}>
          <label>X<input type="number" min={0} max={127} step={1} value={x} aria-invalid={invalid} onChange={e => { controller.cancelSelection(); setX(e.target.value) }} /></label>
          <label>Y<input type="number" min={0} max={127} step={1} value={y} aria-invalid={invalid} onChange={e => { controller.cancelSelection(); setY(e.target.value) }} /></label>
          <button type="submit">Preview pixel</button>
        </form>
        {invalid && <p role="alert">Use whole coordinates from 0 to 127.</p>}
      </fieldset>
      <p id={`${id}-keyboard`} className="mcr-mural__help">Arrow keys select a pixel; Escape cancels. Coordinates work without the canvas.</p>
    </>}
    <div className="mcr-mural__recovery">{state.command && <button type="button" disabled={busy} onClick={() => void controller.check()}>Check original command</button>}
    {onRefresh && <button type="button" disabled={busy} onClick={onRefresh}>Refresh confirmed wall</button>}</div>
  </ConcertWindow>
  </section>
}
