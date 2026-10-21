"""Create a complete test-only review copy without changing upstream files."""
from pathlib import Path
import shutil
import sys

original, stronger = map(Path, sys.argv[1:3])
shutil.copytree(original, stronger, ignore=shutil.ignore_patterns('.git'))
shutil.copy2(Path(__file__).with_name('test_numeric_contracts.py'),
             stronger/'tests/test_numeric_contracts.py')
