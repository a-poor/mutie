"""Pinned pytest observer. Parent must read evidence after process-group shutdown."""
import atexit
import json
import os
from pathlib import Path
import sys
import threading
import unittest


def make_writer(path, nonce):
    descriptor = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_CLOEXEC, 0o600)
    write, encode, getpid = os.write, json.dumps, os.getpid
    lock = threading.RLock()
    sequence = 0

    def emit(event, **fields):
        nonlocal sequence
        # Capture dependencies in the closure so late unraisable callbacks can
        # still write after the driver's module globals start being cleared.
        try:
            with lock:
                data = (encode(dict(seq=sequence, pid=getpid(), nonce=nonce,
                                    event=event, **fields)) + "\n").encode()
                while data:
                    count = write(descriptor, data)
                    if count <= 0:
                        raise OSError("short evidence write")
                    data = data[count:]
                sequence += 1
        except BaseException:
            # A broken observer must never silently produce positive evidence.
            write(2, b"mutie observer evidence write failed\n")

    return emit


def install_lifetime(emit):
    original_unraisable = sys.unraisablehook
    original_thread = threading.excepthook
    original_exception = sys.excepthook
    original_start = threading.Thread.start
    system, threads = sys, threading
    main = threads.main_thread()

    def unraisable(args):
        emit("lifetime_error", kind="unraisable")
        original_unraisable(args)

    def thread_error(args):
        emit("lifetime_error", kind="thread")
        original_thread(args)

    def exception(kind, value, traceback):
        emit("lifetime_error", kind="uncaught")
        original_exception(kind, value, traceback)

    system.unraisablehook = unraisable
    threads.excepthook = thread_error
    system.excepthook = exception

    def start(thread, *args, **kwargs):
        # Pytest can retain a thread's exception after its last warning drain.
        # Until that attribution is audited, every started thread is unsupported.
        emit("unsupported", kind="thread_start")
        return original_start(thread, *args, **kwargs)

    threads.Thread.start = start

    def checkpoint(stage):
        intact = (system.unraisablehook is unraisable
                  and threads.excepthook is thread_error
                  and system.excepthook is exception
                  and threads.Thread.start is start)
        alive = [t.name for t in threads.enumerate() if t is not main and t.is_alive()]
        emit("checkpoint", stage=stage, hooks_intact=intact, alive_threads=alive)

    # Registered before pytest and application imports; ordinary later callbacks
    # run first. This is an atexit boundary, not the end of interpreter teardown.
    atexit.register(checkpoint, "atexit")
    return checkpoint


root, evidence, config, nonce, *selection = sys.argv[1:]
root = Path(root).resolve()
os.chdir(root)
sys.path.insert(0, str(root))
emit = make_writer(evidence, nonce)
checkpoint = install_lifetime(emit)
emit("lifetime_start")

import pytest  # noqa: E402: lifecycle observation precedes third-party imports.

if sys.version_info[:3] != (3, 14, 7) or pytest.__version__ != "9.0.2":
    raise RuntimeError("runner requires CPython 3.14.7 / pytest 9.0.2")


class Observer:
    def pytest_collection_finish(self, session):
        for item in session.items:
            cls = getattr(item, "cls", None)
            if isinstance(cls, type) and issubclass(cls, unittest.TestCase):
                emit("unsupported", kind="unittest", node=item.nodeid)
        emit("collection", nodes=[item.nodeid for item in session.items])

    def pytest_collectreport(self, report):
        if report.failed:
            emit("collection_error", node=report.nodeid)

    @pytest.hookimpl(hookwrapper=True)
    def pytest_runtest_makereport(self, item, call):
        result = yield
        report = result.get_result()
        # A same-named user exception is not positive assertion evidence.
        assertion = bool(call.excinfo and isinstance(
            call.excinfo.value, (AssertionError, pytest.fail.Exception)))
        emit("phase", node=report.nodeid, when=report.when,
             outcome=report.outcome,
             exception=call.excinfo.typename if call.excinfo else None,
             assertion=assertion, xfail=hasattr(report, "wasxfail"))

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
    if filename and Path(filename).resolve().is_relative_to(root):
        imports[name] = str(Path(filename).resolve())
emit("returned", status=int(status), imports=imports)
checkpoint("returned")
# Keep the raw evidence descriptor alive through interpreter teardown.
raise SystemExit(status)
