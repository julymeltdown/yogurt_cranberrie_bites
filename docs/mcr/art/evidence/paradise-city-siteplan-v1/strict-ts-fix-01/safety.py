"""Shared read-only meters, instantiated independently by both controllers."""
import hashlib
import json
import os
from pathlib import Path
import shutil
import time
from inventory import Inventory,process_snapshot

MIB=1024**2
HERE=Path(__file__).resolve().parent
ROOT=HERE.parents[5]
TEST=ROOT/'frontend/src/features/paradise-city-map/art-source/qualifier.test.ts'
DOC=ROOT/'docs/mcr/art/paradise-city-siteplan-v1.md'
RECORD=Path('/Users/lhs/Desktop/mcr-evidence/backend-staffing-01a1a21d/owner-resource-receipts/01a1a231-0dc4-7b50-92bd-c0f0f56bd970.json')
NODE='/Users/lhs/Desktop/mcr-tools/node-v24.22.2-darwin-arm64/bin/node'
PYTHON='/Library/Frameworks/Python.framework/Versions/3.14/bin/python3'
HISTORICAL_NEW=221435639
REPAIR_CAP=64*MIB
PREP_RESERVED=28423407+4479502+MIB  # Both actual failures plus bounded closing-seal upper once.
def atomic(path,value):
 pending=path.with_suffix(path.suffix+'.pending');pending.write_text(json.dumps(value,indent=2)+'\n');pending.replace(path)
def pin(path):
 raw=path.read_bytes();return dict(path=str(path),bytes=len(raw),sha256=hashlib.sha256(raw).hexdigest())
def own_size():return sum(p.stat().st_size for p in HERE.rglob('*') if p.is_file())
def readonly_pins():
 pins=[]
 previous=HERE.parent
 for entry in json.loads((previous/'publication-postimages.json').read_text())['files']:
  p=ROOT/entry['path']
  if p not in (TEST,DOC):
   assert hashlib.sha256(p.read_bytes()).hexdigest()==entry['sha256'];pins.append(pin(p))
 pins.append(pin(previous/'publication-postimages.json'))
 for entry in json.loads((previous/'preserved-custody.json').read_text())['files']:
  p=Path(entry['path']);p=p if p.is_absolute() else ROOT/p
  raw=p.read_bytes();assert len(raw)==entry['bytes'] and hashlib.sha256(raw).hexdigest()==entry['sha256'];pins.append(pin(p))
 return pins
class Meter:
 def __init__(self,config):
  self.config=config;self.temp=Path(config['temp']);self.inventory=Inventory(config['roots'],config['priorWhole'])
  self.peak=dict(whole=self.inventory.carry,repairNew=PREP_RESERVED,rss=0,processes=0,tmp=0)
  self.minimum=shutil.disk_usage(ROOT).free;self.samples=0;self.last=None
 def check(self,work_active=True):
  c=self.config;assert time.time()<(c['work'] if work_active else c['hard']),'Fixed original deadline'
  free=shutil.disk_usage(ROOT).free;self.minimum=min(self.minimum,free)
  whole,new=self.inventory.sample();repair=PREP_RESERVED+max(0,new-34*MIB)
  tmp=sum(p.stat().st_size for p in self.temp.rglob('*') if p.is_file())
  assert free>=8*1024**3 and whole<=48*1024**3 and repair<=REPAIR_CAP,'Storage/repair growth limit'
  assert tmp<=4*MIB and own_size()<=4*MIB,'Bounded private temporary/evidence output'
  state=process_snapshot(c['supervisor'],self.temp/'sampler.lock')
  assert state['processes']<=4 and state['rss']<=512*MIB,'Aggregate process/RSS ceiling'
  other=max(5*1024**3,state['globalCensus']['otherRemainingUpper'])
  assert free-other-max(0,REPAIR_CAP-repair)>=9*1024**3,'Aggregate future margin'
  for k,v in dict(whole=whole,repairNew=repair,tmp=tmp,rss=state['rss'],processes=state['processes']).items():self.peak[k]=max(self.peak[k],v)
  self.samples+=1;self.last=state;return state
