import { validateMotionSelection, type MotionArt, type MotionSelection } from '../../../../entities/avatar/runtime/motion-adoption/art'
import type { MotionAuthority } from '../../../../entities/avatar/runtime/motion-adoption/runtime'

/** Fixture-only record, deliberately incompatible with public AvatarConfig. Never an inventory/version claim. */
export interface TestMotionConfig { readonly testOnlyContract: 'mcr-motion-fixture-v1'; readonly selection: MotionSelection }
export const testMotionPin = Object.freeze({ manifest_id: 'test_only_selected_motion', manifest_version: 1,
  content_hash: 'sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa' })
export function testMotionAuthority(art: MotionArt): MotionAuthority<TestMotionConfig> {
  function decode(value: unknown): TestMotionConfig {
    if (!value || typeof value !== 'object' || Array.isArray(value) || Object.keys(value).sort().join(',') !== 'selection,testOnlyContract'
      || !('testOnlyContract' in value) || value.testOnlyContract !== 'mcr-motion-fixture-v1' || !('selection' in value)) throw Error('Not TEST_ONLY motion config')
    return Object.freeze({ testOnlyContract: 'mcr-motion-fixture-v1', selection: validateMotionSelection(value.selection, art) })
  }
  return Object.freeze({ qualification: 'TEST_ONLY', manifestId: testMotionPin.manifest_id, manifestVersion: 1, manifestHash: testMotionPin.content_hash,
    decodeConfig: decode, selection: (config: TestMotionConfig) => decode(config).selection,
    presets: () => [...new Set(art.descriptor.presets.frames.map(frame => frame.preset))].map((preset, i) => ({ id: preset, label: `Character ${i + 1}`,
      config: decode({ testOnlyContract: 'mcr-motion-fixture-v1', selection: { mode: 'preset', preset } }) })),
    replace: (_config: TestMotionConfig, selection: MotionSelection) => decode({ testOnlyContract: 'mcr-motion-fixture-v1', selection }),
  })
}
