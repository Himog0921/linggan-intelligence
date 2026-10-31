#!/usr/bin/env python3
"""Bounded metadata validation only. No shell execution, database connection or service action."""
import json
import os
from pathlib import Path
import plistlib
import re
import sys
import tempfile
import uuid

sys.dont_write_bytecode = True
from ai_snapshot import query

LABELS = tuple("com.linggan-intelligence." + name for name in
               ("local-runtime", "patrol-worker", "media-worker", "ai-executor"))


def need(condition, code="ai_drain_metadata_invalid"):
    if not condition:
        raise ValueError(code)


def shape(value, keys):
    need(type(value) is dict and set(value) == set(keys))


def integer(value):
    return type(value) is int and value >= 0


def full_revision(value):
    need(type(value) is str and re.fullmatch(r"[0-9a-f]{40}", value) is not None)
    return value


def instance(value):
    shape(value, ("instanceRef", "generation", "processId", "sourceRevision", "state"))
    need(type(value["instanceRef"]) is str and str(uuid.UUID(value["instanceRef"])) == value["instanceRef"])
    need(type(value["generation"]) is str and re.fullmatch(r"[1-9][0-9]{0,18}", value["generation"]) is not None)
    need(int(value["generation"]) <= 9223372036854775807)
    need(integer(value["processId"]) and value["processId"] > 0)
    need(type(value["sourceRevision"]) is str and 0 < len(value["sourceRevision"]) <= 128)
    need(value["state"] in ("running", "draining", "drained", "lost"))


def snapshot(value):
    shape(value, ("contract", "databaseKey", "runningInvocations", "runningItems", "sessionLockHeld", "executor"))
    need(value["contract"] == "ai-drain-snapshot.v1")
    need(type(value["databaseKey"]) is str and 0 < len(value["databaseKey"]) <= 256)
    need(integer(value["runningInvocations"]) and integer(value["runningItems"]))
    need(type(value["sessionLockHeld"]) is bool)
    if value["executor"] is not None:
        instance(value["executor"])
    need(value["runningInvocations"] == 0 and value["runningItems"] == 0
         and not value["sessionLockHeld"], "ai_drain_not_settled")
    return value


def ack_row(value, pid, db):
    snapshot(db)
    if type(value) is dict and value.get("state") == "failed":
        raise ValueError("ai_drain_failed")
    shape(value, ("state", "processId", "receipt"))
    need(value["state"] == "drained" and type(value["processId"]) is int and value["processId"] == pid)
    row = value["receipt"]
    shape(row, ("instanceRef", "generation", "processId", "sourceRevision"))
    instance(dict(row, state="drained"))
    need(row["processId"] == pid, "ai_drain_identity_mismatch")
    need(db["executor"] == dict(row, state="drained"), "ai_drain_instance_mismatch")
    return row


def ack(value, pid, revision, db):
    row = ack_row(value, pid, db)
    need(row["sourceRevision"] == full_revision(revision), "ai_drain_identity_mismatch")
    return {"state": "drained", "databaseKey": db["databaseKey"], "ack": value}


def no_ai(db):
    snapshot(db)
    return {"state": "not_running", "databaseKey": db["databaseKey"]}


def proof(value, revision, db):
    if type(value) is dict and value.get("state") == "drained":
        shape(value, ("state", "databaseKey", "ack"))
        need(value["databaseKey"] == db["databaseKey"], "ai_drain_database_mismatch")
        ack(value["ack"], value["ack"]["processId"], revision, db)
    else:
        shape(value, ("state", "databaseKey"))
        need(value == no_ai(db), "ai_drain_database_mismatch")


def disabled(text):
    need("{" in text and text.rstrip().endswith("}"), "launchd_disabled_unreadable")
    need(text.split("{", 1)[0].strip() in ("", "disabled services ="), "launchd_disabled_unreadable")
    body = text.split("{", 1)[1].rsplit("}", 1)[0]
    result = {label: label == LABELS[-1] for label in LABELS}
    seen = set()
    for line in body.splitlines():
        if not line.strip():
            continue
        match = re.fullmatch(r'\s*"([^"\n]+)"\s*=>\s*(true|false|enabled|disabled)\s*,?\s*', line)
        need(match is not None, "launchd_disabled_unreadable")
        name, flag = match.groups()
        need(name not in seen, "launchd_disabled_unreadable")
        seen.add(name)
        if name in result:
            result[name] = flag in ("true", "disabled")
    return result


