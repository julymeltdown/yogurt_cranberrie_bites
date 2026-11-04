#!/usr/bin/env python3
"""Actual offline C03/C05/C06 reviewed-v2 wire and denial regressions."""
import copy
import importlib.util
import json
from pathlib import Path
import unittest

from jsonschema import Draft202012Validator, ValidationError
ROOT=Path(__file__).resolve().parents[3]
def module(name,path):
 spec=importlib.util.spec_from_file_location(name,ROOT/path);m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m);return m
assets=module('motion_assets','scripts/mcr/validate-assets.py')
core=module('motion_core','scripts/mcr/validate-core.py')
realtime=module('motion_rt','scripts/mcr/validate-realtime.py')

ERRORS=(ValueError, ValidationError, assets.AssetError, realtime.WireError)

def load(p):return json.loads((ROOT/p).read_text())
class MotionWire(unittest.TestCase):
 @classmethod
 def setUpClass(cls):
  cls.manifest=load('backend/releases/selected-character-motion-v2/reviewed/manifest.json')
  cls.config=cls.manifest['presets'][0]['config']
  cls.document=core.generator.build_document()
  cls.checks=assets.validators()
  cls.rt=realtime.Contract()
 def check(self,value,name):core.validate(value,name,self.document)
 def test_reviewed_shared_31_fixture_outcomes(self):
  for kind,valid in [('valid',True),('invalid',False)]:
   for p in sorted((ROOT/'contracts/fixtures/assets/motion-v2/reviewed'/kind).glob('*.json')):
    value=json.loads(p.read_text())
    with self.subTest(path=p.name):
     schema=load('contracts/assets/v2/reviewed/avatar-config.schema.json')
     self.assertEqual(Draft202012Validator(schema).is_valid(value),valid)
     if valid:self.check({'revision':1,'config':value},'AvatarState')
     else:
      with self.assertRaises(ERRORS):self.check({'revision':1,'config':value},'AvatarState')
 def test_real_world_player_reuses_canonical_state_union(self):
  player={'userId':'11111111-1111-4111-8111-111111111111','displayName':'MCR','position':{'x':32,'y':60},'facing':'down','moving':False,'avatar':{'revision':1,'config':self.config}}
  self.rt.validate(player,'WorldPlayer')
  player['avatar']['config']=load('contracts/fixtures/assets/motion-v2/valid/preset-01.json')
  with self.assertRaises(ERRORS):self.rt.validate(player,'WorldPlayer')
 def test_frozen_candidate_never_becomes_public_by_boolean(self):
  candidate=load('backend/releases/selected-character-motion-v2/manifest.json')
  for flag in [False,True]:
   candidate['production_eligible']=flag
   with self.assertRaises(ERRORS):assets.validate_document(candidate,self.checks,'candidate')
  for p in sorted((ROOT/'contracts/fixtures/assets/motion-v2/valid').glob('*.json')):
   with self.assertRaises(ERRORS):self.check({'revision':1,'config':json.loads(p.read_text())},'AvatarState')
 def test_reviewed_hash_and_frozen_schema(self):
  m=self.manifest;self.assertEqual(m['content_hash'],assets.content_hash(m))
  assets.validate_document(m,self.checks,'reviewed')
  assets.validate_avatar(m,[self.config])
  for pointer,value in [('content_hash','sha256:'+'0'*64),('production_eligible',False),('review',{}),('manifest_version',1),('manifest_id','mcr_selected_motion_20261103')]:
   changed=copy.deepcopy(m);changed[pointer]=value
   with self.assertRaises(ERRORS):assets.validate_document(changed,self.checks,pointer)
 def test_resource_and_review_receipt_bytes_are_actual(self):
  import hashlib
  m=self.manifest
  review=load(m['review']['path'])
  pins=[m['review'],m['candidate_manifest'],review['additional_seam_report'],review['additional_seam_checker'],*review['inspected_resources'],*m['source']['resources'],*[s['resource'] for s in m['source']['originals']],m['atlas']]
  for pin in pins:
   raw=(ROOT/pin['path']).read_bytes();self.assertEqual(len(raw),pin['bytes']);self.assertEqual(hashlib.sha256(raw).hexdigest(),pin['sha256'])
  self.assertEqual(review['decision'],'VISUAL_ACCEPTED')
  self.assertFalse(review['originals_modified']);self.assertFalse(review['art_generated'])
 def test_old_schemas_remain_source_identical(self):
  import subprocess
  self.assertEqual(subprocess.check_output(['git','diff','980fc4d85020f297b36fee8195e1ca92079ff33f','--','contracts/assets/v1','frontend'],cwd=ROOT),b'')
  self.assertEqual(subprocess.check_output(['git','diff','ae41db9d','--','contracts/assets/v2/avatar-config.schema.json','contracts/assets/v2/avatar-manifest.schema.json','backend/releases/selected-character-motion-v2/manifest.json','contracts/fixtures/assets/motion-v2/valid','contracts/fixtures/assets/motion-v2/invalid'],cwd=ROOT),b'')
 def test_canonical_encoding_and_raw_duplicates(self):
  # Existing C06 hash rules: sorted objects, preserved arrays, ASCII Unicode
  # escaping including non-BMP surrogate pairs, compact JSON, no floats.
  self.assertEqual(assets.canonical_bytes({'z':'\U0001f3b8','a':[True,None,1]}),b'{"a":[true,null,1],"z":"\\ud83c\\udfb8"}')
  with self.assertRaises(ERRORS):assets.canonical_bytes({'v':1.5})
  with self.assertRaises(ERRORS):json.loads('{"x":1,"x":2}',object_pairs_hook=assets.reject_duplicates)

if __name__=='__main__':unittest.main(verbosity=2)
