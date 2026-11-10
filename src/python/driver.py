"""Pinned pytest observer. Parent must read evidence after process-group shutdown."""
import atexit
import hashlib
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

    thread_sequence = 0
    start_lock = threading.RLock()
    starting = threading.local()

    def lifetime_audit(event, args):
        # Pytest temporarily replaces these hooks. The audit event remains
        # independent of that replacement, including late teardown callbacks.
        if event in {"sys.unraisablehook", "sys.excepthook"}:
            emit("lifetime_error", kind=event)
        elif event in {"_thread.start_new_thread", "_thread.start_joinable_thread"}:
            if not getattr(starting, "active", False):
                emit("unsupported", kind="unobserved_thread_start")

    sys.addaudithook(lifetime_audit)

    def start(thread, *args, **kwargs):
        nonlocal thread_sequence
        with start_lock:
            token = thread_sequence
            thread_sequence += 1
            emit("thread", token=token, stage="requested")
        original_run = thread.run
        def run():
            emit("thread", token=token, stage="running")
            try:
                return original_run()
            except BaseException:
                emit("lifetime_error", kind="thread_body")
                raise
            finally:
                emit("thread", token=token, stage="finished")
        thread.run = run
        try:
            starting.active = True
            return original_start(thread, *args, **kwargs)
        except BaseException:
            thread.run = original_run
            emit("thread", token=token, stage="start_failed")
            raise
        finally:
            starting.active = False

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


def install_project_io_guard(root, temporary, emit, installed_root=None):
    """Constrain I/O originating in audited project frames, including callees.

    This complements static admission and post-run hashes; it is not a general
    Python sandbox. Interpreter imports may read the pinned runtime. Pytest's
    own bookkeeping outside project calls retains its normal behavior.
    """
    root = os.path.realpath(root)
    temporary = os.path.realpath(temporary)
    home = os.path.realpath(os.environ["HOME"])
    installed_root = os.path.realpath(installed_root) if installed_root else None
    import stat
    import fcntl
    # Imports may probe the runner's fresh bytecode prefix even with -B. This
    # prefix is owned by the command and never reused; writing remains forbidden.
    runtime = tuple(os.path.realpath(p) for p in
                    (sys.prefix, sys.base_prefix, sys.pycache_prefix) if p)
    realpath, fspath, frame = os.path.realpath, os.fspath, sys._getframe
    write_flags = os.O_WRONLY | os.O_RDWR | os.O_CREAT | os.O_TRUNC | os.O_APPEND

    def inside(path, directory):
        return path == directory or path.startswith(directory + os.sep)

    def project_call():
        current = frame()
        while current is not None:
            filename = current.f_code.co_filename
            if filename.startswith(root + os.sep) or (installed_root and filename.startswith(installed_root + os.sep)):
                return True
            current = current.f_back
        return False

    def reject():
        # Retain evidence even when a test catches the exception.
        emit("unsupported", kind="project_file_access")
        raise PermissionError("Python project file access escaped audited inputs/temporary output")

    def descriptor_path(fd):
        if sys.platform == "darwin":
            return os.fsdecode(fcntl.fcntl(fd, 50, bytes(1024)).split(b"\0", 1)[0])
        return os.readlink(f"/proc/self/fd/{fd}").removesuffix(" (deleted)")

    def check(path, write=False, directory=-1):
        if isinstance(path, int):
            if stat.S_ISFIFO(os.fstat(path).st_mode):
                return
            try:
                path = descriptor_path(path)
            except OSError:
                reject()
        elif directory not in (-1, None) and not os.path.isabs(path):
            path = os.path.join(descriptor_path(directory), path)
        path = realpath(fspath(path))
        if isinstance(path, bytes):
            reject()
        if inside(path, root) or (installed_root and inside(path, installed_root)):
            if write:
                reject()
            return
        if inside(path, temporary) or inside(path, home):
            return
        if not write and any(inside(path, p) for p in runtime):
            return
        reject()

    # CPython's open audit event omits dir_fd. Preserve the native call while
    # carrying that argument to the hook, so a relative open cannot be checked
    # against cwd and then written through a different directory descriptor.
    original_open = os.open
    opening = threading.local()

    def guarded_open(path, flags, mode=0o777, *, dir_fd=None):
        previous = getattr(opening, "directory", None)
        opening.directory = dir_fd
        try:
            return original_open(path, flags, mode, dir_fd=dir_fd)
        finally:
            opening.directory = previous

    os.open = guarded_open

    def audit(event, args):
        if event not in {"open", "os.listdir", "os.scandir", "os.remove", "os.rmdir",
                         "os.mkdir", "os.rename", "os.link", "os.symlink", "os.chmod",
                         "os.chown", "os.utime", "os.truncate"} or not project_call():
            return
        if event == "open":
            path, mode, flags = args
            # The null device is an empty input/discarded output, not host data.
            # This exception applies to opening only, never filesystem mutation.
            if isinstance(path, str) and realpath(path) == os.devnull:
                return
            check(path, bool(flags & write_flags), getattr(opening, "directory", None))
        elif event in {"os.listdir", "os.scandir"}:
            check(args[0])
        elif event == "os.symlink":
            target, destination, directory = args
            if directory not in (-1, None) and not os.path.isabs(destination):
                destination = os.path.join(descriptor_path(directory), destination)
            if not os.path.isabs(target):
                target = os.path.join(os.path.dirname(os.path.abspath(destination)), target)
            check(target)
            check(destination, True)
        elif event in {"os.rename", "os.link"}:
            check(args[0], True, args[2])
            check(args[1], True, args[3])
        elif event in {"os.remove", "os.rmdir"}:
            check(args[0], True, args[1])
        elif event in {"os.mkdir", "os.chmod"}:
            check(args[0], True, args[2])
        elif event in {"os.chown", "os.utime"}:
            check(args[0], True, args[3])
        else:
            check(args[0], True)

    sys.addaudithook(audit)
    return lambda: os.open is guarded_open


