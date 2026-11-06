// TEST ONLY: actual HUD components, tokens and shell CSS. No app/Auth/World/Phaser claim.
import { createRoot } from 'react-dom/client'
import { MemoryRouter } from 'react-router-dom'
import { PortsProvider } from '../../../../../app/access'
import { NavigationMemory } from '../../../../../app/navigation'
import type { ShellPorts } from '../../../../../app/ports'
import { BottomNavigation, EraControls } from '../../../../../app/shell-chrome'
import { WorldHudHeaderSlot, WorldHudLayout, WorldHudViewport } from '../../index'
import '../../../../../shared/styles/base.css'
import '../../../../../app/shell.css'

const eraState = { phase: 'ready' as const, selected: 'REVENGE' as const, pending: null, message: '', saved: false }
const era = {
  read: () => eraState,
  subscribe: () => () => {},
  select: async () => {},
}
const scope = { userId: '00000000-0000-4000-8000-000000000001', eventId: '00000000-0000-4000-8000-000000000201', generation: 1 }
const view = { scope, phase: 'live', visible: true, baseline: { eventId: scope.eventId, selfUserId: scope.userId },
  entities: [{ self: true, player: { userId: scope.userId, displayName: 'Synthetic self', moving: false } }] }
const binding = { release: { scope, privacyScope: { ...scope, policyGeneration: 1, gate: 'open' }, signal: new AbortController().signal },
  replication: { read: () => view, subscribe: () => () => {}, switchRoom: () => {} } }
const ports = { auth: { session: { isCurrent: () => true }, privacy: { isCurrent: () => true } },
  worldAccess: { read: () => binding, subscribe: () => () => {} } } as unknown as ShellPorts

createRoot(document.getElementById('root')!).render(
  <PortsProvider ports={ports}>
    <MemoryRouter initialEntries={['/world']}>
      <NavigationMemory>
        <WorldHudLayout active toolsOpen={false} onToolsChange={() => {}}>
          <div className="world-shell pi08-world-shell" data-mcr-shell="" data-panel-size="closed">
            <WorldHudHeaderSlot><span>Current authorized identity</span></WorldHudHeaderSlot>
            <div className="shell-era-region"><EraControls controller={era} /></div>
            <div className="shell-stage" data-panel-size="closed">
              <WorldHudViewport><div style={{ background: '#d9d7d1', minHeight: 0 }} /></WorldHudViewport>
            </div>
            <BottomNavigation />
          </div>
        </WorldHudLayout>
      </NavigationMemory>
    </MemoryRouter>
  </PortsProvider>,
)
