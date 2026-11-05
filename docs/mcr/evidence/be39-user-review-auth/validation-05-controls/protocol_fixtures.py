"""Explicit synthetic decoder fixtures. Never writes shared owner metadata."""
import json
import hashlib
import signal
from unittest.mock import patch
from pathlib import Path
import budget
from budget import R, reconciled_cohorts, identity_resolved, identity_gate

OWNER='11111111-1111-4111-8111-111111111111'
ISSUER='01a1a21d-39ec-7912-84cb-910058e7a110'

def check_protocol(actual_plan):
    directory=R/'protocol-fixture-data';directory.mkdir()
    owners=directory/'owners';owners.mkdir()
    # Intentionally never created: canonical acceptance precedes local logs.
    accepted=directory/'intentionally-not-created-accept.jsonl'
    full=directory/'synthetic-full.json';full.write_text(json.dumps({'synthetic':True,'state':'FULL_RELEASED','owned_children':0})+'\n')
    membership=directory/'membership.json';overrides=directory/'overrides.json'
    overrides.write_text(json.dumps({'cohorts':[],'infrastructure':[]})+'\n')
    plan={'owner_receipt_directory':str(owners),'management_membership_file':str(membership),
          'cohort_overrides_file':str(overrides),'historical_receipt_aliases_file':actual_plan['historical_receipt_aliases_file']}
    record={'ownerThreadId':OWNER,'offerId':'SYNTHETIC-BOOKING-A','state':'ACCEPTED','kind':'target',
            'acceptedAtEpoch':100.0,'workDeadlineEpoch':1000.0,'fullDeadlineEpoch':1180.0,
            'processCap':32,'rssCapBytes':8*1024**3,'remainingNewUpperBytes':2*1024**3,
            'controller':None,'guardControllers':[],
            'receiptPaths':{'accept':str(accepted),'full':str(full)}}
    def decode(current,entries=()):
        (owners/(OWNER+'.json')).write_text(json.dumps(current)+'\n')
        membership.write_text(json.dumps({'issuer':ISSUER,'entries':list(entries)})+'\n')
        return reconciled_cohorts(plan)
    def active(values):return [x for x in values.values() if x['state'].startswith('ACTUAL_ACCEPT')]
    phases=['ACCEPTED','ACCEPTED_BOOTSTRAP','ACCEPTED_GUARDS_STARTING','ACCEPTED_INNER_CENSUS','ACCEPTED_RUNNING','ACCEPTED_EXECUTABLES_COPYING']
    for phase in phases:
        current=dict(record,state=phase);values=decode(current);rows=active(values)
        assert len(rows)==1 and rows[0]['controller']=={}
        assert rows[0]['actualAccept']==100.0 and rows[0]['offerId']==record['offerId']
        assert rows[0]['processCap']==32 and rows[0]['rssCapGiB']==8
        assert rows[0]['remainingNewConservativeUpperBytes']==2*1024**3
    legacy=dict(record,owner=OWNER,cohort='SYNTHETIC-LEGACY-BOOKING-A',knownRootMatchers=[])
    values=decode(dict(record,state='FULL'),[legacy])
    assert len(values)==1 and not active(values)
    assert next(iter(values.values()))['state'].startswith('LITERAL_FULL')
    values=decode(dict(record,state='FULL',offerId='SYNTHETIC-BOOKING-B',acceptedAtEpoch=101.0),[legacy])
    rows=active(values)
    assert len(rows)==1 and rows[0]['offerId']==record['offerId'] and rows[0]['actualAccept']==100.0
    assert rows[0]['processCap']==32 and rows[0]['remainingNewConservativeUpperBytes']==2*1024**3
    ordinary=dict(record,kind='ordinary',processCap=None,rssCapBytes=None,remainingNewUpperBytes=None,
                  receiptPaths={'accept':str(directory/'intentionally-absent-ordinary-file.json')})
    values=decode(ordinary,[legacy]);rows=active(values)
    assert len(rows)==1 and rows[0]['processCap']==32 and rows[0]['remainingNewConservativeUpperBytes']==2*1024**3
    ordinary_rows=[x for x in values.values() if x.get('kind')=='ordinary']
    assert len(ordinary_rows)==1 and ordinary_rows[0]['processCap']==0 and ordinary_rows[0]['rssCapGiB']==0
    assert ordinary_rows[0]['remainingNewConservativeUpperBytes']==0
    values=decode(record,[legacy]);rows=active(values)
    assert len(rows)==1 and rows[0]['processCap']==32 and not accepted.exists()
    terminal={'offerId':record['offerId'],'acceptedAtEpoch':100.0,'actualFullEpoch':120.0,
        'resourcesReleased':True,'fullPath':str(full),'fullSHA':hashlib.sha256(full.read_bytes()).hexdigest()}
    # A newer booking can retain the exact terminal history for its predecessor.
    next_record=dict(record,offerId='SYNTHETIC-BOOKING-B',acceptedAtEpoch=121.0,closedOffers=[terminal])
    values=decode(next_record,[legacy]);rows=active(values)
    assert len(rows)==1 and rows[0]['offerId']=='SYNTHETIC-BOOKING-B'
    assert values['SYNTHETIC-LEGACY-BOOKING-A']['state'].startswith('LITERAL_FULL')
    bad_history=dict(terminal,acceptedAtEpoch=99.0)
    values=decode(dict(ordinary,closedHistory=[bad_history]),[legacy])
    assert len(active(values))==1 and active(values)[0]['actualAccept']==100.0
    values=decode(dict(ordinary,closedHistory=[dict(terminal,kind='ordinary')]),[legacy])
    assert len(active(values))==1
    birth='Thu Nov 5 11:26:47 2026';witness={'pid':333,'birth':birth,'command':'/absolute/runtime /absolute/controller.py'}
    row=(444,333,4096,'/absolute/runtime /absolute/controller.py',birth)
    assert not identity_resolved(witness,{},set(),{})
    assert not identity_resolved(witness,{333:(444,333,4096,row[3],'different birth')},{333},{})
    assert identity_resolved(witness,{333:row},{333},{})
    assert identity_resolved(witness,{},set(),{'owner':{'controller':{'pid':333,'birth':birth,'executable':'/absolute/runtime','script':'/absolute/controller.py','cwd':'/absolute'}}})
    gate_root=directory/'gates';gate_root.mkdir()
    (gate_root/'groups.json').write_text('[333]')
    (gate_root/'group-identities.json').write_text(json.dumps({'333':{'birth':birth}}))
    events=[]
    with patch.object(budget,'R',gate_root),patch.object(budget.os,'killpg',side_effect=lambda pg,sig:events.append((pg,sig))):
        with patch.object(budget.os,'getpid',return_value=501):
            assert identity_gate({}, {'controller_pid':444},[witness],{333:row})
        with patch.object(budget.os,'getpid',return_value=502):
            assert identity_gate({}, {'controller_pid':444},[],{333:row})
        assert not any(sig==signal.SIGCONT for _,sig in events)
        with patch.object(budget.os,'getpid',return_value=501):
            assert not identity_gate({}, {'controller_pid':444},[],{333:row})
        assert events[-1]==(333,signal.SIGCONT)
    assert all(pg==333 for pg,_ in events)
    return {'status':'PASS','synthetic_only':True,'cases':10,'accepted_phase_variants':phases,
            'results':['accepted null-controller retains full reservation and original epoch across exact known phases',
                       'matching owner/offer/epoch literal FULL closes its accepted reservation',
                       'different offer/epoch FULL does not close the prior accepted reservation',
                       'ordinary null-cap booking invents no execution cap and cannot close nonordinary acceptance',
                       'canonical and embedded acceptance both retain full caps without a local acceptance log',
                       'new booking exact owner/offer/epoch terminal history closes only its predecessor',
                       'different history epoch does not close an accepted claim',
                       'ordinary terminal history cannot close a nonordinary claim',
                       'absence and reused PID do not resolve identity; exact registered birth or factual descriptor does',
                       'two sampler gates pause only a verified own group and resume only when both are clear; signals mocked'],
            'shared_owner_metadata_written':False,'real_process_identity_validation':'Owned live guards continue actual metadata/PID/birth sampling separately'}