def disabled_shape(value):
    shape(value, LABELS)
    need(all(type(flag) is bool for flag in value.values()))


def grace(paths):
    values = []
    for position, name in enumerate(paths):
        if position and not Path(name).exists():
            continue  # the pre-AI runtime has no resource contract
        value = read(name)
        need(value.get("contract") == "ai-resource-limits.v1", "ai_resource_contract_invalid")
        bounds = [value.get(key) for key in ("maxTimeoutSeconds", "adapterDeadlineAllowanceSeconds", "receiptFinalizationAllowanceSeconds")]
        need(all(integer(n) and 0 < n <= 86400 for n in bounds), "ai_resource_contract_invalid")
        values.append(sum(bounds))
    need(bool(values), "ai_resource_contract_invalid")
    return max(values)


def pairs(entries):
    result = {}
    for key, value in entries:
        need(key not in result)
        result[key] = value
    return result


def loads(text):
    need(len(text.encode("utf-8")) <= 65536)
    return json.loads(text, object_pairs_hook=pairs)


def read(name):
    file = Path(name)
    need(not file.is_symlink() and file.is_file() and file.stat().st_size <= 65536)
    return loads(file.read_text(encoding="utf-8"))


def atomic_bytes(name, data):
    file = Path(name)
    file.parent.mkdir(parents=True, exist_ok=True)
    fd, temp = tempfile.mkstemp(prefix=".ai-update-", dir=str(file.parent))
    try:
        with os.fdopen(fd, "wb") as output:
            output.write(data)
            output.flush()
            os.fsync(output.fileno())
        os.replace(temp, file)
        directory = os.open(str(file.parent), os.O_RDONLY)
        try:
            os.fsync(directory)
        finally:
            os.close(directory)
    finally:
        if os.path.exists(temp):
            os.unlink(temp)


def atomic(name, value):
    atomic_bytes(name, (json.dumps(value, ensure_ascii=False) + "\n").encode("utf-8"))


def plist(args):
    name, label, binary, runtime, support, logs, logname, grace_seconds, port, docker_cli = args
    need(label in LABELS and binary == dict(zip(LABELS, (
        "linggan-api", "linggan-worker", "linggan-media-worker", "linggan-ai-worker")))[label])
    need(Path(docker_cli).is_absolute() and Path(docker_cli).name == "docker"
         and Path(docker_cli).is_file() and os.access(docker_cli, os.X_OK))
    # launchd's default PATH omits common Docker CLI locations. Keep only the verified CLI
    # directory plus system tools, so the pre-start migration check can actually query its ledger.
    launch_path = ":".join(dict.fromkeys((str(Path(docker_cli).parent),
                                         "/usr/bin", "/bin", "/usr/sbin", "/sbin")))
    environment = {"LINGGAN_RUNTIME_DIR": runtime, "LINGGAN_SUPPORT_DIR": support,
                   "PATH": launch_path}
    if binary == "linggan-worker":
        environment["LINGGAN_WORKER_DRAIN_ACK_PATH"] = str(Path(support) / "runtime-drain/worker-drain-ack")
    if binary == "linggan-ai-worker":
        environment["LINGGAN_AI_DRAIN_ACK_PATH"] = str(Path(support) / "runtime-drain/ai-drain-ack")
    if port:
        need(binary == "linggan-api" and re.fullmatch(r"[1-9][0-9]{0,4}", port) is not None
             and int(port) <= 65535)
        environment["LINGGAN_LOCAL_PORT"] = port
    value = {"Label": label, "ProgramArguments": ["/bin/zsh", str(Path(runtime) / "scripts/runtime/launch.sh"), binary],
             "WorkingDirectory": runtime, "EnvironmentVariables": environment, "RunAtLoad": True, "KeepAlive": True,
             "ThrottleInterval": 30, "StandardOutPath": str(Path(logs) / (logname + ".out.log")),
             "StandardErrorPath": str(Path(logs) / (logname + ".err.log"))}
    if binary in ("linggan-worker", "linggan-ai-worker"):
        need(re.fullmatch("[1-9][0-9]{0,5}", grace_seconds) is not None)
        value["ExitTimeOut"] = int(grace_seconds)
    atomic_bytes(name, plistlib.dumps(value))


