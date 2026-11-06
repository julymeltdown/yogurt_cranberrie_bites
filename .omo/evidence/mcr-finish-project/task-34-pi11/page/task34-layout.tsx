// TEST ONLY: PI11 layout probe. Real WorldRooms/WorldNearby/WorldHelp components and
// controller-facing hooks; synthetic room/roster state; no product data or live service.
import { createRoot } from 'react-dom/client'
import { MemoryRouter } from 'react-router-dom'
import { WorldRooms } from '../ui/WorldRooms'
import { WorldNearby } from '../../world-nearby/WorldNearby'
import { WorldHelp } from '../../world-help/WorldHelp'
import { PanelHeader } from '../../../app/shell-chrome'
import '../../../shared/styles/base.css'
import '../../../app/shell.css'

const parameters = new URLSearchParams(location.search)
const screen = parameters.get('screen') ?? 'rooms', panel = (parameters.get('panel') ?? 'full') as 'full'
const titles: Record<string, string> = { rooms: 'Rooms', nearby: 'Nearby', help: 'Help' }
const self = { userId: '00000000-0000-4000-8000-000000000001', displayName: 'Fixture self', moving: false, self: true }
const others = ['Ghost', 'Jet', 'Kite', 'Lumen', 'Moss'].map((displayName, index) => ({
  userId: `00000000-0000-4000-8000-00000000001${index}`, displayName, moving: index % 2 === 0, self: false,
}))
let state: any = {
  available: true, phase: 'live',
  rooms: { phase: 'ready', error: null, data: { default_room_id: '00000000-0000-4000-8000-000000000010', items: [
    { room_id: '00000000-0000-4000-8000-000000000010', label: 'MAIN STAGE', capacity: 50, availability: 'AVAILABLE' },
    { room_id: '00000000-0000-4000-8000-000000000011', label: 'SECOND ROOM', capacity: 50, availability: 'AVAILABLE' },
  ] } },
  currentRoomId: '00000000-0000-4000-8000-000000000010', switch: { phase: 'idle', targetId: null, attempts: 0 },
  nearby: [self, ...others], partyNavigation: true,
}
const listeners = new Set<() => void>()
const notify = () => listeners.forEach(listener => listener())
const controller = {
  subscribe(listener: () => void) { listeners.add(listener); return () => { listeners.delete(listener) } },
  read() { return state }, start() {}, dispose() {},
  loadRooms: async () => { state = { ...state, rooms: { ...state.rooms, phase: 'ready' } }; notify() },
  switchReason(roomId: string) { return roomId === state.currentRoomId ? 'You are already in this room.' : null },
  async switchRoom(roomId: string) {
    state = { ...state, switch: { phase: 'switching', targetId: roomId, attempts: state.switch.attempts + 1 } }; notify()
    await new Promise(done => setTimeout(done, 30))
    state = { ...state, currentRoomId: roomId, switch: { phase: 'admitted', targetId: roomId, attempts: state.switch.attempts } }; notify()
  },
}
function Panel() {
  const content = screen === 'nearby' ? <WorldNearby controller={controller as never} /> : screen === 'help' ? <WorldHelp /> : <WorldRooms controller={controller as never} />
  return <div className="world-shell pi08-world-shell" data-mcr-shell="" data-panel-size={panel}>
    <div className="shell-stage" data-panel-size={panel}>
      <main id="shell-content" className="shell-panel" data-mcr-panel="" aria-labelledby="panel-title">
        <PanelHeader title={titles[screen]} size={panel} back={() => {}} toggleSize={() => {}} toggleContent={() => {}} />
        <div id="shell-panel-body" className="shell-panel-body">{content}</div>
      </main>
    </div>
  </div>
}
createRoot(document.getElementById('root')!).render(<MemoryRouter initialEntries={['/world']}><Panel /></MemoryRouter>)
Object.assign(window, { __TASK34_LAYOUT__: { state } })
