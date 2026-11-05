"""Source05 continuation: all Source04 actuals retained with conservative16MiB."""
from pathlib import Path
import hashlib
raw=Path(__file__).with_name('controller04.py').read_bytes()
assert hashlib.sha256(raw).hexdigest()=='8a9ce9d992e72e048a174f95bed486e0b54c611b01ac031c5a947a3522351423'
source=raw.decode()
tail="""
code=code.replace('source04','source05').replace('60884868','44107652')
code=code.replace("['author.py','controller05.py'", "['source-control04.py','controller04.py','author.py','controller05.py'")
code=code.replace("saved=HERE/'source03-candidate';assert not saved.exists();saved.mkdir()", "saved=HERE/'source03-candidate';assert saved.exists(), 'Preserve the real Source03 candidate saved by Source04'")
code=code.replace('for p in oldfiles:shutil.copyfile(p,saved/p.name)', 'for p in oldfiles:assert (saved/p.name).exists()')
code=code.replace("control.atomic(saved/'manifest.json',dict(meaning='Actual Source03 candidate before material visual correction; qualifier/probe were unrun',files=[control.pin(saved/p.name) for p in oldfiles]))", "assert (saved/'manifest.json').exists()")
exec(compile(code,__file__,'exec'))
"""
exec(compile(source.replace("exec(compile(code,__file__,'exec'))",tail),__file__,'exec'))
