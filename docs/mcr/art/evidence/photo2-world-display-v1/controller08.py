"""Fresh source round after failed pre-ACCEPT Source07 projection lookup."""
from pathlib import Path
raw=Path(__file__).with_name('controller07.py').read_text()
code=raw.replace('source07','source08').replace('source-control07.py','source-control08.py').replace('SOURCE07-0734','SOURCE08-0752').replace('controller07.py','controller08.py')
code=code.replace("projection['totalConservativeFutureUpperBytes']","projection['conservativeApplicableFutureUpperBytes']")
code=code.replace('1793866500','1793866800').replace('Accept by08:15UTC','Accept by08:20UTC').replace('retainedFamilyBeforeRoundBytes=475840002','retainedFamilyBeforeRoundBytes=492617218')
# Both wrapper inputs are included in the actual freeze.
code=code.replace("'author.py','controller08.py','controller.py'","'author.py','controller08.py','controller07.py','controller.py'")
code=code.replace("'source-control08.py','source-control05.py'","'source-control08.py','source-control07.py','source-control05.py'")
v3="""  for filename,expected in [('producer-input-v3.json','4a41acdb1f331c668e2a9b017d1627d0aa970258aa2c5abbae893b2baa9e1f0d'),('producer-contract-v3.json','cc12d028b4615246ea457722d13af6d35fc505e44b6d1946adcf1811a37ab98e'),('producer-ready-v3.json','6355a2556e6dd8ff7e5ab2519985026a5ac3b675f951842921cb9aeeb3d5a68a')]:
   value=subprocess.check_output(['/usr/bin/git','show','3d256d4216327a59dce96e550e4feb355e55b18a:docs/mcr/evidence/pi08-finish-01/photo2-world-contract/'+filename],cwd=ROOT);assert control.sha(value)==expected,('Actual published v3 READY source drift',filename)
"""
code=code.replace('exec(compile(c,str(Path(__file__).resolve()),\'exec\'))',"c=c.replace('  paths=[',v3+'  paths=[')\nexec(compile(c,str(Path(__file__).resolve()),'exec'))")
exec(compile(code,str(Path(__file__).resolve()),'exec'))
