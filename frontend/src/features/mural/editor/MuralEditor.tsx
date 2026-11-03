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
  const [view, setView] = useState<View>(fitView), [size, setSize] = useState(256), [height, setHeight] = useState(281.6), [, tick] = useState(0)
  const viewRef = useRef(view), sizeRef = useRef(size), heightRef = useRef(height)
  // A portrait viewport contains a square grid. Uniform scale and the same offset
  // are used for raster drawing and pixelAt; no CSS stretching changes hit tests.
  const boundViewport = (next: View) => {
    const result = boundView(next, sizeRef.current), grid = sizeRef.current * result.zoom, high = heightRef.current
    return { ...result, y: grid <= high ? (high - grid) / 2 : Math.min(0, Math.max(high - grid, next.y)) }
  }
  const zoomViewport = (zoom: number) => {
    const old = viewRef.current, next = zoomView(old, zoom, sizeRef.current), factor = next.zoom / old.zoom
    return boundViewport({ ...next, y: heightRef.current / 2 - (heightRef.current / 2 - old.y) * factor })
  }
  const [tool, setTool] = useState<'pencil' | 'pan'>('pencil'), toolRef = useRef(tool)
  const chooseTool = (next: 'pencil' | 'pan') => { input.cancel(); toolRef.current = next; setTool(next) }
  const updateView = (next: View) => { viewRef.current = next; setView(next) }
  const [x, setX] = useState('0'), [y, setY] = useState('0'), [invalid, setInvalid] = useState(false)
  const input = useMemo(() => createPointerInput({ cancel: () => controller.cancelSelection(),
    pan: delta => {
      const next = boundViewport({ ...viewRef.current, x: viewRef.current.x + delta.x, y: viewRef.current.y + delta.y })
      viewRef.current = next; setView(next)
    },
    select: point => { const pixel = pixelAt(point, viewRef.current, sizeRef.current); if (pixel && toolRef.current === 'pencil') controller.select(pixel.x, pixel.y) },
  }), [controller])
  const hasSnapshot = snapshot !== null
  useEffect(() => {
    const target = surface.current
    if (!target) return
    const observer = new ResizeObserver(([entry]) => {
      const width = entry?.contentRect.width ?? 256
      if (width <= 0) return
      sizeRef.current = width; heightRef.current = entry?.contentRect.height ?? width
      setSize(width); setHeight(heightRef.current)
      const next = boundViewport(viewRef.current); viewRef.current = next; setView(next)
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
      toolRef.current = 'pan'; setTool('pan')
      const next = boundViewport(fitView()); viewRef.current = next; setView(next)
    } else { toolRef.current = 'pencil'; setTool('pencil') }
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
    context.resetTransform(); context.clearRect(0, 0, 512, Math.ceil(512 * height / size)); context.scale(512 / size, 512 / size)
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
  }, [raster, snapshot, view, selection, size, height])
  const point = (event: PointerEvent<HTMLDivElement>) => {
    const rect = event.currentTarget.getBoundingClientRect()
    return { x: event.clientX - rect.left, y: event.clientY - rect.top }
  }
  const showPixel = (px: number, py: number) => {
    if (!controller.select(px, py)) return
    updateView(boundViewport({ ...viewRef.current, x: size / 2 - (px + 0.5) * size * view.zoom / 128,
      y: heightRef.current / 2 - (py + 0.5) * size * view.zoom / 128 }))
  }
  const keyboard = (event: KeyboardEvent<HTMLDivElement>) => {
    if (event.key === 'Escape') { event.preventDefault(); input.cancel(); return }
    if (!['ArrowLeft', 'ArrowRight', 'ArrowUp', 'ArrowDown'].includes(event.key)) return
    event.preventDefault()
    if (toolRef.current === 'pan') {
      updateView(boundViewport({ ...viewRef.current, x: viewRef.current.x + (event.key === 'ArrowLeft' ? size / 8 : event.key === 'ArrowRight' ? -size / 8 : 0),
        y: viewRef.current.y + (event.key === 'ArrowUp' ? heightRef.current / 8 : event.key === 'ArrowDown' ? -heightRef.current / 8 : 0) })); return
    }
    const px = selection?.x ?? 0, py = selection?.y ?? 0
    showPixel(Math.min(127, Math.max(0, px + (event.key === 'ArrowRight' ? 1 : event.key === 'ArrowLeft' ? -1 : 0))),
      Math.min(127, Math.max(0, py + (event.key === 'ArrowDown' ? 1 : event.key === 'ArrowUp' ? -1 : 0))))
  }
  const cooldown = Math.ceil(controller.remaining() / 1000)
  const busy = state.phase === 'sending' || state.phase === 'checking'
  const recovery = !!state.command || state.availability !== 'writable' || snapshot?.state !== 'EDITABLE'
  return <section className="mcr-mural" aria-label="Wall editor"><ConcertWindow title="Wall" icon="wall">
    {snapshot && <>
      <h3 className="mcr-mural__heading">{snapshot.width} × {snapshot.height} PIXEL WALL</h3>
      <div className="mcr-mural__workspace">
        <div ref={surface} className={`mcr-mural__surface mcr-mural__surface--${tool}`} role="group" aria-label="Mural pixel preview" aria-describedby={`${id}-help ${id}-keyboard`} tabIndex={0}
          onKeyDown={keyboard} onContextMenu={e => { e.preventDefault(); input.cancel() }}
          onPointerDown={e => { if (e.button !== 0 || e.pointerType === 'mouse' && !e.isPrimary) return; e.currentTarget.focus(); e.currentTarget.setPointerCapture(e.pointerId); input.down(e.pointerId, point(e)) }}
          onPointerMove={e => input.move(e.pointerId, point(e))} onPointerUp={e => input.up(e.pointerId, point(e))}
          onPointerCancel={() => input.cancel()} onLostPointerCapture={e => input.lost(e.pointerId)}>
          <canvas ref={canvas} width={512} height={Math.ceil(512 * height / size)} aria-hidden="true" />
        </div>
        <div className="mcr-mural__toolbar" role="group" aria-label="Wall view controls">
          <div className="mcr-mural__tools">
            <button type="button" aria-label="Pencil preview" title="Pencil preview" aria-pressed={tool === 'pencil'} disabled={!controller.canEdit()} onClick={() => chooseTool('pencil')}><ToolIcon name="pencil" /></button>
            <button type="button" aria-label="Pan wall" title="Pan wall" aria-pressed={tool === 'pan'} onClick={() => chooseTool('pan')}><ToolIcon name="pan" /></button>
          </div>
          <div className="mcr-mural__tools">
            <output aria-label="Zoom level">{view.zoom}×</output>
            <button type="button" aria-label="Zoom in" title="Zoom in" onClick={() => { input.cancel(); updateView(zoomViewport(view.zoom * 2)) }} disabled={view.zoom >= 8}><ToolIcon name="plus" /></button>
            <button type="button" aria-label="Zoom out" title="Zoom out" onClick={() => { input.cancel(); updateView(zoomViewport(view.zoom / 2)) }} disabled={view.zoom <= 1}><ToolIcon name="minus" /></button>
            <button type="button" aria-label="Fit wall" title="Fit wall" onClick={() => { input.cancel(); updateView(boundViewport(fitView())) }}><ToolIcon name="fit" /></button>
          </div>
        </div>
      </div>
      <fieldset className="mcr-mural__palette-frame" disabled={!controller.canEdit()}><legend className="mcr-mural__sr-only">Palette · color {state.color}</legend>
        <div className="mcr-mural__palette" role="group" aria-label="Wall palette" aria-describedby={`${id}-palette-help`} tabIndex={0}
          onKeyDown={e => { if (e.target !== e.currentTarget || !['ArrowLeft', 'ArrowRight', 'Home', 'End'].includes(e.key)) return
            e.preventDefault(); e.currentTarget.scrollLeft = e.key === 'Home' ? 0 : e.key === 'End' ? e.currentTarget.scrollWidth : e.currentTarget.scrollLeft + (e.key === 'ArrowRight' ? 48 : -48)
          }}>
          {snapshot.palette.map((hex, color) => <button key={color} type="button" aria-label={`Color ${color} ${hex}`} title={`Color ${color} ${hex}`}
            aria-pressed={state.color === color} onClick={() => controller.chooseColor(color)}><span className="mcr-mural__swatch" style={{ backgroundColor: hex }} aria-hidden="true" />
            {state.color === color && <span className="mcr-mural__color-marker" aria-hidden="true">✓</span>}</button>)}
        </div>
      </fieldset>
      <p id={`${id}-palette-help`} className="mcr-mural__sr-only">{snapshot.palette.length} server colors. Scroll horizontally or focus this palette and use Left, Right, Home or End. Tab reaches every color.</p>
      {(selection || state.command) && <div className="mcr-mural__preview" aria-label="Selection preview">
        <div><strong>PREVIEW ONLY</strong><p>{selection ? `Pixel (${selection.x}, ${selection.y}) · color ${selection.color_index}` : 'No pixel selected.'}</p>
          {selection && <p>Confirmed color: {snapshot.pixels[selection.y * 128 + selection.x]}.</p>}</div>
        <ConcertAction className="mcr-mural__confirm" pending={state.phase === 'sending'} disabled={!controller.canConfirm()} onClick={() => void controller.confirm()}>Confirm pixel</ConcertAction>
        <button type="button" aria-label="Cancel preview" title="Cancel preview" disabled={!selection || busy} onClick={() => controller.cancelSelection()}><span aria-hidden="true">×</span></button>
      </div>}
    </>}
    <div className="mcr-mural__meta"><span>{snapshot ? `REV ${snapshot.revision} · ${snapshot.state}` : 'No confirmed wall'}</span>
      <span>{state.command ? 'Placement unresolved' : state.availability !== 'writable' ? state.availability : cooldown > 0 ? `Cooldown: ${cooldown}s` : snapshot ? `${snapshot.palette.length} colors · scroll →` : 'Loading'}</span></div>
    <div className="mcr-mural__status"><p role="status" aria-label="Wall command status" aria-live="polite">{state.message}</p>{synchronizationStatus}</div>
    {recovery && <div className="mcr-mural__recovery">{state.command && <button type="button" disabled={busy} onClick={() => void controller.check()}>Check original command</button>}
      {onRefresh && <button type="button" disabled={busy} onClick={onRefresh}>Refresh confirmed wall</button>}</div>}
    {snapshot && <details className="mcr-mural__details"><summary>Coordinates &amp; help</summary>
      <p id={`${id}-help`}>Pencil previews one pixel; drag to pan. Pan mode never selects. Only Confirm pixel sends a placement.</p>
      <p id={`${id}-keyboard`}>In Pencil mode, arrow keys select a pixel. In Pan mode, arrow keys move the view. Escape cancels. Coordinates work without the canvas.</p>
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
      <p>The server decides cooldown and region permissions. Completed pixels cannot be undone here.</p>
      {!recovery && onRefresh && <button type="button" disabled={busy} onClick={onRefresh}>Refresh confirmed wall</button>}
    </details>}
  </ConcertWindow></section>
}

function ToolIcon({ name }: { name: 'pencil' | 'pan' | 'plus' | 'minus' | 'fit' }) {
  return <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="square" strokeLinejoin="miter" aria-hidden="true" focusable="false">
    {name === 'pencil' ? <path d="m4 15 11-11 5 5L9 20l-6 1 1-6Zm9-9 5 5M4 15l5 5" />
      : name === 'pan' ? <><path d="M8 13V5a2 2 0 0 1 4 0v7-9a2 2 0 0 1 4 0v9-6a2 2 0 0 1 4 0v10c0 5-3 6-7 6-3 0-5-2-7-5l-3-4c-1-2 1-4 3-2l2 2Z" /></>
      : name === 'plus' ? <path d="M12 4v16M4 12h16" /> : name === 'minus' ? <path d="M4 12h16" />
      : <path d="M3 9V3h6M15 3h6v6M21 15v6h-6M9 21H3v-6M8 8h8v8H8Z" />}
  </svg>
}
