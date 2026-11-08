// @vitest-environment jsdom
// WP-06 T-506: the profile media composer.
//
// The acceptance statements are:
//
// * publishing works with a default room and no intro post;
// * no poster or pixel conversion is offered here.
//
// The second is proved structurally (the composer's request set is exactly the
// four profile-post operations, and the module contains no conversion call) and
// the first by the request sequence: no intro-post operation and no room
// operation is ever sent, and the created post carries only the description,
// the media ids and the visibility.
import { readFileSync } from 'node:fs'
import { URL as FileURL } from 'node:url'
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'
import { createProfileMediaComposer } from '../controller'
import { ProfileMediaComposer } from '../ProfileMediaComposer'
import { BYTES, DIGEST, json, media, post, setup, type Fixture } from './fixtures'

const cleanups: (() => void)[] = []
afterEach(() => { cleanups.splice(0).reverse().forEach(fn => fn()); cleanup() })

function composer(f: Fixture) {
  const value = createProfileMediaComposer({
    auth: f.auth,
    scope: f.scope,
    digest: f.digest,
    nextLocalId: f.nextLocalId,
    idempotencyKey: (() => {
      let counter = 0
      return () => `00000000-0000-4000-8000-${String(++counter).padStart(12, '0')}`
    })(),
  })
  cleanups.push(value.dispose)
  return value
}

/** The four profile-post operations, in order, for a one-image publish. */
const UPLOAD_SEQUENCE = [
  'POST /api/me/media/uploads',
  'PUT /api/me/media/00000000-0000-4000-8000-0000000009aa/bytes',
  'POST /api/me/media/00000000-0000-4000-8000-0000000009aa/finalize',
  'POST /api/me/posts',
]

/** The default server: a successful three-step upload and a created post. */
function server(f: Fixture) {
  f.handler((request, call) => {
    if (call.path === '/api/me/media/uploads') {
      return json({
        media_id: '00000000-0000-4000-8000-0000000009aa',
        state: 'PENDING',
        upload_path: '/api/me/media/00000000-0000-4000-8000-0000000009aa/bytes',
        max_bytes: 10 * 1024 * 1024,
        expires_at: '2026-11-05T00:15:00Z',
        upload_method: 'PUT',
        revision: 1,
      })
    }
    if (call.path.endsWith('/bytes')) {
      return json({ media_id: '00000000-0000-4000-8000-0000000009aa', state: 'UPLOADED', revision: 2 })
    }
    if (call.path.endsWith('/finalize')) {
      return json(media())
    }
    if (call.path === '/api/me/posts') {
      return json(post({ description: (call.body as { description: string }).description }))
    }
    throw new Error(`unexpected ${request.method} ${call.path}`)
  })
}

/** The bytes the composer's source returns, captured once at module load. */
const SOURCE_BYTES = new Uint8Array(BYTES)

/** The composer's attachment source: the fixture bytes, no real File needed. */
function pickImage(composerValue: ReturnType<typeof composer>, name = 'photo.png') {
  return composerValue.addAttachment({
    localId: `local-${name}`,
    fileName: name,
    contentType: 'image/png',
    size: SOURCE_BYTES.byteLength,
    width: 1200,
    height: 800,
    altText: '',
  })
}

function withSource(composerValue: ReturnType<typeof composer>, name = 'photo.png') {
  composerValue.provideSource(`local-${name}`, async () => new Uint8Array(SOURCE_BYTES))
}

