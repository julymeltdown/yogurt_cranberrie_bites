"""Pure fixtures for actual identity and immutable ACCEPT-relative clocks."""
from inventory import controller_matches,qualification_deadlines,cohort,OWNER

BIRTH='Thu Nov 5 10:29:38 2026'
PYTHON='/Library/Frameworks/Python.framework/Versions/3.14/Resources/Python.app/Contents/MacOS/Python'
SCRIPT='/Users/lhs/Desktop/mcr-worktrees/art-map-cozy-v1/docs/mcr/art/evidence/paradise-city-siteplan-v1/guard.py'
CWD='/Users/lhs/Desktop/mcr-worktrees/art-map-cozy-v1'
controller=dict(pid=123,birth=BIRTH,executable=PYTHON,script=SCRIPT,cwd=CWD)
row=dict(parent=1,rss=1000,birth=BIRTH,state='S',command=PYTHON+' -B '+SCRIPT)
count=0
def yes(value):
 global count
 assert value;count+=1
def rejects(action):
 global count
 try:action()
 except AssertionError:count+=1
 else:raise AssertionError('Invalid census fixture accepted')
yes(controller_matches(row,controller))
relative=dict(row,command=PYTHON+' -B docs/mcr/art/evidence/paradise-city-siteplan-v1/guard.py',actualCwd=CWD)
yes(controller_matches(relative,controller))
rejects(lambda:controller_matches(dict(relative,actualCwd='/tmp'),controller))
rejects(lambda:controller_matches(dict(row,birth='another birth'),controller))
rejects(lambda:controller_matches(dict(row,command='/bin/sh '+SCRIPT),controller))
rejects(lambda:controller_matches(dict(row,command=PYTHON+' -B '+SCRIPT+'.other'),controller))
yes(controller_matches(dict(row,state='Z',command='(Python)'),controller))
rejects(lambda:controller_matches(dict(row,state='Z',birth='reused PID',command='(Python)'),controller))
record=dict(ownerThreadId=OWNER,offerId='fixture',state='ACCEPTED',kind='source',processCap=4,rssCapBytes=512*1024**2,remainingNewUpperBytes=100,controller=controller,guardControllers=[])
state=cohort({123:dict(row,state='Z',command='(Python)')},{OWNER:record},set())
yes(state['workflows']==1 and state['reservedProcesses']==4 and state['reservedRSS']==512*1024**2)
state=cohort({}, {OWNER:dict(record,controller=None)},set())
yes(state['workflows']==1 and state['reservedProcesses']==4)
rejects(lambda:cohort({999:dict(row,command=PYTHON+' /tmp/mcr-unknown.py')},{},set()))
yes(qualification_deadlines(100,307,367)==(280,340))
yes(qualification_deadlines(100,250,320)==(250,320))
print({'status':'PASS','guardRegressionCases':count,'noExternalWorkload':True})
