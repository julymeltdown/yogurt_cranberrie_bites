#!/usr/bin/env python3
"""Render editable native proposals with an existing Tesseract install."""
import json
import os
from pathlib import Path
import shutil
import subprocess

HERE=Path(__file__).resolve().parent
CLI=os.environ.get('MCR_TSRCT') or shutil.which('tsrct') or str(Path.home()/'Library/Application Support/Tesseract/bin/tsrct')
results=[]
for skin in ['parade','revenge','danger']:
    for kind in ['ground','scenery']:
        stem=HERE/f'{skin}-{kind}';project=str(stem)+'.tsrct'
        commands=[]
        if not Path(project).exists():commands.append(['project','create','--project',project])
        commands += [['project','commit','--project',project,'--file',str(stem)+'-editable.json'],['preview','--project',project,'--time','1','--output',str(stem)+'.png']]
        for args in commands:
            result=subprocess.run([CLI]+args,capture_output=True,text=True)
            results.append({'command':['tsrct']+[a.replace(str(HERE)+'/', '') for a in args],'exit_code':result.returncode,'output':(result.stdout+result.stderr).replace(str(HERE)+'/', '')})
            print(skin,kind,args[0],result.returncode,flush=True)
            if result.returncode:raise SystemExit(result.stdout+result.stderr)
(HERE/'render-evidence.json').write_text(json.dumps(results,indent=2)+'\n')