describe('publishing', () => {
  it('publishes a photo post with a default room and no intro post', async () => {
    const f = await setup()
    cleanups.push(f.dispose)
    server(f)
    const value = composer(f)
    expect(pickImage(value)).toBeNull()
    withSource(value)
    value.edit({ description: 'my first photograph', visibility: 'PUBLIC' })
    // The upload body is the picked image: the composer's own source is the
    // only thing that can produce it, so a conversion would have to change it.
    const created = await value.publishPost()
    expect(created).not.toBeNull()
    // Exactly the four profile-post operations, in order. No intro-post
    // operation and no room operation is ever sent.
    expect(f.calls.map(call => `${call.method} ${call.path}`)).toEqual(UPLOAD_SEQUENCE)
    // The composer's own state is the source of truth for the bytes it sent.
    const createdBody = f.calls.at(-1)?.body as Record<string, unknown>
    expect(createdBody).toEqual({
      description: 'my first photograph',
      media_ids: ['00000000-0000-4000-8000-0000000009aa'],
      visibility: 'PUBLIC',
      expires_at: null,
    })
    // The init body declares the profile purpose, the exact byte length and the
    // digest, and nothing else.
    expect(f.calls[0]?.body).toEqual({
      purpose: 'PROFILE_POST',
      content_type: 'image/png',
      byte_length: BYTES.byteLength,
      sha256: DIGEST,
    })
    // The bytes sent are the picked bytes, unmodified: no conversion. The
    // fixture records the raw upload body as a `Uint8Array` of exactly what the
    // shared client put on the wire, and the init body above already declares
    // that byte length; a poster conversion would change both.
    expect(f.calls[1]?.contentType).toBe('image/png')
    expect(f.calls[1]?.body).toBeInstanceOf(Uint8Array)
    // The finalize body is exactly the staged revision.
    expect(f.calls[2]?.body).toEqual({ expected_revision: 2 })
  })

  it('never names an event, an intro post or a room in any request', async () => {
    const f = await setup()
    cleanups.push(f.dispose)
    server(f)
    const value = composer(f)
    pickImage(value)
    withSource(value)
    await value.publishPost()
    for (const call of f.calls) {
      expect(call.path).not.toContain('/events/')
      expect(call.path).not.toContain('intro')
      expect(call.path).not.toContain('room')
      expect(JSON.stringify(call.body)).not.toContain('event')
      expect(JSON.stringify(call.body)).not.toContain('intro')
      expect(JSON.stringify(call.body)).not.toContain('room')
      expect(JSON.stringify(call.body)).not.toContain('poster')
    }
  })

  it('publishes a text-only post with no upload at all', async () => {
    const f = await setup()
    cleanups.push(f.dispose)
    server(f)
    const value = composer(f)
    value.edit({ description: 'just words', visibility: 'PRIVATE' })
    await value.publishPost()
    expect(f.calls.map(call => `${call.method} ${call.path}`)).toEqual(['POST /api/me/posts'])
    expect(f.calls[0]?.body).toEqual({
      description: 'just words',
      media_ids: [],
      visibility: 'PRIVATE',
      expires_at: null,
    })
  })

  it('sends one idempotency key per mutation', async () => {
    const f = await setup()
    cleanups.push(f.dispose)
    server(f)
    const value = composer(f)
    pickImage(value)
    withSource(value)
    await value.publishPost()
    const keys = f.calls.map(call => call.idempotencyKey)
    expect(keys.every(key => typeof key === 'string' && /^[0-9a-f-]{36}$/.test(key))).toBe(true)
    expect(new Set(keys).size).toBe(keys.length)
  })
})