root, evidence, config, nonce, source_root, package, *selection = sys.argv[1:]
root = Path(root).resolve()
os.chdir(root)
os.environ["PROFILER_ITERATIONS"] = "1"
if source_root not in ("", "src") or not package.isidentifier():
    raise RuntimeError("invalid audited Python package root")
import_root = root / source_root
receipt_path = os.environ.get("MUTIE_PYTHON_INSTALLATION", "")
receipt_bytes = Path(receipt_path).read_bytes() if receipt_path else None
receipt = json.loads(receipt_bytes) if receipt_bytes else None
scope_path = os.environ.get("MUTIE_PYTHON_SCOPE", "")
scope_bytes = Path(scope_path).read_bytes() if scope_path else None
scope = json.loads(scope_bytes) if scope_bytes else None
if receipt:
    import_root = Path(receipt["site"]).resolve(strict=True)
    if receipt["package"] != package or scope["package"] != package:
        raise RuntimeError("installed Python package identity mismatch")
    if not sys.pycache_prefix:
        raise RuntimeError("installed Python requires a fresh bytecode prefix")
    for name in ("PYTHONPATH", "PYTHONHOME", "PYTHONSTARTUP", "PYTHONUSERBASE"):
        os.environ.pop(name, None)
    os.environ["PYTHONNOUSERSITE"] = "1"
    os.environ["PYTHONDONTWRITEBYTECODE"] = "1"
    os.environ["PYTHONPYCACHEPREFIX"] = sys.pycache_prefix


def validate_installation():
    if not receipt:
        return
    if Path(receipt_path).read_bytes() != receipt_bytes or Path(scope_path).read_bytes() != scope_bytes:
        raise RuntimeError("Python installation receipt changed")
    observed = {}
    for path in import_root.rglob("*"):
        if path.is_symlink():
            raise RuntimeError("installed Python symlink")
        if path.is_file():
            observed[path.relative_to(import_root).as_posix()] = "sha256:" + hashlib.sha256(path.read_bytes()).hexdigest()
    if observed != receipt["files"]:
        raise RuntimeError("installed Python package changed")


