"""Source planning only: enumerate exact HEAD-relative read dependencies; never restore files."""
import json, posixpath, re, subprocess
from pathlib import Path
root = Path(__file__).resolve().parents[6]
records = subprocess.check_output(['git','ls-tree','-r','-l','HEAD','frontend','contracts'],cwd=root,text=True).splitlines()
tracked = {}
for line in records:
 meta, name = line.split('\t',1)
 fields = meta.split()
 if fields[1]=='blob': tracked[name]=(fields[2],int(fields[3]))
seeds = [p.relative_to(root).as_posix() for p in (root/'frontend/src').rglob('*') if p.is_file() and p.suffix in ['.tsx','.ts','.css']]
seeds += ['frontend/src/app/community-adoption/__tests__/native/preview.tsx','frontend/src/features/gift-pickup/__tests__/pickup.test.tsx','frontend/src/features/gift-discovery/__tests__/client-completion.test.tsx','frontend/src/app/community-entry/__tests__/routes.test.tsx','frontend/src/features/giveaway-authoring/__tests__/authoring.test.tsx']
seen, pending, unresolved = set(), list(seeds), []
while pending:
 name = pending.pop()
 if name in seen: continue
 local=root/name
 if not local.is_file() and name not in tracked: continue
 seen.add(name)
 if Path(name).suffix not in ['.ts','.tsx','.js','.mjs','.css','.html']: continue
 text = local.read_text() if local.is_file() else subprocess.check_output(['git','show','HEAD:'+name],cwd=root,text=True)
 refs=re.findall(r'''(?:from\s*|import\s*\(\s*|import\s*|@import\s*|url\(\s*)["']([^"']+)["']''',text)
 for ref in refs:
  if not ref.startswith('.'): continue
  base=posixpath.normpath(posixpath.join(posixpath.dirname(name),ref.split('?')[0]))
  matches=[p for p in [base,base+'.ts',base+'.tsx',base+'.js',base+'.json',base+'/index.ts',base+'/index.tsx'] if p in tracked or (root/p).is_file()]
  if matches: pending.append(matches[0])
  else: unresolved.append({'importer':name,'reference':ref})
items=[{'path':p,'blob':tracked[p][0],'bytes':tracked[p][1]} for p in sorted(seen) if p in tracked and not (root/p).is_file()]
result={'base':subprocess.check_output(['git','rev-parse','HEAD'],cwd=root,text=True).strip(),'purpose':'Exact read-only dependencies for owned PI04 tests and functioning default router fixture; no materialization authorization implied.','count':len(items),'logicalBytes':sum(p['bytes'] for p in items),'files':items,'unresolved':unresolved}
out=root/'docs/mcr/evidence/parade-ink-giveaway/readonly-source-closure.json'
if out.exists(): raise SystemExit('Preserve the granted manifest. Select a new explicit output path for any revised readset.')
out.write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps({k:result[k] for k in ['count','logicalBytes','unresolved']}))
