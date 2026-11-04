import type { ReactElement } from 'react'
import { createRoot } from 'react-dom/client'
import { flushSync } from 'react-dom'
import { assertAbsent, check, type Role } from './oracle'

export const mountedUI: { domain: string; role: Role; qualification: string; assertions: string[] }[] = []

/** Real components, synthetic controllers/transport. No product shell or service acceptance. */
export function mountPrivateView(domain: string, role: Role, view: ReactElement) {
  const host = document.createElement('div')
  host.dataset.qa04Fixture = `${domain}:${role}`
  document.body.append(host)
  const root = createRoot(host), assertions: string[] = []
  flushSync(() => root.render(view))
  const snapshot = () => {
    flushSync(() => undefined)
    return { markup: host.innerHTML, controls: [...host.querySelectorAll<HTMLInputElement | HTMLTextAreaElement>('input,textarea')].map(input => input.value) }
  }
  return {
    visible(marker: string) {
      snapshot()
      check(host.textContent?.includes(marker), `${domain}/${role} authorized marker rendered in actual component`)
      assertions.push('authorized synthetic private text visible')
    },
    absent(reason: string, extraValues: string[] = []) {
      const state = snapshot()
      assertAbsent(state)
      for (const value of extraValues) check(!JSON.stringify(state).includes(value), `${domain}/${role} extra private control value absent`)
      assertions.push(reason + ': no private markers in DOM/attributes/control values')
    },
    noImage(reason: string) {
      snapshot()
      check(!host.querySelector('img'), `${domain}/${role} ${reason}`)
      assertions.push(reason + ': no image element retained')
    },
    image(url: string) {
      snapshot()
      check([...host.querySelectorAll('img')].some(img => img.src === url), 'authorized evidence URL assigned to real image element')
      assertions.push('authorized synthetic Blob URL assigned; image decode not qualified')
    },
    text(value: string) {
      snapshot()
      check(host.textContent?.includes(value), `${domain}/${role} expected control output visible`)
      assertions.push('authorized control output visible')
    },
    click(label: string) {
      snapshot()
      const button = [...host.querySelectorAll('button')].find(button => button.textContent?.trim() === label)
      check(button && !button.disabled, `${domain}/${role} explicit enabled ${label} control`)
      flushSync(() => button.click())
    },
    finish() {
      check(assertions.length > 0, 'mounted scenario has assertions')
      mountedUI.push({ domain, role, qualification: 'SYNTHETIC_MOUNTED_COMPONENT_ONLY', assertions })
    },
    dispose() { flushSync(() => root.unmount()); host.remove() },
  }
}