def main(args):
    mode, *args = args
    if mode == "grace":
        print(grace(args))
    elif mode == "snapshot-query":
        print(query(*args))
    elif mode == "disabled":
        atomic(args[1], disabled(Path(args[0]).read_text(encoding="utf-8")))
    elif mode == "disabled-value":
        value = read(args[0]); disabled_shape(value)
        print("true" if value[args[1]] else "false")
    elif mode == "pin":
        atomic_bytes(args[0], (full_revision(args[1]) + "\n").encode("ascii"))
    elif mode == "plist":
        plist(args)
    elif mode == "process-gone":
        pid = int(args[0]); need(1 < pid <= 2147483647)
        try:
            os.kill(pid, 0)
        except ProcessLookupError:
            return
        raise ValueError("runtime_process_still_running")
    elif mode == "check-ack":
        print(json.dumps(ack(read(args[0]), int(args[1]), args[2], loads(sys.stdin.read(65537)))))
    elif mode == "check-local-ack":
        value = read(args[0]); db = loads(sys.stdin.read(65537))
        ack_row(value, int(args[1]), db)
        # Stopping an owned development child can bind its unknown/working-tree identity.
        # This proof type is deliberately rejected by release permits.
        print(json.dumps({"state": "local_drained", "databaseKey": db["databaseKey"], "ack": value}))
    elif mode == "check-patrol":
        file = Path(args[0])
        need(not file.is_symlink() and file.is_file() and file.stat().st_size <= 256)
        text = file.read_text(encoding="utf-8")
        rows = [row.split("=", 1) for row in text.splitlines()]
        need(all(len(row) == 2 for row in rows) and len(rows) == 2)
        value = pairs(rows); shape(value, ("pid", "state"))
        need(value["pid"] == str(int(args[1])) and value["state"] == "drained", "patrol_drain_failed")
    elif mode == "no-ai":
        print(json.dumps(no_ai(loads(sys.stdin.read(65537)))))
    elif mode == "permit":
        name, before, after, patrol, ai_file, disabled_file = args
        full_revision(before); full_revision(after)
        need(patrol in ("drained", "not_running", "no_inflight_bootstrap"))
        db = snapshot(loads(sys.stdin.read(65537)))
        ai = read(ai_file); proof(ai, before, db)
        flags = read(disabled_file); disabled_shape(flags)
        atomic(name, {"contract": "linggan-runtime-update.v1", "from": before, "target": after,
                      "patrolState": patrol, "ai": ai, "databaseKey": db["databaseKey"], "disabled": flags})
    elif mode == "check-permit":
        value = read(args[0]); before, after = args[1:]
        shape(value, ("contract", "from", "target", "patrolState", "ai", "databaseKey", "disabled"))
        need(value["contract"] == "linggan-runtime-update.v1" and value["from"] == full_revision(before)
             and value["target"] == full_revision(after), "runtime_permit_revision_mismatch")
        need(value["patrolState"] in ("drained", "not_running", "no_inflight_bootstrap"))
        disabled_shape(value["disabled"])
        db = snapshot(loads(sys.stdin.read(65537)))
        need(value["databaseKey"] == db["databaseKey"], "ai_drain_database_mismatch")
        proof(value["ai"], before, db)
    else:
        raise ValueError("ai_guard_command_invalid")


if __name__ == "__main__":
    try:
        main(sys.argv[1:])
    except (ValueError, OSError, KeyError, TypeError, IndexError) as error:
        code = str(error)
        print(code if re.fullmatch("[a-z_]{1,80}", code) else "ai_drain_metadata_invalid", file=sys.stderr)
        sys.exit(1)
