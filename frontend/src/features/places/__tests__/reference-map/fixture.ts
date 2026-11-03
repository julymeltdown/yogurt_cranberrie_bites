// TEST ONLY shell-scaffold preview until the independently owned root handoff.
// Actual default router/Auth/owner/resolver/Phaser/Places, original scenery and
// simulated admission metadata from the existing qualified default fixture.
import '../../../../app/__tests__/default-shell/illustrated-overview/fixture'
const style = document.createElement('style')
style.textContent = `
 .world-shell:has(.shell-panel){grid-template-rows:minmax(0,1fr)}
 .world-shell:has(.shell-panel) .mcr-chrome-bottom,.world-shell:has(.shell-panel) .mcr-chrome-header,.world-shell:has(.shell-panel) .shell-era-region{display:none}
 .world-shell:has(.shell-panel) .shell-panel{block-size:100%;width:100%;inset-inline:0}
 .shell-panel-body:has(.mcr-root-map){padding:6px;display:flex;overflow:hidden}
 .mcr-root-map{flex:1 1 auto;width:100%;min-height:0}
`
document.head.append(style)
