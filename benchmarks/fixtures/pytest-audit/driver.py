"""Direct pytest phase probe, not a production Mutie adapter or security boundary."""
import json
import os
from pathlib import Path
import sys

import pytest

root, evidence, config, *selection = sys.argv[1:]
root = Path(root).resolve()
os.chdir(root)
# -I excludes cwd/PYTHONPATH; deliberately admit only the owned project root.
sys.path.insert(0, str(root))
stream = open(evidence, "x", encoding="utf-8", buffering=1)


def emit(event, **fields):
    stream.write(json.dumps({"event": event, **fields}) + "\n")


class Observer:
    def pytest_collection_finish(self, session):
        emit("collection", nodes=[item.nodeid for item in session.items])

    def pytest_collectreport(self, report):
        if report.failed:
            emit("collection_error", node=report.nodeid)

    @pytest.hookimpl(hookwrapper=True)
    def pytest_runtest_makereport(self, item, call):
        result = yield
        report = result.get_result()
        emit(
            "phase", node=report.nodeid, when=report.when,
            outcome=report.outcome,
            exception=call.excinfo.typename if call.excinfo else None,
            xfail=hasattr(report, "wasxfail"),
        )

    def pytest_warning_recorded(self, warning_message, when, nodeid, location):
        emit("warning", category=warning_message.category.__name__, node=nodeid)

    def pytest_internalerror(self, excrepr, excinfo):
        emit("internal_error")

    def pytest_sessionfinish(self, session, exitstatus):
        emit("finish", status=int(exitstatus))


emit("start", python=sys.version, pytest=pytest.__version__, root=str(root))
status = pytest.main(
    ["-q", "-c", config, "--rootdir", str(root), "--import-mode=importlib",
     "--noconftest", "-p", "no:cacheprovider", *selection],
    plugins=[Observer()],
)
imports = {}
for name, module in list(sys.modules.items()):
    filename = getattr(module, "__file__", None)
    if filename and (name == "subject" or name == "inflection"):
        imports[name] = str(Path(filename).resolve())
emit("returned", status=int(status), imports=imports)
stream.close()
raise SystemExit(status)
