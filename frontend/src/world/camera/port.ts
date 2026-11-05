export interface Point { readonly x: number; readonly y: number }
export interface CameraSnapshot { readonly zoom: number; readonly scrollX: number; readonly scrollY: number; readonly width: number; readonly height: number }
/** CSS-client coordinates at the boundary; implementations own canvas scaling and actual camera transforms. */
export interface WorldCamera {
  surface(): HTMLCanvasElement | null
  center(foot: Point, zoom: number): void
  project(client: Point): Point | null
  unproject(world: Point): Point | null
  read(): CameraSnapshot | null
}
export function createCameraControl(view: WorldCamera, foot: Point, zoom: number) {
  if (!Number.isFinite(zoom) || zoom < 1 || zoom > 8) throw new Error('Invalid local camera policy')
  let latest = foot, lookedAt = foot, currentZoom = zoom, following = true, disposed = false
  const valid = (p: Point) => Number.isFinite(p.x) && Number.isFinite(p.y)
  return {
    update(point: Point) { if (disposed || !valid(point)) return; latest = { ...point }; if (following) view.center(latest, currentZoom) },
    lookAt(point: Point) { if (disposed || !valid(point)) return; following = false; lookedAt = { ...point }; view.center(point, currentZoom) },
    recenter() { if (disposed) return; following = true; view.center(latest, currentZoom) },
    project: (client: Point) => disposed || !valid(client) ? null : view.project(client),
    unproject: (point: Point) => disposed || !valid(point) ? null : view.unproject(point),
    setZoom(value: number) { if (disposed || !Number.isFinite(value) || value < 1 || value > 8) return false; currentZoom = value; view.center(following ? latest : lookedAt, currentZoom); return true },
    read: () => { const camera = view.read(); return { following, camera: camera ? { ...camera, zoom: currentZoom } : null } },
    dispose() { disposed = true },
  }
}