validate_installation()
package_root = import_root / package
if not (package_root / "__init__.py").is_file():
    raise RuntimeError("missing audited Python package")
sys.path.insert(0, str(import_root))
emit = make_writer(evidence, nonce)
checkpoint = install_lifetime(emit)
emit("lifetime_start")

import pytest  # noqa: E402: lifecycle observation precedes third-party imports.

io_intact = install_project_io_guard(root, os.environ["TMPDIR"], emit, import_root if receipt else None)

if receipt:
    allowed_executables = {str(root / path) for path in scope["tests"] + scope["conftests"]}
    allowed_executables.update(str(root / path) for path in scope["installation"]["ancillary"]
                               if Path(path).name == "__init__.py" and path.startswith("tests/"))
    allowed_executables.update(str(import_root / path) for path in receipt["files"] if path.endswith(".py"))

    def installed_audit(event, args):
        if event == "import" and args[0].split(".")[0] in {"ctypes", "msvcrt", "typing_extensions", "_typeshed"}:
            emit("unsupported", kind="inactive_dependency_import")
            raise ImportError("inactive dependency cannot execute")
        if event == "exec":
            filename = args[0].co_filename
            if (filename.startswith(str(root) + os.sep) or filename.startswith(str(import_root) + os.sep)) and filename not in allowed_executables:
                emit("unsupported", kind="unaudited_project_execution")
                raise ImportError("project execution escaped installed source/tests")

    sys.addaudithook(installed_audit)

if sys.version_info[:3] != (3, 14, 7) or pytest.__version__ != "9.0.2":
    raise RuntimeError("runner requires CPython 3.14.7 / pytest 9.0.2")


# unittest queues setup/body/cleanup exceptions in pytest's call phase. Observe
# the pinned dispatchers before project imports and retain exact body exceptions.
case_type = unittest.TestCase
protected = ("run", "__call__", "__init__", "__getattribute__", "__setattr__", "__delattr__",
             "_callSetUp", "_callTestMethod", "_callTearDown", "_callCleanup",
             "doCleanups", "doClassCleanups", "failureException", "subTest",
             "addCleanup", "addClassCleanup", "enterContext", "enterClassContext",
             "defaultTestResult", "_addUnexpectedSuccess")
original_case = {name: getattr(case_type, name) for name in protected}
active_case = None
unit_results = {}
unit_nodes = set()


def observe_dispatch(name, stage):
    original = original_case[name]

    def wrapped(self, *args, **kwargs):
        state = active_case
        if state is None or state["instance"] is not self:
            emit("unsupported", kind="unittest_dispatch_outside_test")
            return original(self, *args, **kwargs)
        if stage == "cleanups":
            state["steps"].append(dict(stage="cleanups_begin", outcome="passed", assertion=False))
        outcome, assertion = "passed", False
        try:
            return original(self, *args, **kwargs)
        except BaseException as error:
            outcome = "failed"
            assertion = isinstance(error, (AssertionError, pytest.fail.Exception))
            if stage == "body":
                state["body_exception"] = error
            raise
        finally:
            state["steps"].append(dict(stage="cleanups_end" if stage == "cleanups" else stage,
                                       outcome=outcome, assertion=assertion))

    return wrapped


for name, stage in (("_callSetUp", "setup"), ("_callTestMethod", "body"),
                    ("_callTearDown", "teardown"), ("_callCleanup", "cleanup"),
                    ("doCleanups", "cleanups")):
    setattr(case_type, name, observe_dispatch(name, stage))


def initialize_case(self, *args, **kwargs):
    if active_case is not None:
        emit("unsupported", kind="unittest_initialization_during_call")
    return original_case["__init__"](self, *args, **kwargs)


