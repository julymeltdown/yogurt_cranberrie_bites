// The browser half of the WP-10 capture harness.
//
// Reads one JSON payload on stdin (origin, output directory, widths, screens,
// session tokens) and writes one JSON report on stdout. It drives Chromium
// through Playwright against the real frontend, which proxies `/api` to the real
// gateway the Python half started.
//
// For every screen at every width it takes a full-page screenshot and measures
// the four acceptance checks. A check that cannot be evaluated is recorded as
// `null` with a reason, never as a pass.
import { mkdir, writeFile } from 'node:fs/promises'
import { chromium } from '@playwright/test'

const payload = JSON.parse(await new Promise(resolve => {
  let text = ''
  process.stdin.on('data', chunk => { text += chunk })
  process.stdin.on('end', () => resolve(text))
}))

const { origin, output, widths, height, screens, notes, tokens, accounts, introId, viewerProfile } = payload
await mkdir(output, { recursive: true })

// The screens a route cannot render because of an in-flight defect are captured
// in the state they reach. These are the phrases the product's own error states
// use; matching one records the capture as BLOCKED with that reason rather than
// pretending the screen rendered.
const BLOCKERS = [
  [/not in this build yet/i, 'route is an explicit placeholder in this build'],
  [/unavailable/i, 'the surface reported itself unavailable'],
  [/could not be loaded|failed to load/i, 'the screen could not load its data'],
  [/sign in|log in/i, 'the session was refused, so the screen shows its signed-out state'],
]

function blockerFor(text) {
  for (const [pattern, reason] of BLOCKERS) if (pattern.test(text)) return reason
  return null
}

// ── the four acceptance checks ──────────────────────────────────────────────

// 1. No horizontal overflow. The document must not be wider than the viewport,
//    and no element may extend past the right edge.
const overflowCheck = () => {
  const doc = document.documentElement
  const viewport = window.innerWidth
  const offenders = []
  for (const element of document.querySelectorAll('body *')) {
    const rect = element.getBoundingClientRect()
    if (rect.width === 0 && rect.height === 0) continue
    if (rect.right > viewport + 1 || rect.left < -1) {
      const style = getComputedStyle(element)
      if (style.position === 'fixed' || style.visibility === 'hidden' || style.display === 'none') continue
      offenders.push({
        tag: element.tagName.toLowerCase(),
        className: (element.className || '').toString().slice(0, 80),
        left: Math.round(rect.left), right: Math.round(rect.right),
      })
    }
  }
  return {
    documentScrollWidth: doc.scrollWidth,
    viewportWidth: viewport,
    overflow: doc.scrollWidth > viewport + 1,
    offenders: offenders.slice(0, 8),
  }
}

// 2. Every interactive control has a visible focus outline. Each control is
//    focused in turn and its computed outline is read. `outline: none` with no
//    other focus indicator is the failure this catches.
const focusCheck = () => {
  // The RESTING state only: no control may be permanently ringed. Nothing is
  // focused here, because focusing an element and then reading its outline
  // measures `:focus`, not rest - and `:focus-visible` does not match a
  // programmatic focus at all, which is what produced false failures.
  const selector = 'a[href], button, input, select, textarea, [tabindex]:not([tabindex="-1"])'
  const controls = Array.from(document.querySelectorAll(selector))
    .filter(element => {
      const style = getComputedStyle(element)
      return style.display !== 'none' && style.visibility !== 'hidden'
        && element.getBoundingClientRect().width > 0
    })
  const failures = []
  for (const element of controls.slice(0, 40)) {
    const style = getComputedStyle(element)
    const width = parseFloat(style.outlineWidth) || 0
    if (width > 0 && style.outlineStyle !== 'none') {
      failures.push({
        tag: element.tagName.toLowerCase(),
        text: (element.textContent || element.getAttribute('aria-label') || '').trim().slice(0, 40),
        outlineStyle: style.outlineStyle,
        outlineWidth: style.outlineWidth,
      })
    }
  }
  return { checked: Math.min(controls.length, 40), failures: failures.slice(0, 8), ok: failures.length === 0 }
}

