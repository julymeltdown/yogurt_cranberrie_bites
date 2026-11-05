"""Corrected fresh round; failed Source07 bootstrap remains immutable."""
from pathlib import Path
import hashlib
raw=Path(__file__).with_name('source-control07.py').read_bytes()
code=raw.decode().replace('ART231-PHOTO2-SOURCE07-0734','ART231-PHOTO2-SOURCE08-0752').replace('source07-independent-full.json','source08-independent-full.json')
code=code.replace('475840002+positive','492617218+positive')
code=code.replace("exec(compile(m,__file__,'exec'),base.__dict__)","m=m.replace('  result=census(c)',\"  self.peak['positive']=max(self.peak['positive'],positive);self.peak['whole']=max(self.peak['whole'],whole)\\n  result=census(c)\")\nexec(compile(m,__file__,'exec'),base.__dict__)")
exec(compile(code,str(Path(__file__).resolve()),'exec'))
