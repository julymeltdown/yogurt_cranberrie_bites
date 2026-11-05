"""TEST_ONLY isolated resource-identity regressions; no real receipt mutations."""
import hashlib,json,os,tempfile,unittest
from pathlib import Path
from unittest.mock import patch
import membership as m

class MembershipCases(unittest.TestCase):
 def setUp(self):
  self.tmp=tempfile.TemporaryDirectory(dir=os.environ['MCR_PI04_TEMP']);self.base=Path(self.tmp.name);self.directory=self.base/'owners';self.directory.mkdir()
  self.policy={'global':4,'source':3,'browser':2,'warmRustTarget':1,'sumPIDCaps':96,'sumRSSBytes':18*1024**3,'ordinaryFuturePositiveUpperBytes':int(2.5*1024**3)}
  (self.base/'resource-receipt-interface.json').write_text(json.dumps({'limits':self.policy}));self.legacy([])
  self.patches=[patch.object(m,'BASE',self.base),patch.object(m,'DIRECTORY',self.directory),patch.object(m,'processes',return_value={})]
  self.handles=[p.start() for p in self.patches]
 def tearDown(self):
  for p in reversed(self.patches):p.stop()
  self.tmp.cleanup()
 def legacy(self,entries):(self.base/'resource-membership-current.json').write_text(json.dumps({'entries':entries}))
 def record(self,**values):
  r={'ownerThreadId':'peer','offerId':'new','state':'ACCEPTED','acceptedAtEpoch':2,'kind':'source','processCap':8,'rssCapBytes':1024**3,'remainingNewUpperBytes':1024,'controller':None,'guardControllers':[]};r.update(values);(self.directory/'peer.json').write_text(json.dumps(r));return r
 def test_unknown_receipt_state_is_rejected_with_owner_and_state(self):
  self.record(state='UNSUPPORTED_TEST_ONLY')
  with self.assertRaisesRegex(AssertionError,'unsupported resource receipt state:.*peer.*UNSUPPORTED_TEST_ONLY'):m.sample()
 def test_completed_state_observation_cannot_identify_new_payload(self):
  (self.directory/'ordinary.json').write_text(json.dumps({'ordinaryObservation':{'state':'COMPLETED','pid':99,'birthAndComm':'old Python -'}}));self.assertEqual(m.sample()['reservedProcesses'],0)
  self.handles[-1].return_value={99:{'parent':1,'birth':'new','rss':1024,'command':'node /private/tmp/mcr-unknown/run.mjs'}}
  with self.assertRaisesRegex(AssertionError,'unresolved'):m.sample()
 def test_incomplete_live_observation_rejects_with_owner(self):
  (self.directory/'ordinary.json').write_text(json.dumps({'ordinaryObservation':{'pid':99,'role':'TEST_ONLY live incomplete'}}))
  with self.assertRaisesRegex(AssertionError,'incomplete live ordinary observation:.*ordinary'):m.sample()
 def test_absent_pid_keeps_reservation(self):
  self.record();self.assertEqual(m.sample()['reservedProcesses'],8)
 def test_observation_only_does_not_invent_workflow_or_full(self):
  (self.directory/'ordinary.json').write_text(json.dumps({'ordinaryObservation':{'actualpid':99,'birth':'now'}}));self.handles[-1].return_value={99:{'parent':1,'birth':'now','rss':1024,'command':'Python -'}};s=m.sample();self.assertEqual(s['reservedProcesses'],0);self.assertEqual(s['actualPids'],[99])
 def test_completed_observation_cannot_identify_new_payload(self):
  (self.directory/'ordinary.json').write_text(json.dumps({'ordinaryObservation':{'actualpid':99,'birth':'now','completedAt':3,'commandExit':0}}));self.handles[-1].return_value={99:{'parent':1,'birth':'now','rss':1024,'command':'Python /private/tmp/mcr-other/run.py'}}
  with self.assertRaisesRegex(AssertionError,'unresolved'):m.sample()
 def test_newer_offer_does_not_erase_unreleased_legacy(self):
  old=self.record(offerId='old',acceptedAtEpoch=1);self.legacy([old]);self.record()
  with self.assertRaisesRegex(AssertionError,'ambiguous'):m.sample()
 def test_exact_hashed_full_closes_only_old_offer(self):
  old=self.record(offerId='old',acceptedAtEpoch=1);self.legacy([old]);proof=self.base/'full.json';proof.write_text(json.dumps({'booking':'old','actualFullEpoch':3,'resources':0}))
  self.record(state='FULL',closedOffers=[{'ownerThreadId':'peer','offerId':'old','resources':0,'actualFullEpoch':3,'immutableFULLpath':str(proof),'immutableFULLsha256':hashlib.sha256(proof.read_bytes()).hexdigest()}]);self.assertEqual(m.sample()['owners'],[])
  resources={'source':0,'node':0,'browser':0,'guard':0,'descendants':0,'target':0,'growthReservation':0};proof.write_text(json.dumps({'offerId':'old','state':'FULL','actualFullEpoch':3,'resources':resources}))
  self.record(state='FULL',closedOffers=[{'ownerThreadId':'peer','offerId':'old','resources':resources,'actualFullEpoch':3,'fullPath':str(proof),'sha256':hashlib.sha256(proof.read_bytes()).hexdigest()}]);self.assertEqual(m.sample()['owners'],[])
 def test_unknown_runtime_fails_closed(self):
  self.handles[-1].return_value={99:{'parent':1,'birth':'now','rss':1024,'command':'node /private/tmp/mcr-unknown/run.mjs'}}
  with self.assertRaisesRegex(AssertionError,'unresolved'):m.sample()
 def test_pid_reuse_is_not_identity(self):
  self.record(controller={'pid':99,'birth':'old','script':'/private/tmp/mcr-known/run.py'});self.handles[-1].return_value={99:{'parent':1,'birth':'new','rss':1024,'command':'Python /private/tmp/mcr-known/run.py'}}
  with self.assertRaisesRegex(AssertionError,'birth mismatch'):m.sample()
 def test_ordinary_exact_publisher_has_no_workflow_reservation(self):
  self.record(kind='ordinary',state='FULL',controller={'pid':99,'birth':'now','script':'/private/tmp/mcr-publish/metadata.py'});self.handles[-1].return_value={99:{'parent':1,'birth':'now','rss':1024,'command':'Python /private/tmp/mcr-publish/metadata.py'}};s=m.sample();self.assertEqual(s['reservedProcesses'],0);self.assertEqual(s['actualPids'],[99])
 def test_ordinary_cannot_hide_compiler_child(self):
  self.record(kind='ordinary',state='FULL',controller={'pid':99,'birth':'now','script':'/private/tmp/mcr-publish/metadata.py'});self.handles[-1].return_value={99:{'parent':1,'birth':'now','rss':1024,'command':'Python /private/tmp/mcr-publish/metadata.py'},100:{'parent':99,'birth':'now','rss':1024,'command':'cargo test'}}
  with self.assertRaisesRegex(AssertionError,'runtime payload'):m.sample()

if __name__=='__main__':unittest.main()