case_type.__init__ = initialize_case
expected_case = {name: getattr(case_type, name) for name in protected}

# Fixture setup can occur inside a call via getfixturevalue, and its exception
# can even be caught by the body. Observe independently of pytest phase reports.
from _pytest.fixtures import FixtureDef

fixture_finish = FixtureDef.finish
fixtures = {}
fixture_sequence = 0


def finish_fixture(self, request):
    token = fixtures.get(self)
    try:
        return fixture_finish(self, request)
    except BaseException:
        emit("fixture_error", stage="finalizer", name=self.argname)
        raise
    finally:
        if token is not None:
            if fixtures.pop(self, None) != token:
                emit("fixture_error", stage="identity", name=self.argname)
            emit("fixture", token=token, stage="finished")


FixtureDef.finish = finish_fixture


def case_intact(cls, instance=None):
    def function(value):
        return getattr(value, "__func__", value)
    return (cls.__bases__ == (case_type,)
            and type(cls) is type
            and all(function(getattr(case_type, name)) is function(expected_case[name])
                    and function(getattr(cls, name)) is function(expected_case[name])
                    for name in protected)
            and not any(name in cls.__dict__ for name in ("__getattr__", "__del__"))
            and (instance is None or not any(name in instance.__dict__ for name in protected)))


class Observer:
    def __init__(self):
        self.deselected = []

    def pytest_deselected(self, items):
        self.deselected.extend(item.nodeid for item in items)

    @pytest.hookimpl(hookwrapper=True, tryfirst=True)
    def pytest_fixture_setup(self, fixturedef, request):
        global fixture_sequence
        token = fixture_sequence
        fixture_sequence += 1
        if fixturedef in fixtures:
            emit("fixture_error", stage="duplicate", name=fixturedef.argname)
        fixtures[fixturedef] = token
        emit("fixture", token=token, stage="started")
        result = yield
        if result.excinfo is not None:
            emit("fixture_error", stage="setup", name=fixturedef.argname)
        else:
            emit("fixture", token=token, stage="ready")

    def pytest_collection_finish(self, session):
        for item in session.items:
            cls = getattr(item, "cls", None)
            if isinstance(cls, type) and issubclass(cls, unittest.TestCase):
                if not case_intact(cls):
                    emit("unsupported", kind="unittest_dispatch_override", node=item.nodeid)
                unit_nodes.add(item.nodeid)
        loaded = []
        for plugin in session.config.pluginmanager._conftest_plugins:
            path = Path(plugin.__file__).resolve()
            if not path.is_relative_to(root):
                emit("fixture_error", stage="external_conftest")
            else:
                loaded.append(path.relative_to(root).as_posix())
        deselected = None
        if self.deselected:
            if len(set(self.deselected)) != len(self.deselected):
                emit("unsupported", kind="duplicate_deselected_test")
            digest = hashlib.sha256()
            for node in self.deselected:
                value = node.encode()
                digest.update(len(value).to_bytes(8, "big"))
                digest.update(value)
            deselected = dict(count=len(self.deselected), fingerprint="sha256:" + digest.hexdigest(),
                              files=sorted({node.split("::", 1)[0] for node in self.deselected}))
        emit("collection", nodes=[item.nodeid for item in session.items], unittest=sorted(unit_nodes),
             conftests=sorted(loaded), deselected=deselected)

    @pytest.hookimpl(hookwrapper=True, tryfirst=True)
    def pytest_runtest_call(self, item):
        global active_case
        if item.nodeid not in unit_nodes:
            yield
            return
        instance = item.instance
        state = dict(instance=instance, steps=[], body_exception=None,
                     intact=active_case is None and case_intact(type(instance), instance))
        active_case = state
        try:
            yield
        finally:
            state["intact"] = state["intact"] and case_intact(type(instance), instance)
            unit_results[item.nodeid] = state
            active_case = None

    def pytest_collectreport(self, report):
        if report.failed:
            emit("collection_error", node=report.nodeid)

    @pytest.hookimpl(hookwrapper=True)
    def pytest_runtest_makereport(self, item, call):
        result = yield
        report = result.get_result()
        # A same-named user exception is not positive assertion evidence.
        expected_failure = bool(call.when == "call" and call.excinfo
                                and type(call.excinfo.value) is pytest.xfail.Exception)
        assertion = bool(not expected_failure and call.excinfo and isinstance(
            call.excinfo.value, (AssertionError, pytest.fail.Exception)))
        # Only type/value/arithmetic/lookup errors are eligible for bounded
        # continuation. Resource and system failures remain fatal, including
        # multiply inherited application exceptions.
        candidate_exception = bool(not assertion and call.when == "call" and call.excinfo
            and isinstance(call.excinfo.value, (TypeError, ValueError, ArithmeticError, LookupError))
            and not isinstance(call.excinfo.value, (OSError, MemoryError, SystemError, RecursionError)))
        unit = None
        if call.when == "call" and item.nodeid in unit_nodes:
            state = unit_results.pop(item.nodeid, None)
            if state is not None:
                unit = dict(steps=state["steps"], intact=state["intact"],
                            body_matches=(call.excinfo.value if call.excinfo else None)
                            is state["body_exception"])
        emit("phase", node=report.nodeid, when=report.when,
             outcome=report.outcome,
             exception=call.excinfo.typename if call.excinfo else None,
             assertion=assertion, candidate_exception=candidate_exception,
             xfail=hasattr(report, "wasxfail"), expected_failure=expected_failure, unittest=unit,
             skip=bool(call.excinfo and type(call.excinfo.value) is pytest.skip.Exception),
             marked_xfail=bool(call.when == "call" and call.excinfo
                               and not expected_failure and hasattr(report, "wasxfail")
                               and report.skipped and item.get_closest_marker("xfail") is not None
                               and not isinstance(call.excinfo.value,
                                   (OSError, MemoryError, SystemError, RecursionError))))

    def pytest_warning_recorded(self, warning_message, when, nodeid, location):
        emit("warning", category=warning_message.category.__name__, node=nodeid)

    def pytest_internalerror(self, excrepr, excinfo):
        emit("internal_error")

    def pytest_sessionfinish(self, session, exitstatus):
        if not io_intact():
            emit("unsupported", kind="project_io_guard_changed")
        if fixtures or FixtureDef.finish is not finish_fixture:
            emit("fixture_error", stage="incomplete")
        emit("finish", status=int(exitstatus))


