"""Freeze actual source/fixture/asset inputs without editing borrowed producers."""
from pathlib import Path
import hashlib
import json
import subprocess
root=Path(__file__).resolve().parents[4]
out=Path(__file__).resolve().parent
paths=set()
for prefix in ['frontend/src','frontend/public','contracts','frontend/e2e/reference-integrated/whole']:
    for path in (root/prefix).rglob('*'):
        if path.is_file() and not path.is_symlink() and not any(part in {'evidence','node_modules','__pycache__'} for part in path.relative_to(root).parts):
            paths.add(path.relative_to(root))
for name in ['frontend/package.json','frontend/pnpm-lock.yaml','frontend/vite.config.ts','frontend/tsconfig.json']:
    paths.add(Path(name))
value={'base':'fcac0bed033b3f0bca336df6417d3ff617ffb637','head':subprocess.check_output(['git','rev-parse','HEAD'],cwd=root,text=True).strip(),
       'qualification':'Read-only source/controlled fixture/immutable public asset inputs; no live producer certification',
       'files':{str(path):hashlib.sha256((root/path).read_bytes()).hexdigest() for path in sorted(paths)}}
target=out/'source-readset.json'
assert not target.exists()
target.write_text(json.dumps(value,indent=2)+'\n')
print(json.dumps({'inputs':len(paths),'output':str(target)}))
