import { createRoot } from 'react-dom/client'
import { flushSync } from 'react-dom'
import { setup } from '../reviewed-production/fixtures'
import { createMotionRuntime, type MotionPart } from '../../../../entities/avatar/runtime/motion-adoption/runtime'
import { MotionOption } from '../../motion-adoption/MotionOption'

async function start() {
  // Native rendering with a minted authority from deterministic protected bootstrap.
  // This does not claim a real backend or provider journey.
  const owner = await setup(), authority = await owner.create()
  const runtime = createMotionRuntime({ authority, scope: owner.scope, privacy: owner.auth.privacy, origin: location.origin })
  const catalog = await runtime.catalog(owner.signal.signal), draft = catalog.presets[0]!.config, selection = authority.selection(draft)
  const root = createRoot(document.getElementById('cards')!)
  function show(kind: MotionPart['kind']) {
    const items = catalog.items.filter(item => item.kind === kind).slice(0, 4)
    flushSync(() => root.render(<>{items.map((item, i) => <MotionOption key={`${item.kind}:${item.key}`} runtime={runtime}
      draft={draft} selection={selection} part={{ kind: item.kind, key: item.key }} label={item.key} pressed={i === 0} disabled={false} onChoose={() => {}} />)}</>))
  }
  return { runtime, catalog, show,
    clear() { flushSync(() => root.render(null)) },
    revoke() { owner.auth.privacy.changePolicy(owner.scope, 'block') },
    dispose() { flushSync(() => root.unmount()); runtime.dispose(); owner.dispose() } }
}
declare global { interface Window { partPreviewFixture: Awaited<ReturnType<typeof start>> } }
window.partPreviewFixture = await start()
