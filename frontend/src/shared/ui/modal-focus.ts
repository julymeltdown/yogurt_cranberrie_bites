// Newly authored DOM focus/isolation support; no source UI or dialog polyfill.
export interface ModalLayer {
  id: string
  parentId: string | null
  overlay: HTMLElement
  panel: HTMLElement
  focus: () => void
  returnTarget: HTMLElement | null
}

const layers: ModalLayer[] = []
const inertBefore = new Map<Element, boolean>()
let overflowBefore = ''
let observer: MutationObserver | undefined

export function topModal() {
  return layers.at(-1)
}

function isolate() {
  const top = topModal()
  if (!top) return
  layers.forEach((layer, index) => layer.overlay.style.setProperty('--mcr-modal-depth', String(index)))
  for (const element of document.body.children) {
    if (!inertBefore.has(element)) inertBefore.set(element, element.hasAttribute('inert'))
    element.toggleAttribute('inert', element !== top.overlay)
  }
}

export function addModal(layer: ModalLayer) {
  if (layers.length === 0) {
    overflowBefore = document.body.style.overflow
    document.body.style.overflow = 'hidden'
    observer = new MutationObserver(isolate)
    observer.observe(document.body, { childList: true })
  }
  // React runs child layout effects first when nested dialogs mount together.
  const childIndex = layers.findIndex(child => child.parentId === layer.id)
  if (childIndex === -1) layers.push(layer)
  else {
    const child = layers[childIndex]!
    // The child may already have focused itself before the parent effect runs.
    if (layer.returnTarget && child.panel.contains(layer.returnTarget)) layer.returnTarget = child.returnTarget
    layers.splice(childIndex, 0, layer)
  }
  isolate()
  return () => {
    const index = layers.indexOf(layer)
    // If a parent closes before its child, preserve the route back to the page.
    const child = layers[index + 1]
    if (index !== -1 && child?.returnTarget && layer.panel.contains(child.returnTarget)) {
      child.returnTarget = layer.returnTarget
    }
    if (index !== -1) layers.splice(index, 1)
    if (layers.length) {
      isolate()
    } else {
      observer?.disconnect()
      observer = undefined
      for (const [element, wasInert] of inertBefore) element.toggleAttribute('inert', wasInert)
      inertBefore.clear()
      document.body.style.overflow = overflowBefore
    }
  }
}

export function canFocus(element: HTMLElement, panel: HTMLElement) {
  if (!element.isConnected || !panel.contains(element) || element.matches(':disabled')) return false
  for (let node: HTMLElement | null = element; node; node = node.parentElement) {
    if (node.hidden || node.hasAttribute('inert') || node.getAttribute('aria-hidden') === 'true') return false
    const style = getComputedStyle(node)
    if (style.display === 'none' || style.visibility === 'hidden' || style.visibility === 'collapse') return false
    if (node === panel) break
  }
  return true
}

export function tabStops(panel: HTMLElement) {
  const elements = [...panel.querySelectorAll<HTMLElement>(
    'a[href],area[href],button,input,select,textarea,iframe,[contenteditable="true"],[tabindex]',
  )].filter(element => element.tabIndex >= 0 && canFocus(element, panel))
  // A radio group has one tab stop: the checked item, or its first enabled item.
  const stops = elements.filter(element => {
    if (!(element instanceof HTMLInputElement) || element.type !== 'radio' || !element.name) return true
    const group = elements.filter((other): other is HTMLInputElement => other instanceof HTMLInputElement
      && other.type === 'radio' && other.name === element.name && other.form === element.form)
    return element === (group.find(other => other.checked) ?? group[0])
  })
  return stops.sort((a, b) => (a.tabIndex > 0 ? a.tabIndex : Infinity)
    - (b.tabIndex > 0 ? b.tabIndex : Infinity))
}
