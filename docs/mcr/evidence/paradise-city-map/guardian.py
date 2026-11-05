import os
import signal
import subprocess
import sys
import traceback
import time
sys.dont_write_bytecode=True
from finite import EVIDENCE, ROOT, Round
import membership

round=Round('guardian')
resource=__import__('json').loads(membership.OWN.read_text())
assert resource['state']=='ACCEPTED' and resource['offerId']==round.lease
resource['guardControllers']=[membership.identity(os.getpid(),EVIDENCE/'guardian.py',round.out)]; membership.publish(resource)
code,failure,child=1,None,None
signal.signal(signal.SIGTERM,lambda *_: (_ for _ in ()).throw(RuntimeError('Orderly stop requested')))
try:
    round.initialize()
    node='/Users/lhs/Desktop/mcr-tools/node-v24.22.2-darwin-arm64/bin/node'
    if round.mode=='source':
        all_commands=dict(zip(('metadata','yieldmetadata','consumer','adapter','types','lint'), [[sys.executable,'-B',str(EVIDENCE/'membership_test.py'),
          'MembershipTests.test_inline_metadata_filename_is_not_an_executed_script',
          'MembershipTests.test_unknown_relative_python_script_still_fails_closed',
          'MembershipTests.test_factual_ordinary_parent_counts_without_hiding_runtime_siblings',
          'MembershipTests.test_ordinary_cannot_hide_compiler_or_test_child'],
          [sys.executable,'-B',str(EVIDENCE/'membership_test.py'),
          'MembershipTests.test_unready_yield_is_not_a_workflow_reservation',
          'MembershipTests.test_unready_yield_cannot_hide_positive_or_unknown_work',
          'MembershipTests.test_unready_yield_needs_actual_prior_full_to_reconcile_legacy'],
          [node,'node_modules/vitest/vitest.mjs','run','--config','src/features/paradise-city-map/__tests__/consumer/vitest.config.ts','--configLoader','runner'],
          [node,'node_modules/vitest/vitest.mjs','run','--config','src/features/paradise-city-map/__tests__/consumer/adapter-vitest.config.ts','--configLoader','runner'],
          [node,str(EVIDENCE/'typecheck.mjs')],
          [node,'node_modules/oxlint/bin/oxlint','--deny-warnings','src/features/paradise-city-map/ui','src/features/paradise-city-map/model','src/features/paradise-city-map/ports.ts','src/features/paradise-city-map/index.ts','src/features/paradise-city-map/__tests__/consumer','src/app/venue-entry']]))
        stages=os.environ.get('MCR_VENU_SOURCE_STAGES','metadata,consumer,types,lint').split(',')
        assert stages and len(set(stages))==len(stages) and all(stage in all_commands for stage in stages)
        commands=[all_commands[stage] for stage in stages]
    else:
        native_commands=dict(zip(('metadata','lint','syntax','browser','compare'),[[sys.executable,'-B',str(EVIDENCE/'membership_test.py'),
          'MembershipTests.test_exact_new_full_resamples_a_stale_terminal_claim',
          'MembershipTests.test_incomplete_or_other_booking_full_cannot_release_terminal_claim',
          'MembershipTests.test_terminal_identity_requires_this_guard_prior_proof_and_keeps_full_cap'],
          [node,'node_modules/oxlint/bin/oxlint','--deny-warnings','e2e/venue-map/native.mjs'],
          [node,'--check','e2e/venue-map/native.mjs'],[node,'e2e/venue-map/native.mjs'],[sys.executable,'-B',str(EVIDENCE/'compare.py')]]))
        stages=os.environ.get('MCR_VENU_NATIVE_STAGES','lint,syntax,browser,compare').split(',')
        assert stages and len(set(stages))==len(stages) and all(stage in native_commands for stage in stages)
        commands=[native_commands[stage] for stage in stages]
    env=dict(os.environ,TMPDIR=str(round.tmp)+'/',PATH=str(__import__('pathlib').Path(node).parent)+':'+os.environ['PATH'])
    for index,argv in enumerate(commands):
        round.check()
        with (round.out/f'command-{index:02}.log').open('wb') as log:
            child=subprocess.Popen(argv,cwd=ROOT/'frontend',env=env,stdout=log,stderr=subprocess.STDOUT,start_new_session=True)
            round.seen[child.pid]=membership.processes()[child.pid]['birth']
            while child.poll() is None:
                round.check(); time.sleep(.25)
        code=child.returncode; round.event(dict(type='COMMAND',index=index,argv=argv,exitCode=code,at=time.time()))
        if code: raise RuntimeError('Command failed; preserve immutable attempt and close offer')
    code=0
except BaseException as error:
    failure=traceback.format_exc(); code=1
finally:
    value=round.release(code,failure,child)
raise SystemExit(code if not value['resources'] else 2)
