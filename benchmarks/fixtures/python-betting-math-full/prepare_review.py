"""Create a complete separate copy with added public-contract checks."""
import difflib
from pathlib import Path
import shutil
import sys
root=Path(sys.argv[1]);strong=Path(sys.argv[2]);here=Path(__file__).resolve().parent
shutil.copytree(root,strong,ignore=shutil.ignore_patterns('.git'))
shutil.copy2(here/'test_public_contracts.py',strong/'tests/test_public_contracts.py')
shutil.copy2(here.parent/'python-betting-math/test_metric_boundaries.py',strong/'tests/test_metric_boundaries.py')
p=strong/'tests/test_kelly.py';old=p.read_text();new=old
for value in ('0.0','1.0'):
    before=f'        assert full_kelly_fraction({value}, 5.0) == 0.0'
    after=(f'        try:\n            actual = full_kelly_fraction({value}, 5.0)\n'
           '        except InvalidProbabilityError as error:\n'
           "            pytest.fail(f'valid endpoint was rejected: {error}')\n"
           '        assert actual == 0.0')
    assert new.count(before)==1
    new=new.replace(before,after)
p.write_text(new)
(strong.parent/'kelly-tests.patch').write_text(''.join(difflib.unified_diff(old.splitlines(True),new.splitlines(True),fromfile='a/tests/test_kelly.py',tofile='b/tests/test_kelly.py')))
