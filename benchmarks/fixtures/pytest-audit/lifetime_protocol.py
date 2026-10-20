"""Strict positive-evidence reader for the native lifetime experiment."""
import json

from audit import classify as classify_phases

MAX_EVIDENCE = 4 * 1024 * 1024
FIELDS = {
    "lifetime_start": set(),
    "start": {"python", "pytest", "root"},
    "collection": {"nodes"},
    "phase": {"node", "when", "outcome", "exception", "assertion", "xfail"},
    "finish": {"status"},
    "returned": {"status", "imports"},
    "checkpoint": {"stage", "hooks_intact", "alive_threads"},
}


def unique_object(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError("duplicate key")
        result[key] = value
    return result


def decode(raw):
    if not raw or len(raw) > MAX_EVIDENCE or not raw.endswith(b"\n"):
        raise ValueError("missing, oversized or incomplete evidence")
    def reject_constant(value):
        raise ValueError("non-JSON numeric constant")

    return [json.loads(line, object_pairs_hook=unique_object, parse_constant=reject_constant)
            for line in raw.decode("utf-8").splitlines()]


def classify(raw, code, *, pid, nonce, stderr=b"", descendants=False):
    # Consult only after wait/reap and process-group cleanup. Any uncaptured
    # stderr is ambiguous in this initial scope, even after the atexit checkpoint.
    if stderr or descendants or type(code) is not int or code not in (0, 1):
        return "error"
    try:
        events = decode(raw)
        for index, e in enumerate(events):
            if (type(e) is not dict or type(e.get("seq")) is not int
                    or e["seq"] != index or type(e.get("pid")) is not int
                    or e["pid"] != pid or e.get("nonce") != nonce):
                return "error"
        kinds = [e.get("event") for e in events]
        if any(type(k) is not str or k not in FIELDS for k in kinds):
            return "error"
        if any(set(e) != FIELDS[e["event"]] | {"seq", "pid", "nonce", "event"}
               for e in events):
            return "error"
        if kinds[:3] != ["lifetime_start", "start", "collection"]:
            return "error"
        if kinds[-4:] != ["finish", "returned", "checkpoint", "checkpoint"]:
            return "error"
        if kinds.count("lifetime_start") != 1 or kinds.count("checkpoint") != 2:
            return "error"
        checkpoints = events[-2:]
        if [e.get("stage") for e in checkpoints] != ["returned", "atexit"]:
            return "error"
        if any(e.get("hooks_intact") is not True or e.get("alive_threads") != []
               for e in checkpoints):
            return "error"
        for e in events:
            kind = e["event"]
            if kind == "start":
                if (type(e["python"]) is not str or not e["python"].startswith("3.14.7 ")
                        or e["pytest"] != "9.0.2" or type(e["root"]) is not str):
                    return "error"
            elif kind == "collection":
                if type(e.get("nodes")) is not list or any(
                        type(n) is not str or not n for n in e["nodes"]):
                    return "error"
            elif kind == "phase":
                if (type(e.get("node")) is not str or type(e.get("xfail")) is not bool
                        or type(e.get("assertion")) is not bool):
                    return "error"
                if e.get("outcome") == "failed" and not e["assertion"]:
                    return "error"
                if e.get("outcome") == "passed" and (
                        e["assertion"] or e.get("exception") is not None):
                    return "error"
            elif kind in ("finish", "returned"):
                if type(e.get("status")) is not int:
                    return "error"
                if kind == "returned" and (type(e["imports"]) is not dict or any(
                        type(k) is not str or type(v) is not str
                        for k, v in e["imports"].items())):
                    return "error"
        phases = [e for e in events if e["event"] not in ("lifetime_start", "checkpoint")]
        return classify_phases(phases, code)
    except (ValueError, TypeError, KeyError, UnicodeError):
        return "error"
