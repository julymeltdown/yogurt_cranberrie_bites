import { useState } from 'react'
import { createRoot } from 'react-dom/client'
import { EventOperations } from '../../ui/EventOperations'
import { PlacesOperations } from '../../../places/PlacesOperations'
import { AuditOperations } from '../../../audit/AuditOperations'
import { placeProducerFixture } from './producer-fixture'
import '../../../../../shared/styles/base.css'

const f = placeProducerFixture(new URLSearchParams(location.search).get('producer') === 'unavailable')
const root = createRoot(document.getElementById('root')!)
function Preview() {
  const [surface, setSurface] = useState('places')
  return <><p>SYNTHETIC FE38 producer-failure fixture. No real operator or venue.</p>
    <nav aria-label="Fixture surfaces">{['places', 'event', 'audit'].map(name => <button key={name} type="button" onClick={() => setSurface(name)}>{name}</button>)}</nav>
    {surface === 'places' ? <PlacesOperations controller={f.controller} /> : surface === 'audit' ? <AuditOperations controller={f.controller} /> : <EventOperations controller={f.controller} />}</>
}
root.render(<Preview />)
Object.assign(window, { __FE38_CURRENT__: f })
declare global { interface Window { __FE38_CURRENT__: typeof f } }
