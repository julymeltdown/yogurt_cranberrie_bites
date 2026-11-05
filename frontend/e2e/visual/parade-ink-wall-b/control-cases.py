"""Isolated regressions for actual receipt compatibility and fail-closed boundaries."""
import hashlib
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch
import resource_receipts as r

class Cases(unittest.TestCase):
    def setUp(self):
        self.temp=tempfile.TemporaryDirectory(dir=sys.argv[1]);self.base=Path(self.temp.name)
        self.directory=self.base/'owners';self.directory.mkdir()
        (self.base/'resource-receipt-interface.json').write_text(json.dumps({'directory':str(self.directory)}))
        self.members=[];self.config={'job':'TEST','censusRoot':None}
        self.records={r.OWNER:{'ownerThreadId':r.OWNER,'offerId':'TEST','state':'ACCEPTED','kind':'browser','acceptedAtEpoch':1,'processCap':24,'rssCapBytes':4*r.GiB,'controller':{'pid':111,'birth':'TEST_BIRTH','script':'/mcr/test.py'}}}
        self.rows=[(111,1,1024,'python /mcr/test.py')]
    def tearDown(self):self.temp.cleanup()
    def census(self):
        (self.base/'resource-membership-current.json').write_text(json.dumps({'asOfUTC':'TEST','entries':self.members}))
        for owner,record in self.records.items():(self.directory/(owner+'.json')).write_text(json.dumps(record))
        with patch.object(r,'BASE',self.base),patch.object(r,'birth',return_value='TEST_BIRTH'),patch.object(r.subprocess,'check_output',return_value='/actual/python\n',side_effect=getattr(self,'comm_response',None)):
            return r.census(self.config,self.rows)
    def observation(self,booking,**extra):
        self.records['ordinary']={'ordinaryObservation':dict(actualpid=222,birth='TEST_BIRTH',role='ordinary Git',cwd='/mcr',booking=booking,commandDigest=hashlib.sha256(b'python -').hexdigest(),**extra)}
        self.rows.append((222,1,1024,'python -'))
    def test_observation_only_ended(self):
        self.observation({'id':'PREP'},completedAt=2,commandExit=0)
        self.assertEqual(self.census()['claims'],1)
    def test_absent_ordinary_observation_is_not_a_live_identity(self):
        self.records['ordinary']={'ordinaryObservation':{'actualpid':222,'birth':'TEST_BIRTH'}}
        self.assertEqual(self.census()['claims'],1)
        self.records['ordinary']={'ordinaryObservation':{'pid':222,'birth':'TEST_BIRTH'}}
        self.assertEqual(self.census()['claims'],1)
        self.rows.append((222,1,1024,'python /mcr/ordinary.py'))
        with self.assertRaisesRegex(AssertionError,'Incomplete live ordinary observation'):self.census()
        self.records['ordinary']={'ownerThreadId':'ordinary','offerId':'CLOSED','state':'FULL','kind':'browser','controller':{'pid':222,'birth':'TEST_BIRTH','script':'/mcr/ordinary.py'},'ordinaryObservation':{'actualpid':222,'birth':'TEST_BIRTH','cwd':'/mcr','role':'ordinary final metadata writer','executable':'/actual/python','commandDigest':hashlib.sha256(b'python /mcr/ordinary.py').hexdigest()}}
        result=self.census()
        self.assertEqual(result['claims'],1)
        self.assertEqual(next(item for item in result['observed'] if item['owner']=='ordinary')['pids'],1)
    def test_string_booking_live_digest(self):
        self.observation('PREP')
        result=self.census();self.assertEqual(result['claims'],1);self.assertEqual(len(result['observed']),2)
    def test_dictionary_booking_live_digest(self):
        self.observation({'id':'PREP'})
        self.assertEqual(self.census()['observed'][-1]['offer'],'PREP')
    def test_null_script_ordinary(self):
        self.records['ordinary']={'ownerThreadId':'ordinary','offerId':'PREP','state':'ACCEPTED','kind':'ordinary','processCap':None,'rssCapBytes':None,'controller':{'pid':222,'birth':'TEST_BIRTH','executable':'/actual/python','script':None,'outputRoot':None}}
        self.rows.append((222,1,1024,'python -'));self.assertEqual(self.census()['claims'],1)
    def test_absent_controller_reservation(self):
        self.records['absent']={'ownerThreadId':'absent','offerId':'SOURCE','state':'ACTUAL_ACCEPT','kind':'source','processCap':6,'rssCapBytes':r.GiB,'controller':{'pid':333,'birth':'TEST_BIRTH','script':'/mcr/absent.py'}}
        result=self.census();self.assertEqual(result['claims'],2);self.assertEqual(result['reservedPids'],30)
    def test_unknown_script_fails(self):
        self.rows.append((222,1,1024,'python /mcr/unknown.py'))
        with self.assertRaisesRegex(AssertionError,'Unresolved live'):self.census()
    def test_guard_ancestor_deduplicates_and_requires_birth(self):
        self.records['target']={'ownerThreadId':'target','offerId':'TARGET','state':'ACCEPTED','kind':'target','processCap':3,'rssCapBytes':4096,'controller':{'pid':222,'birth':'TEST_BIRTH','script':'/mcr/run.py'},'guardControllers':[{'pid':333,'birth':'TEST_BIRTH','script':'/mcr/guardian.py'}]}
        self.rows.extend([(333,1,1024,'python /mcr/guardian.py'),(222,333,1024,'python /mcr/run.py'),(444,222,1024,'cargo /mcr/build')])
        observed=next(item for item in self.census()['observed'] if item['owner']=='target')
        self.assertEqual((observed['pids'],observed['rss']),(3,3072))
        self.records['target']['guardControllers'][0]['birth']='OTHER_BIRTH'
        with self.assertRaisesRegex(AssertionError,'Guard PID birth mismatch'):self.census()
        self.records['target']['guardControllers'][0].update(birth='TEST_BIRTH',script='/mcr/wrong-guardian.py')
        with self.assertRaisesRegex(AssertionError,'Guard absolute script mismatch'):self.census()
    def test_ended_ordinary_identity_query_restarts_complete_census(self):
        self.observation('PREP',executable='/actual/python')
        self.rows=r.ProcessRows(self.rows,{111:'TEST_BIRTH',222:'TEST_BIRTH'})
        def comm_response(command,**_kwargs):
            raise r.subprocess.CalledProcessError(1,command)
        self.comm_response=comm_response
        fresh=r.ProcessRows([self.rows[0]],{111:'TEST_BIRTH'})
        with patch.object(r,'processes',return_value=fresh):result=self.census()
        self.assertEqual(result['reservedPids'],24)
        self.assertFalse(any(item['owner']=='ordinary' for item in result['observed']))
        # An unchanged live identity must retain the failure, not be skipped.
        fresh=r.ProcessRows(self.rows,{111:'TEST_BIRTH',222:'TEST_BIRTH'})
        with patch.object(r,'processes',return_value=fresh),self.assertRaises(r.subprocess.CalledProcessError):self.census()
    def test_exact_old_and_new_infrastructure_pairs_require_birth(self):
        self.rows=r.ProcessRows([*self.rows,(333,1,1024,'node /mcr/old/kernel.js'),(444,1,1024,'node /mcr/new/kernel.js')],{111:'TEST_BIRTH',333:'TEST_BIRTH',444:'TEST_BIRTH'})
        old={'pids':[333],'ppid':1,'pgid':99,'birth':'TEST_BIRTH','executable':'/actual/node','actualPrograms':['/mcr/old/kernel.js']}
        new={**old,'pids':[444],'actualPrograms':['/mcr/new/kernel.js']}
        interface={'directory':str(self.directory),'verifiedInfrastructureObservation':old,'verifiedInfrastructureObservations':[new]}
        self.comm_response=lambda command,**_kwargs:'/actual/node\n' if command[-1]=='comm=' else '99\n'
        path=self.base/'resource-receipt-interface.json';path.write_text(json.dumps(interface))
        self.assertEqual(self.census()['unknown'],[])
        interface['verifiedInfrastructureObservations'][0]['birth']='OTHER_BIRTH';path.write_text(json.dumps(interface))
        with self.assertRaisesRegex(AssertionError,'Unresolved live workflow'):self.census()
        interface['verifiedInfrastructureObservations'][0]['birth']='TEST_BIRTH';path.write_text(json.dumps(interface))
        self.rows.append((555,1,1024,'python /mcr/bootstrap.py'));self.rows.births[555]='TEST_BIRTH'
        self.config.update(initialIdentityReconciliation=True,censusRoot=str(self.base),workDeadline=r.time.time()+10)
        fresh=r.ProcessRows([row for row in self.rows if row[0]!=555],{pid:value for pid,value in self.rows.births.items() if pid!=555})
        with patch.object(r,'processes',return_value=fresh),patch.object(r.time,'sleep'):
            self.assertEqual(self.census()['unknown'],[])
        with patch.object(r,'processes',return_value=self.rows),patch.object(r.time,'sleep'),self.assertRaisesRegex(AssertionError,'Unresolved live workflow'):
            self.census()
        (self.base/'payload.started').write_text('payload boundary')
        with patch.object(r,'processes',return_value=fresh) as resample,self.assertRaisesRegex(AssertionError,'Unresolved live workflow'):
            self.census()
        self.assertEqual(resample.call_count,0)
    def test_ordinary_cannot_hide_node(self):
        self.observation('PREP');self.rows.append((333,222,1024,'node /mcr/test.mjs'))
        with self.assertRaisesRegex(AssertionError,'Ordinary role cannot'):self.census()
    def test_relative_primary_requires_actual_cwd_executable_and_exact_program(self):
        controller=self.records[r.OWNER]['controller']
        controller.update(script='/mcr/owned folder/start.py',executable='/actual/python')
        self.rows=[(111,1,1024,"python 'owned folder/start.py'")]
        def metadata(command,**_kwargs):
            return '/actual/python\n' if command[0]=='ps' else 'p111\nn/mcr\n'
        self.comm_response=metadata
        self.assertEqual(self.census()['claims'],1)
        self.comm_response=lambda command,**kwargs:'/wrong/python\n' if command[0]=='ps' else metadata(command,**kwargs)
        with self.assertRaisesRegex(AssertionError,'Controller script/root mismatch'):self.census()
        self.comm_response=lambda command,**kwargs:metadata(command,**kwargs) if command[0]=='ps' else 'p111\nn/wrong\n'
        with self.assertRaisesRegex(AssertionError,'Controller script/root mismatch'):self.census()
        self.comm_response=metadata
        self.rows=[(111,1,1024,"python other.py --output 'owned folder/start.py'")]
        with self.assertRaisesRegex(AssertionError,'Controller script/root mismatch'):self.census()
        self.rows=[(111,1,1024,"python -c 'print(1)' 'owned folder/start.py'")]
        with self.assertRaisesRegex(AssertionError,'Controller script/root mismatch'):self.census()
    def test_absent_ordinary_guard_history_does_not_hide_live_guard(self):
        self.records['ordinary']={'ownerThreadId':'ordinary','offerId':'PREP','state':'FULL','kind':'ordinary','controller':None,'guardControllers':[{'pid':333,'birth':'TEST_BIRTH','script':'/mcr/old-guard.py'}]}
        self.assertEqual(self.census()['claims'],1)
        self.rows.append((333,1,1024,'python /mcr/old-guard.py'))
        with self.assertRaisesRegex(AssertionError,'Ordinary role cannot declare live execution guards'):self.census()
    def test_ordinary_image_departure_restarts_complete_census_but_live_mismatch_fails(self):
        self.observation('PREP',executable='/actual/python')
        self.rows=r.ProcessRows(self.rows,{111:'TEST_BIRTH',222:'TEST_BIRTH'})
        self.config['workDeadline']=r.time.time()+10
        self.comm_response=lambda *_args,**_kwargs:'/ended/image\n'
        fresh=r.ProcessRows([self.rows[0]],{111:'TEST_BIRTH'})
        with patch.object(r,'processes',return_value=fresh),patch.object(r.time,'sleep'):
            result=self.census()
        self.assertEqual(result['claims'],1)
        self.assertFalse(any(item['owner']=='ordinary' for item in result['observed']))
        fresh=r.ProcessRows(self.rows,{111:'TEST_BIRTH',222:'TEST_BIRTH'})
        with patch.object(r,'processes',return_value=fresh),patch.object(r.time,'sleep'),self.assertRaisesRegex(AssertionError,'Ordinary actual executable mismatch'):
            self.census()
    def test_nested_yield_preserves_active_and_legacy_claims(self):
        self.records['yielded']={'ownerThreadId':'yielded','offerId':'OLD','acceptedAtEpoch':1,'state':'ACCEPTED','kind':'source','processCap':6,'rssCapBytes':r.GiB,'controller':None,'guardControllers':[],'queueDisposition':{'offerId':'NEW','state':'UNREADY_YIELD'}}
        self.members=[{'owner':'yielded','offerId':'OLD','acceptedAtEpoch':1,'state':'ACCEPTED','kind':'source'}]
        result=self.census();self.assertEqual(result['claims'],2);self.assertEqual(result['reservedPids'],30)
        self.rows.append((222,1,1024,'python /mcr/hidden.py'))
        with self.assertRaisesRegex(AssertionError,'Unresolved live workflow'):self.census()
        self.rows.pop()
        self.records['yielded'].update(offerId='NEW',state='FULL',acceptedAtEpoch=2)
        with self.assertRaisesRegex(AssertionError,'Unresolved legacy booking'):self.census()
        self.members=[];self.records['yielded']['state']='UNREADY_YIELD'
        with self.assertRaisesRegex(AssertionError,'Unknown receipt state'):self.census()
    def test_same_offer_exact_epoch(self):
        self.assertTrue(r.same_offer({'offerId':'A','acceptedAtEpoch':1},{'offerId':'A','acceptedAtEpoch':1}))
        self.assertFalse(r.same_offer({'offerId':'A','acceptedAtEpoch':1},{'offerId':'B','acceptedAtEpoch':1}))
        self.assertFalse(r.same_offer({'offerId':'A','acceptedAtEpoch':1},{'offerId':'A','acceptedAtEpoch':2}))
        proof=self.base/'literal-full.json';proof.write_text(json.dumps({'offerId':'OLD','state':'FULL','acceptedEpoch':1,'actualFullEpoch':2,'resources':{'node':0,'guard':0,'descendants':0}}))
        self.records['closed']={'ownerThreadId':'closed','offerId':'PREP','state':'ACCEPTED','kind':'ordinary','controller':None,'closedHistory':[{'offerId':'OLD','actualFullEpoch':2,'fullPath':str(proof),'sha256':hashlib.sha256(proof.read_bytes()).hexdigest()}]}
        self.members=[{'owner':'closed','offerId':'OLD','acceptedAtEpoch':1,'state':'ACCEPTED','kind':'source'}]
        self.assertEqual(self.census()['claims'],1)
        self.members[0]['acceptedAtEpoch']=3
        with self.assertRaisesRegex(AssertionError,'Unresolved legacy'):self.census()

if __name__=='__main__':
    selected=unittest.defaultTestLoader.loadTestsFromNames(sys.argv[2:],module=sys.modules[__name__]) if len(sys.argv)>2 else unittest.defaultTestLoader.loadTestsFromTestCase(Cases)
    result=unittest.TextTestRunner(verbosity=2).run(selected)
    sys.exit(0 if result.wasSuccessful() else 1)