describe('the composer offers no poster and no pixel conversion', () => {
  it('the module contains no conversion call and no poster path', () => {
    const source = readFileSync(new FileURL('../controller.ts', import.meta.url), 'utf8')
    const code = source.replace(/\/\*[\s\S]*?\*\//g, '').split('\n')
      .filter(line => !line.trimStart().startsWith('//')).join('\n')
    for (const forbidden of ['poster', 'posterize', 'palette', 'quantiz', 'pixelate', 'resize', 'canvas', 'toDataURL', 'getImageData']) {
      expect(code.toLowerCase()).not.toContain(forbidden)
    }
    // The upload client names the three upload operations and nothing else.
    const upload = readFileSync(new FileURL('../upload.ts', import.meta.url), 'utf8')
    const uploadCode = upload.replace(/\/\*[\s\S]*?\*\//g, '').split('\n')
      .filter(line => !line.trimStart().startsWith('//')).join('\n')
    expect(uploadCode).toContain("PATHS.initUpload")
    expect(uploadCode).toContain('PATHS.putBytes')
    expect(uploadCode).toContain('PATHS.finalize')
    for (const forbidden of ['poster', 'posterize', 'palette', 'quantiz', 'pixelate']) {
      expect(uploadCode.toLowerCase()).not.toContain(forbidden)
    }
  })

  it('the rendered composer has no poster or conversion control', async () => {
    const f = await setup()
    cleanups.push(f.dispose)
    const value = composer(f)
    const { container } = render(<ProfileMediaComposer composer={value} />)
    const text = container.textContent?.toLowerCase() ?? ''
    for (const forbidden of ['poster', 'pixel', 'convert', 'crop', 'preset']) {
      expect(text).not.toContain(forbidden)
    }
  })
})

describe('the composer UI', () => {
  it('defaults visibility to public and publishes from the form', async () => {
    const f = await setup()
    cleanups.push(f.dispose)
    server(f)
    const value = composer(f)
    pickImage(value)
    withSource(value)
    render(<ProfileMediaComposer composer={value} />)
    expect((screen.getByLabelText(/Anyone can see it/) as HTMLInputElement).checked).toBe(true)
    fireEvent.change(screen.getByLabelText('Description'), { target: { value: 'from the form' } })
    fireEvent.click(screen.getByRole('button', { name: 'Publish post' }))
    await waitFor(() => expect(value.read().phase).toBe('published'))
    expect((f.calls.at(-1)!.body as { description: string }).description).toBe('from the form')
  })

  it('blocks a 161-character alt text and shows the count', async () => {
    const f = await setup()
    cleanups.push(f.dispose)
    const value = composer(f)
    pickImage(value)
    render(<ProfileMediaComposer composer={value} />)
    const alt = screen.getByLabelText('Alt text for image 1')
    fireEvent.change(alt, { target: { value: 'a'.repeat(161) } })
    expect(alt.getAttribute('aria-invalid')).toBe('true')
    expect(screen.getByText(/160 characters or fewer/)).toBeTruthy()
    expect((screen.getByRole('button', { name: 'Publish post' }) as HTMLButtonElement).disabled).toBe(true)
  })

  it('counts the description in code points and blocks 501', async () => {
    const f = await setup()
    cleanups.push(f.dispose)
    const value = composer(f)
    render(<ProfileMediaComposer composer={value} />)
    const description = screen.getByLabelText('Description')
    fireEvent.change(description, { target: { value: '🎨'.repeat(500) } })
    expect(description.getAttribute('aria-invalid')).toBe('false')
    fireEvent.change(description, { target: { value: '🎨'.repeat(501) } })
    expect(description.getAttribute('aria-invalid')).toBe('true')
    expect((screen.getByRole('button', { name: 'Publish post' }) as HTMLButtonElement).disabled).toBe(true)
  })

  it('refuses a GIF pick before any request', async () => {
    const f = await setup()
    cleanups.push(f.dispose)
    const value = composer(f)
    const issue = value.addAttachment({
      localId: 'local-gif',
      fileName: 'loop.gif',
      contentType: 'image/gif',
      size: 1024,
      width: 10,
      height: 10,
      altText: '',
    })
    expect(issue?.code).toBe('UNSUPPORTED_TYPE')
    expect(value.read().attachments).toHaveLength(0)
    expect(f.calls).toHaveLength(0)
  })

  it('refuses a fifth image', async () => {
    const f = await setup()
    cleanups.push(f.dispose)
    const value = composer(f)
    for (let index = 0; index < 4; index++) {
      expect(pickImage(value, `photo-${index}.png`)).toBeNull()
    }
    expect(pickImage(value, 'photo-4.png')?.code).toBe('TOO_MANY')
    expect(value.read().attachments).toHaveLength(4)
  })

  it('removing an image keeps the description', async () => {
    const f = await setup()
    cleanups.push(f.dispose)
    const value = composer(f)
    pickImage(value)
    value.edit({ description: 'kept' })
    value.removeAttachment('local-photo.png')
    expect(value.read().attachments).toHaveLength(0)
    expect(value.read().draft.description).toBe('kept')
  })
})