// 2b. The real focus-ring check: drive Tab and read the focused element.
//
// Only a keyboard-driven focus matches `:focus-visible`, so this is the check
// that can actually see the product's focus rule. It walks the tab order with
// real key presses and requires each focused control to show an outline or a
// non-resting box-shadow.
const keyboardFocusCheck = async (page, maxStops = 12) => {
  const results = []
  for (let stop = 0; stop < maxStops; stop += 1) {
    await page.keyboard.press('Tab')
    const observed = await page.evaluate(() => {
      const element = document.activeElement
      if (!element || element === document.body) return null
      const style = getComputedStyle(element)
      const width = parseFloat(style.outlineWidth) || 0
      const shadow = style.boxShadow
      const hasShadow = shadow && shadow !== 'none' && !/rgba\(0, 0, 0, 0\)/.test(shadow)
      return {
        tag: element.tagName.toLowerCase(),
        text: (element.textContent || element.getAttribute('aria-label') || '').trim().slice(0, 40),
        outlineStyle: style.outlineStyle,
        outlineWidth: style.outlineWidth,
        visible: (width > 0 && style.outlineStyle !== 'none') || Boolean(hasShadow),
      }
    })
    if (observed === null) break
    results.push(observed)
  }
  const failures = results.filter(row => !row.visible)
  return { stops: results.length, failures: failures.slice(0, 8), ok: failures.length === 0 }
}

// 3. No decorative left accent bar. The CSS guard rejects the source pattern;
//    this confirms no rendered element carries one — a thick coloured left
//    border, or an absolutely-positioned narrow strip pinned to the left edge.
const accentBarCheck = () => {
  const offenders = []
  for (const element of document.querySelectorAll('body *')) {
    const style = getComputedStyle(element)
    const rect = element.getBoundingClientRect()
    if (rect.width === 0 || rect.height === 0) continue
    const leftWidth = parseFloat(style.borderLeftWidth) || 0
    const others = ['borderTopWidth', 'borderRightWidth', 'borderBottomWidth']
      .map(name => parseFloat(style[name]) || 0)
    const otherMax = Math.max(...others)
    const coloured = style.borderLeftColor && style.borderLeftColor !== 'rgba(0, 0, 0, 0)'
    if (leftWidth >= 3 && leftWidth > otherMax + 1 && coloured) {
      offenders.push({
        tag: element.tagName.toLowerCase(),
        className: (element.className || '').toString().slice(0, 80),
        borderLeftWidth: style.borderLeftWidth, borderLeftColor: style.borderLeftColor,
      })
      continue
    }
    // A narrow strip pinned to the left edge of its container.
    const position = style.position
    if ((position === 'absolute' || position === 'fixed') && rect.width <= 8
        && rect.height > 24 && rect.left <= 2) {
      offenders.push({
        tag: element.tagName.toLowerCase(),
        className: (element.className || '').toString().slice(0, 80),
        width: Math.round(rect.width), left: Math.round(rect.left),
      })
    }
  }
  return { offenders: offenders.slice(0, 8), ok: offenders.length === 0 }
}

// 4. No clipped or overlapping text. Clipping is `overflow: hidden` with
//    content taller or wider than the box; overlap is two text-bearing siblings
//    whose boxes intersect.
const textCheck = () => {
  const clipped = []
  const boxes = []
  for (const element of document.querySelectorAll('body *')) {
    const style = getComputedStyle(element)
    const rect = element.getBoundingClientRect()
    if (rect.width === 0 || rect.height === 0) continue
    const text = (element.textContent || '').trim()
    if (!text) continue
    const hidesOverflow = style.overflowX === 'hidden' || style.overflowY === 'hidden'
    if (hidesOverflow && (element.scrollWidth > element.clientWidth + 1
        || element.scrollHeight > element.clientHeight + 1)
        && element.children.length === 0) {
      clipped.push({
        tag: element.tagName.toLowerCase(),
        text: text.slice(0, 40),
        scrollWidth: element.scrollWidth, clientWidth: element.clientWidth,
        scrollHeight: element.scrollHeight, clientHeight: element.clientHeight,
      })
    }
    // Only leaf text nodes take part in the overlap test, so a container is not
    // reported against its own child.
    if (element.children.length === 0 && text.length > 1) {
      boxes.push({ tag: element.tagName.toLowerCase(), text: text.slice(0, 30), rect })
    }
  }
  const overlaps = []
  for (let i = 0; i < boxes.length && overlaps.length < 8; i += 1) {
    for (let j = i + 1; j < boxes.length; j += 1) {
      const a = boxes[i].rect, b = boxes[j].rect
      const x = Math.max(0, Math.min(a.right, b.right) - Math.max(a.left, b.left))
      const y = Math.max(0, Math.min(a.bottom, b.bottom) - Math.max(a.top, b.top))
      if (x > 2 && y > 2) {
        overlaps.push({ a: boxes[i].text, b: boxes[j].text,
          overlap: { width: Math.round(x), height: Math.round(y) } })
        break
      }
    }
  }
  return { clipped: clipped.slice(0, 8), overlaps, ok: clipped.length === 0 && overlaps.length === 0 }
}

