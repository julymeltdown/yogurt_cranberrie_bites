"""Bind unchanged product positives to current bytes; retain overall failed runs."""
import ast
import hashlib
import json
from pathlib import Path
import sys

HERE=Path(__file__).resolve().parent;ROOT=HERE.parents[4]
def sha(raw):return hashlib.sha256(raw).hexdigest()
def pinned(name,expected):
 raw=(HERE/name).read_bytes();assert sha(raw)==expected
 return [json.loads(line) for line in raw.splitlines()]
first=pinned('PARADISECITY-ART-QUALIFY01-0125.jsonl','b27a40066784f6b187767ff55d5699eeaa8bcab5640187af3e93efc0a9a8440e')
third=pinned('PARADISECITY-ART-QUALIFY03-0133.jsonl','f7c999bd07ed4389133e1fe89a40baa846c51a72144f9b3ef2f2f0f6026bf4aa')
fourth=pinned('PARADISECITY-ART-QUALIFY04-0152.jsonl','57e0565d1b7251adc52adb52040812a2410e19bafedd7ee04b74b28d7cdcdc0f')
assert first[-1]['status']=='FAIL' and third[-1]['status']=='FAIL'
attempt=next(row for row in first if row['event']=='ATTEMPT')
names=('package.py','qualify.ts','qualifier.test.ts','source-anchors.json','compare.py')
source=[]
for pin in attempt['sourcePins']:
 if Path(pin['path']).name in names:
  assert sha((ROOT/pin['path']).read_bytes())==pin['sha256'];source.append(pin)
assert len(source)==5
children={row['name']:row for row in first if row['event']=='CHILD'}
for name in ('package-source','verify-source-package','source-qualification-and-hostile-tests','measured-reference-asset-comparison'):
 assert children[name]['exit']==0
tests=children['source-qualification-and-hostile-tests']['stdout']
assert 'tests 12' in tests and 'pass 12' in tests and 'fail 0' in tests
ready=next(row for row in first if row['event']=='READY_FOR_READONLY_INSPECTION')
pack=ROOT/'frontend/public/venue/paradise-city/v1'
assert sorted(p.name for p in pack.iterdir())==sorted(pin['file'] for pin in ready['outputs'])
for pin in ready['outputs']:
 raw=(pack/pin['file']).read_bytes();assert len(raw)==pin['bytes'] and sha(raw)==pin['sha256']
comparison=json.loads(children['measured-reference-asset-comparison']['stdout'])
assert comparison['rgbDifferencePixels']==0 and comparison['comparison']=='UNMASKED_RGB_EXACT' and comparison['sourceROI']==[0,0,1024,1280] and comparison['annotationIsSeparateTransparentRGBA']
for pin in comparison['outputs']:
 raw=(ROOT/pin['path']).read_bytes();assert len(raw)==pin['bytes'] and sha(raw)==pin['sha256']
guard=next(row for row in third if row['event']=='CHILD' and row['name']=='census-and-actual-start-clock-regressions')
assert guard['exit']==0 and ast.literal_eval(guard['stdout'])['guardRegressionCases']==13
guard_pin=next(pin for row in third if row['event']=='ATTEMPT' for pin in row['sourcePins'] if pin['path'].endswith('/guard_regressions.py'))
assert sha((ROOT/guard_pin['path']).read_bytes())==guard_pin['sha256']
assert fourth[-1]['status']=='FAIL'
changed=next(row for row in fourth if row['event']=='CHILD' and row['name']=='changed-controller-identity-regressions')
assert changed['exit']==0 and ast.literal_eval(changed['stdout'])['changedIdentityCases']==8
for pin in next(row for row in fourth if row['event']=='ATTEMPT')['sourcePins']:
 if Path(pin['path']).name in ('identity_regressions.py','inventory.py'):
  assert sha((ROOT/pin['path']).read_bytes())==pin['sha256']
result=dict(status='PASS',offer=sys.argv[1],qualifiedSourceCommit=sys.argv[2],scope='CURRENT_CLOSED_PACK_AND_EXACT_UNCHANGED_PRODUCT_SOURCE_BINDING',reusedProductCases=12,reusedPreviousGuardCases=13,reusedChangedIdentityCases=8,sourcePins=source,outputs=ready['outputs'],comparison=comparison,priorOverallFailuresPreserved=True,notAppCapture=True,notGeographicCalibration=True)
(HERE/(sys.argv[1]+'-positive-binding.json')).write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps(dict(status='PASS',reusedProductCases=12,reusedGuardCases=13,reusedChangedIdentityCases=8,currentPackByteIdentical=True,priorOverallFailuresPreserved=True)))
