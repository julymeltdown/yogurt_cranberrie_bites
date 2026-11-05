// TEST_ONLY qualified feature fixture. This is not default-App physical authority.
import { createRoot } from 'react-dom/client'
import { GiftPickup } from '../../../gift-pickup/ui/GiftPickup'
import { harness, gift, reservation, meeting, now } from '../../../gift-pickup/__tests__/harness'
import '../../../../shared/styles/base.css'
if (!import.meta.env.DEV) throw Error('Development-only PI04 controlled state fixture')
let issued = false
const h = harness({ handler: request => {
  const path = new URL(request.url).pathname
  const r = reservation({ revision: issued ? 2 : 1 })
  if (path.endsWith('/pickup-code')) {
    issued = true
    return Response.json({ reservation_id: r.reservation_id, confirmation_code: '654321', reservation_revision: 2, expires_at: new Date(now + 120000).toISOString() })
  }
  return Response.json(path.endsWith('/pickup') ? { reservation: r, meeting: meeting() }
    : path.endsWith(gift().giveaway_id) ? gift() : r)
} })
const root=createRoot(document.getElementById('root')!)
root.render(<div style={{ maxWidth: 390, padding: 6 }}><GiftPickup controller={h.controller} /></div>)
Object.assign(window, { __PI04_STATE_FIXTURE__: { classification: 'TEST_ONLY qualified feature fixture; not default-App physical authority', dispose() { root.unmount(); h.dispose() } } })