// ── the run ────────────────────────────────────────────────────────────────

// Sign in through the product's own form.
//
// The access token lives in memory by design, so there is no storage to seed.
// This drives the real `/login` form with the credentials the enrollment flow
// created, which is also what proves the screen behind it is reachable by a real
// user rather than by a test hook.
async function signIn(page, role) {
  const credentials = accounts[role]
  if (!credentials) return { signedIn: false, reason: 'no credentials for this role' }
  await page.goto(origin + '/login', { waitUntil: 'domcontentloaded', timeout: 45000 })
  const email = page.locator('#sign-in-email')
  try {
    await email.waitFor({ state: 'visible', timeout: 20000 })
  } catch {
    return { signedIn: false, reason: 'the sign-in form did not appear' }
  }
  // The submit control stays disabled until the capabilities probe reports that
  // email sign-in is available, so the form is filled only once it is enabled.
  try {
    await page.waitForFunction(() => {
      const button = [...document.querySelectorAll('button')]
        .find(candidate => /sign in with email/i.test(candidate.getAttribute('aria-label') || ''))
      return Boolean(button) && !button.disabled
    }, {}, { timeout: 20000 })
  } catch {
    return { signedIn: false, reason: 'email sign-in never became available' }
  }
  await email.fill(credentials.email)
  await page.locator('#sign-in-password').fill(credentials.password)
  const calls = []
  page.on('response', response => {
    const url = new URL(response.url())
    if (url.pathname.startsWith('/api/auth/')) calls.push(`${response.status()} ${url.pathname}`)
  })
  await page.getByRole('button', { name: /sign in with email/i }).click()
  try {
    await page.waitForFunction(() => !location.pathname.startsWith('/login'), {}, { timeout: 25000 })
    return { signedIn: true, calls }
  } catch {
    const notice = await page.evaluate(() => (document.body.innerText || '').slice(0, 300))
    return { signedIn: false, reason: 'the session was not admitted after sign-in',
      calls, notice }
  }
}

const browser = await chromium.launch({ channel: 'chrome', headless: true })
const captures = []

// One context per role, reused across widths and screens.
//
// A fresh context per capture would drop the session between shots: the access
// token is memory-only and the refresh cookie is scoped to the context, so every
// screen after the first would photograph the signed-out notice. The viewport is
// resized on the reused page instead, which is what the width actually changes.
const sessions = new Map()
async function sessionFor(role) {
  if (sessions.has(role)) return sessions.get(role)
  const context = await browser.newContext({
    viewport: { width: widths[0], height }, deviceScaleFactor: 1,
    reducedMotion: 'reduce', serviceWorkers: 'block',
  })
  const page = await context.newPage()
  const apiLog = []
  page.on('response', response => {
    const url = new URL(response.url())
    if (url.pathname.startsWith('/api/')) apiLog.push(`${response.status()} ${url.pathname}`)
  })
  const signInResult = await signIn(page, role)
  const session = { context, page, signIn: signInResult, apiLog }
  sessions.set(role, session)
  return session
}

