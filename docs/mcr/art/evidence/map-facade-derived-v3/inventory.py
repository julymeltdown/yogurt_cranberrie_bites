"""Read-only inode accounting and serialized process sampling for both controls."""
from pathlib import Path
import fcntl
import os
import subprocess

MIB=1024**2
def scan(roots):
    result={}
    for root in roots:
        pending=[Path(root)]
        while pending:
            path=pending.pop()
            try:st=path.lstat()
            except FileNotFoundError:continue
            if path.is_symlink():
                key=(st.st_dev,st.st_ino,getattr(st,'st_birthtime_ns',int(getattr(st,'st_birthtime',0)*10**9)))
                result[key]=st.st_size
            elif path.is_dir():
                try:pending.extend(Path(e.path) for e in os.scandir(path))
                except FileNotFoundError:continue
            elif path.is_file():
                key=(st.st_dev,st.st_ino,getattr(st,'st_birthtime_ns',int(getattr(st,'st_birthtime',0)*10**9)))
                result[key]=st.st_size
    return result

class Inventory:
    def __init__(self,roots,prior):
        self.roots=roots;self.baseline=scan(roots);self.seen={}
        self.logical=sum(self.baseline.values());self.carry=max(prior,self.logical+34*MIB)
    def sample(self):
        current=scan(self.roots)
        for key,size in current.items():
            gain=max(0,size-self.baseline.get(key,0))
            if gain:self.seen[key]=max(self.seen.get(key,0),gain)
        positive=sum(self.seen.values())
        return self.carry+positive,34*MIB+positive

def process_snapshot(supervisor,lock):
    # Two independent controls may sample, but only one transient ps process
    # exists at once. Supervisor+guardian+one work child+one sampler <=4.
    with Path(lock).open('a') as handle:
        fcntl.flock(handle,fcntl.LOCK_EX)
        sampler=subprocess.Popen(['ps','-axo','pid=,ppid=,rss=,comm='],stdout=subprocess.PIPE,text=True)
        raw=sampler.communicate()[0]
        table={}
        for line in raw.splitlines():
            parts=line.split(None,3)
            if len(parts)==4:table[int(parts[0])]=(int(parts[1]),int(parts[2])*1024,parts[3])
        owned={supervisor}
        while True:
            children={pid for pid,(parent,_,_) in table.items() if parent in owned}
            if children<=owned:break
            owned|=children
        if supervisor in table:
            parent=table[supervisor][0]
            if parent in table and Path(table[parent][2]).name in ('zsh','bash','sh'):owned.add(parent)
        active=owned&table.keys()
        assert supervisor in active,'Supervisor disappeared'
        return dict(processes=len(active),rss=sum(table[p][1] for p in active)+16*MIB,pids=sorted(active))

