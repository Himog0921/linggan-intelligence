#!/usr/bin/env python3
"""Hermetic install transport. Every unsupported command fails; it never forwards a command."""
import json
import os
from pathlib import Path
import plistlib
import sys

state_file = Path(os.environ["AI_INSTALL_PROOF_STATE"])
state = json.loads(state_file.read_text())
tool = Path(sys.argv[0]).name
args = sys.argv[1:]
with Path(state["trace"]).open("a") as log:
    log.write(json.dumps([tool, *args], ensure_ascii=False) + "\n")


def save():
    state_file.write_text(json.dumps(state))


def fail():
    print("unsupported_proof_command", file=sys.stderr)
    sys.exit(97)


if tool == "git":
    cwd = str(Path.cwd())
    if args[:1] == ["-C"]:
        cwd, args = args[1], args[2:]
    if cwd not in (state["source"], state["runtime"]): fail()
    if args[:1] == ["fetch"]:
        sys.exit(0)
    if args[:1] == ["diff"]:
        sys.exit(1 if state.get("dirty") else 0)
    if args[:2] == ["remote", "get-url"]:
        print("synthetic://owned-install-proof"); sys.exit(0)
    if args[:1] == ["rev-parse"]:
        if args[-1] == "--is-inside-work-tree": print("true")
        elif args[-1] == "origin/main": print(state["target"])
        elif args[-1] == "HEAD":
            head = state["target"] if cwd == state["source"] else state["head"]
            print(head[:8] if "--short" in args else head)
        else: fail()
        sys.exit(0)
    if args[:1] == ["reset"] and args[-1] == state["target"]:
        state["head"] = state["target"]; save(); sys.exit(0)
    fail()
elif tool == "launchctl":
    if args[:1] == ["print-disabled"]:
        if state.get("disabled_unreadable"):
            print("disabled_read_failed", file=sys.stderr); sys.exit(2)
        print("disabled services = {")
        for label, flag in state["disabled"].items(): print(f' "{label}" => {str(flag).lower()}')
        print("}"); sys.exit(0)
    if args[:1] == ["print"]:
        label = args[-1].split("/", 2)[-1]
        if state.get("inspection_unreadable") == label:
            print("inspection_failed", file=sys.stderr); sys.exit(2)
        if not state["loaded"].get(label, False):
            print(f'Could not find service "{label}" in domain for user', file=sys.stderr); sys.exit(1)
        print(f"pid = {state['pid']}"); sys.exit(0)
    if args[:1] == ["bootout"]:
        label = args[-1].split("/", 2)[-1]
        if not state["loaded"].get(label, False): fail()
        state["loaded"][label] = False
        drain = Path(state["support"]) / "runtime-drain"; drain.mkdir(exist_ok=True)
        if label.endswith("patrol-worker"):
            (drain / "worker-drain-ack").write_text(f"pid={state['pid']}\nstate=drained\n")
        if label.endswith("ai-executor"):
            state["snapshot"]["sessionLockHeld"] = False
            state["snapshot"]["executor"]["state"] = "drained"
            row = {k: v for k, v in state["snapshot"]["executor"].items() if k != "state"}
            ack = {"state": "drained", "processId": state["pid"], "receipt": row}
            mode = state.get("fault")
            if mode == "ack_source": row["sourceRevision"] = "c" * 40
            if mode == "ack_generation": row["generation"] = "8"
            if mode == "failed_ack": ack = {"state": "failed", "processId": state["pid"], "reason": "synthetic_receipt_failed"}
            if mode == "inflight": state["snapshot"]["runningInvocations"] = 1
            (drain / "ai-drain-ack").write_text(json.dumps(ack))
        save(); sys.exit(0)
    if args[:1] == ["disable"]:
        state["disabled"][args[-1].split("/", 2)[-1]] = True; save(); sys.exit(0)
    if args[:1] == ["bootstrap"]:
        value = plistlib.loads(Path(args[-1]).read_bytes())
        label = value["Label"]
        if state["disabled"].get(label, False): fail()
        state["loaded"][label] = True; save(); sys.exit(0)
    fail()
elif tool == "docker":
    if args[:3] != ["exec", "--env", "PGPASSWORD"] or state["password"] in " ".join(args): fail()
    if os.environ.get("PGPASSWORD") != state["password"]: fail()
    if "default_transaction_read_only=on" not in os.environ.get("PGOPTIONS", "") or os.environ.get("PGCONNECT_TIMEOUT") != "5": fail()
    if state.get("fault") == "database": sys.exit(2)
    sql = args[-1]
    if "to_regclass('linggan_ai_executor_current')" in sql: print("1,1")
    elif "'ai-drain-snapshot.v1'" in sql: print(json.dumps(state["snapshot"]))
    elif "SELECT count(*) FROM linggan_model_invocation" in sql: print(state["snapshot"]["runningInvocations"])
    elif "SELECT migration_id FROM" in sql: print("0131_ai_gen1_provider\n0140_ai_gen1_writing")
    else: fail()
    sys.exit(0)
elif tool == "cargo":
    if args[:1] != ["build"]: fail()
    for binary in ("linggan-api", "linggan-worker", "linggan-media-worker", "linggan-ai-worker"):
        file = Path(state["runtime"]) / "target/debug" / binary; file.parent.mkdir(parents=True, exist_ok=True)
        file.write_text("#!/bin/sh\nexit 0\n"); file.chmod(0o700)
    sys.exit(0)
elif tool == "plutil":
    if args[:1] != ["-lint"]: fail()
    plistlib.loads(Path(args[-1]).read_bytes()); sys.exit(0)
elif tool == "pgrep":
    if args[:1] != ["-x"]: fail()
    sys.exit(2 if state.get("process_probe_unreadable") else 1)
elif tool == "lsof":
    if state.get("port_probe_unreadable"): print("synthetic_inspection_failed", file=sys.stderr)
    sys.exit(1)
elif tool == "sleep":
    # A proof must never really wait for an hour or forward an unexpected wait.
    fail()
else:
    fail()
