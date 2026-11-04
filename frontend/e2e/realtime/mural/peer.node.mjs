import {test} from 'node:test'
import assert from 'node:assert/strict'
import {muralPeer} from './peer.mjs'
const event='00000000-0000-4000-8000-000000000002',actor='00000000-0000-4000-8000-000000000001'
test('synthetic restore/resume and peer restart retain monotonic revisions without pretending a live restart',()=>{
 const p=muralPeer(event,actor);const operations=p.paint(2);assert.deepEqual(operations.map(o=>o.revision),[11,12]);assert.throws(()=>p.restore());p.pause();const s=p.restore();assert.equal(s.revision,14);assert.equal(s.state,'PAUSED');assert.ok(s.pixels.every(n=>n===2));p.resume();assert.equal(p.snapshot().revision,15);assert.equal(p.snapshot().state,'EDITABLE');assert.equal(p.restart(),2);assert.equal(p.snapshot().revision,15);p.dispose()
})
test('fault peer refuses duplicate held reads rather than accumulating an unbounded pending list',()=>{
 const p=muralPeer(event,actor);p.hold('a','snapshot');assert.throws(()=>p.hold('a','snapshot'));assert.equal(p.stats().holds,1);p.dispose();assert.equal(p.stats().holds,0)
})
