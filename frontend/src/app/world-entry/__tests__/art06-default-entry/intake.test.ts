import {readFile} from 'node:fs/promises'
import {expect,it} from 'vitest'
import {currentComposition} from '../../../../world/reference-scene/map4-service-v2/current'
import {map4Parade,map4ThemeHashes} from '../../../../world/reference-scene/map4-service-v1/identity'

it('preserves all thirteen frozen ART05 authority/layout fields while rejecting its stale presentation digest',async()=>{
 const previous=JSON.parse(await readFile('../docs/mcr/evidence/map-art06-default-app/frozen-art05-invariants.json','utf8'))
 const actual=JSON.parse(await readFile('public/world/map-reference-composition-v1/layout.json','utf8'))
 expect(Object.keys(previous)).toHaveLength(13)
 for(const key of Object.keys(previous))expect(actual[key],key).toEqual(previous[key])
 const model=currentComposition();if(!model)throw Error('Fresh ART06 registration required')
 const identity={...map4Parade,themeId:'PARADE',themeHash:map4ThemeHashes.PARADE,presentationVersion:model.registration.version,presentationHash:model.registration.presentationHash}
 expect(model.matches(identity)).toBe(true);expect(model.annotations(identity)).toHaveLength(4)
 expect(model.matches({...identity,presentationHash:'sha256:f7a89dc6d176f2414f845139de39607ba75830a54e5e623f2de2cd6fc76bed17'})).toBe(false)
 expect(model.annotations({...identity,presentationHash:'sha256:f7a89dc6d176f2414f845139de39607ba75830a54e5e623f2de2cd6fc76bed17'})).toEqual([])
})