emit("start", python=sys.version, pytest=pytest.__version__, root=str(root))
status = pytest.main(
    ["-q", "-c", config, "--rootdir", str(root), "--import-mode=" + ("prepend" if receipt else "importlib"),
     "--confcutdir", str(root), "-p", "no:cacheprovider", *selection],
    plugins=[Observer()],
)
imports = {}
installed_imports = {}
for name, module in list(sys.modules.items()):
    filename = getattr(module, "__file__", None)
    if name == package or name.startswith(package + "."):
        relative = Path(*name.split("."))
        allowed = {import_root / relative.with_suffix(".py"),
                   import_root / relative / "__init__.py"}
        if not filename or Path(filename).resolve() not in allowed:
            raise RuntimeError("Python application import escaped audited package: " + name)
    if filename and Path(filename).resolve().is_relative_to(root):
        imports[name] = str(Path(filename).resolve())
    if receipt and filename and Path(filename).resolve().is_relative_to(import_root):
        # Relative installed names bind to the verified receipt, while project
        # imports retain their actual original-root locations.
        installed_imports[name] = Path(filename).resolve().relative_to(import_root).as_posix()
validate_installation()
emit("returned", status=int(status), imports=imports, installed_imports=installed_imports)
checkpoint("returned")
# Keep the raw evidence descriptor alive through interpreter teardown.
raise SystemExit(status)
