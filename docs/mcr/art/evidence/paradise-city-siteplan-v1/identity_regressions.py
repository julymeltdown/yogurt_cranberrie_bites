"""Changed identity fixtures only; previous thirteen-case evidence stays pinned."""
from inventory import controller_matches,cohort,OWNER
P='/Library/Frameworks/Python.framework/Versions/3.14/Resources/Python.app/Contents/MacOS/Python'
S='/Users/lhs/Desktop/mcr-worktrees/art-map-cozy-v1/docs/mcr/art/evidence/paradise-city-siteplan-v1/guard.py'
C=dict(pid=123,birth='Thu Nov 5 10:44:23 2026',executable=P,script=S)
R=dict(parent=100,rss=12345,birth=C['birth'],state='S',command=P+' -B '+S)
cache={};cases=0
def yes(value):
 global cases
 assert value;cases+=1
def no(action):
 global cases
 try:action()
 except AssertionError:cases+=1
 else:raise AssertionError('Unverified controller accepted')
yes(controller_matches(R,C,cache))
terminal=dict(R,command='(Python)')
yes(controller_matches(terminal,C,cache))
no(lambda:controller_matches(terminal,C,{}))
no(lambda:controller_matches(dict(terminal,birth='new birth'),C,cache))
no(lambda:controller_matches(terminal,dict(C,pid=999),cache))
no(lambda:controller_matches(terminal,dict(C,script=S+'.other'),cache))
no(lambda:controller_matches(dict(terminal,parent=999),C,cache))
record=dict(ownerThreadId=OWNER,offerId='fixture',state='ACCEPTED',kind='source',processCap=4,rssCapBytes=512*1024**2,remainingNewUpperBytes=100,controller=C,guardControllers=[])
state=cohort({123:terminal},{OWNER:record},set(),cache)
yes(state['reservedProcesses']==4 and state['reservedRSS']==512*1024**2 and state['workflows']==1 and state['owners'][0]['pids']==[123])
print({'status':'PASS','changedIdentityCases':cases,'unavailableArgvDoesNotReleaseReservation':True})
