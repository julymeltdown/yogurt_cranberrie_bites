"""QA02 supplemental full-Git-byte and normalized visual-source comparison.
Read-only Git object access also covers files omitted from the sparse worktree.
Common-token matches are review candidates, never an automatic originality verdict.
"""
import hashlib,json,pathlib,re,subprocess,collections
root=pathlib.Path(__file__).resolve().parents[4]
out=root/'docs/mcr/evidence/ui-origin'
old=pathlib.Path('/Users/lhs/Desktop/mcr-sources/bigyeting')
revision='5eecdcaffd1a9d505b9ab2a974f8d5623217e90a'
def git(repo,*args):return subprocess.check_output(['git','-C',str(repo),*args])
def blobs(repo,rev,prefix):
    rows=git(repo,'ls-tree','-r','-z',rev,'--',prefix).split(b'\0');entries=[]
    for row in rows:
        if not row:continue
        meta,path=row.split(b'\t');mode,kind,oid=meta.split()
        if kind==b'blob':entries.append((path.decode(),oid.decode()))
    proc=subprocess.Popen(['git','-C',str(repo),'cat-file','--batch'],stdin=subprocess.PIPE,stdout=subprocess.PIPE)
    result={}
    for path,oid in entries:
        proc.stdin.write((oid+'\n').encode());proc.stdin.flush();header=proc.stdout.readline();size=int(header.split()[2]);data=proc.stdout.read(size);proc.stdout.read(1);result[path]=(oid,data)
    proc.stdin.close();proc.wait();return result
candidate=git(root,'rev-parse','HEAD').decode().strip()
current=blobs(root,candidate,'frontend');legacy=blobs(old,revision,'frontend')
policy=json.loads((root/'docs/mcr/reuse-allowlist.json').read_text());inventory=json.loads((root/'docs/mcr/frontend/reuse-inventory.json').read_text())
excluded={e['sha256']:e['path'] for e in inventory['legacy_exclusions']}
violations=[]
for p,(oid,data) in current.items():
    h=hashlib.sha256(data).hexdigest()
    if h in excluded:violations.append({'path':p,'legacy':excluded[h]})
# Only JSX/TSX and styles can establish shared rendering/token matches here.
def visual(p):return (p.endswith(('.tsx','.css','.scss')) and '/__tests__/' not in p and not re.search(r'\.(test|spec)\.',p))
def tokens(data):
    text=data.decode('utf8',errors='replace');text=re.sub(r'/\*[\s\S]*?\*/|//[^\n]*',' ',text)
    return re.findall(r'[A-Za-z_$][\w$-]*|#[a-fA-F0-9]{3,8}|\d+(?:\.\d+)?|[^\s\w]',text)
# 32 consecutive normalized tokens, excluding whitespace and comments.
size=32;index=collections.defaultdict(set)
for p,(oid,data) in legacy.items():
    if visual(p):
        t=tokens(data)
        for i in range(len(t)-size+1):index[tuple(t[i:i+size])].add(p)
matches=[];rendering=[]
for p,(oid,data) in current.items():
    if not visual(p):continue
    t=tokens(data);counts=collections.Counter();example={}
    for i in range(len(t)-size+1):
        key=tuple(t[i:i+size])
        for source in index.get(key,[]):counts[source]+=1;example.setdefault(source,' '.join(key))
    rendering.append({'path':p,'blob':oid,'sha256':hashlib.sha256(data).hexdigest(),'tokens':len(t)})
    if counts:
        source,count=counts.most_common(1)[0];matches.append({'path':p,'source':source,'matching_windows':count,'current_tokens':len(t),'sample':example[source]})
# Preserve source archive and selected originals against task base; no checkout writes.
source=git(root,'diff','--name-only',candidate,'--','docs/source').decode().splitlines()
report={'candidate':candidate,'legacy_revision':revision,'legacy_checkout_head':git(old,'rev-parse','HEAD').decode().strip(),'current_frontend_blobs':len(current),'legacy_frontend_blobs':len(legacy),'excluded_hashes':len(excluded),'exact_legacy_matches':violations,'reviewed_extracts':policy['reviewed_headless_extracts'],'rendering_files':rendering,'normalized_window_size':size,'similarity_candidates':sorted(matches,key=lambda x:-x['matching_windows']),'source_worktree_changes':source,'limits':'Token windows identify common source for manual review; they cannot prove originality, transformed art or behavioral acceptance. All tracked frontend bytes were read from Git, including sparse-omitted PNGs.'}
(out/'source-audit.json').write_text(json.dumps(report,indent=2)+'\n')
print(json.dumps({k:report[k] for k in ['candidate','current_frontend_blobs','legacy_frontend_blobs','exact_legacy_matches']}));print(json.dumps(report['similarity_candidates'][:15],indent=2))