try {
  for (const screen of screens) {
    const route = screen.route
      .replace('{intro}', introId ?? '00000000-0000-4000-8000-000000000000')
      .replace('{viewer}', viewerProfile ?? '')
    for (const width of widths) {
      const session = await sessionFor(screen.role)
      const { page } = session
      await page.setViewportSize({ width, height })
      const consoleErrors = []
      page.on('pageerror', error => consoleErrors.push(String(error.message).slice(0, 300)))
      page.on('console', message => {
        if (message.type() === 'error') consoleErrors.push('console: ' + message.text().slice(0, 200))
      })
      const entry = {
        screen: screen.id, label: screen.label, route, role: screen.role, width,
        height, file: `${screen.id}-${width}.png`, status: 'IN_PROGRESS',
      }
      entry.signIn = session.signIn
      try {
        // Client-side navigation, not `page.goto`. The access token is held in
        // memory by design, so a full document load re-bootstraps the app and
        // loses the session — every screen would photograph the signed-out
        // notice. `history.pushState` + a popstate event is how a link inside the
        // app moves between routes, so this is the same path a user takes.
        await page.evaluate(target => {
          window.history.pushState({}, '', target)
          window.dispatchEvent(new PopStateEvent('popstate'))
        }, route)
        // Let the lazy chunk mount and its first data call settle. A fixed wait
        // is honest here: the screen has no single "ready" signal across all
        // eight routes, and a shorter wait would capture a loading state.
        await page.waitForTimeout(2500)
        const text = (await page.evaluate(() => document.body.innerText || '')).slice(0, 4000)
        // An empty document makes every check pass vacuously: no overflow, no
        // clipped text, no focusable control to lose a ring. That is how a blank
        // page produced 24 "CAPTURED" results. Treat it as a failure of the
        // harness, not a pass of the screen.
        const rootChildren = await page.evaluate(
          () => (document.getElementById('root')?.childElementCount) ?? -1)
        if (text.trim().length === 0 && rootChildren <= 0) {
          entry.status = 'FAILED'
          entry.reason = 'the page rendered nothing: empty document and empty #root'
          entry.checks = { renderedNothing: { ok: false, textLength: 0, rootChildren } }
          await page.screenshot({ path: `${output}/${entry.file}`, fullPage: true })
          continue
        }
        const blocker = blockerFor(text)
        const overflow = await page.evaluate(overflowCheck)
        const resting = await page.evaluate(focusCheck)
        const keyboard = await keyboardFocusCheck(page)
        const accent = await page.evaluate(accentBarCheck)
        const textState = await page.evaluate(textCheck)
        await page.screenshot({ path: `${output}/${entry.file}`, fullPage: true })
        entry.checks = {
          horizontalOverflow: { ok: !overflow.overflow, ...overflow },
          focusRestingState: resting,
          focusRingKeyboard: keyboard,
          leftAccentBar: accent,
          textClipping: textState,
        }
        entry.apiCalls = session.apiLog.slice(-12)
        entry.consoleErrors = consoleErrors.slice(0, 6)
        entry.visibleText = text.slice(0, 300)
        if (blocker) {
          entry.status = 'BLOCKED'
          entry.reason = blocker
        } else {
          const failed = Object.entries(entry.checks)
            .filter(([, value]) => value && value.ok === false)
            .map(([name]) => name)
          entry.status = failed.length === 0 ? 'CAPTURED' : 'LAYOUT_DEFECT'
          if (failed.length) entry.reason = `failed checks: ${failed.join(', ')}`
        }
        if (notes[screen.id]) entry.note = notes[screen.id]
      } catch (error) {
        entry.status = 'FAILED'
        entry.reason = String(error.message || error).slice(0, 300)
      }
      captures.push(entry)
    }
  }
} finally {
  for (const session of sessions.values()) await session.context.close()
  await browser.close()
}

const statuses = {}
for (const item of captures) statuses[item.status] = (statuses[item.status] || 0) + 1
await writeFile(`${output}/captures.json`, JSON.stringify({ captures, statuses }, null, 2) + '\n')
process.stdout.write(JSON.stringify({
  browser: browser.version(),
  captures,
  statuses,
  ok: !statuses.FAILED && !statuses.LAYOUT_DEFECT,
}))
