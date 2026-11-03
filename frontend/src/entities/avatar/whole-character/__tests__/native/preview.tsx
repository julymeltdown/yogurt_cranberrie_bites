import { useState } from 'react'
import { createRoot } from 'react-dom/client'
import manifest from '../../../../../../../contracts/fixtures/assets/selected-static/avatar-manifest.json'
import { createSessionController } from '../../../../../features/auth/model/session'
import { createWholeCharacterRuntime, WholeCharacterView, type WholeCharacterConfig } from '../../index'
import './preview.css'
const eventId = '00000000-0000-4000-8000-000000000001', userId = '00000000-0000-4000-8000-000000000002'
const auth = createSessionController({ eventId, cookiePolicy: { available: true, onInvalidate: () => () => {}, run: (_kind, work) => work(), dispose() {} } })
auth.session.replace(0, { userId, eventId, accessToken: 'fixture-memory-token' })
const requests: string[] = []
const runtime = createWholeCharacterRuntime({ privacy: auth.privacy, manifest, pin: manifest, origin: location.origin,
  fetch: (input, init) => { requests.push(String(input)); return fetch(input, init) } })
const choices = await runtime.choices(new AbortController().signal)
function Preview() {
  const [era, setEra] = useState('REVENGE'), [visible, setVisible] = useState(true), [selected, setSelected] = useState<WholeCharacterConfig>(choices[0]!.config), [detail, setDetail] = useState(false)
  return <main data-era={era}><h1>Selected whole characters</h1><p>Actual reviewed six-source atlas · front idle only. Canonical identity fixtures; no live save, auth or world journey.</p>
    <nav aria-label="Backdrop">{['REVENGE', 'PARADE', 'DANGER'].map(e => <button key={e} onClick={() => setEra(e)}>{e}</button>)}</nav>
    <button onClick={() => setVisible(v => !v)}>{visible ? 'Unmount views' : 'Remount views'}</button>
    {visible && <><div className="choices">{choices.map(c => <button key={c.id} aria-label={`Preview ${c.id}`} onClick={() => setSelected(c.config)}><WholeCharacterView runtime={runtime} config={c.config} label={c.label} /><span>{c.id.slice(-2)}</span></button>)}</div>
      <h2>Same identity · main preview</h2><WholeCharacterView runtime={runtime} config={selected} label="Selected character" size={128} detail={detail} />
      <button onClick={() => setDetail(v => !v)}>{detail ? 'Use compact atlas' : 'Request detail atlas'}</button>
      <p>Current preset: {selected.preset_id}</p></>}
    <button onClick={() => auth.privacy.changePolicy(auth.privacy.capture(), 'block')}>Revoke scope</button>
  </main>
}
const root = createRoot(document.getElementById('root')!);root.render(<Preview />)
window.wholeRuntime = { state: () => ({ requests, runtime: runtime.diagnostics() }), runtime, dispose: () => { root.unmount(); runtime.dispose(); auth.dispose() } }
declare global { interface Window { wholeRuntime: { state(): unknown; runtime: typeof runtime; dispose(): void } } }
