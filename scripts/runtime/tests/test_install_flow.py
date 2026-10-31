"""Execute copied release scripts against owned files and strict command doubles only."""
import json
import os
from pathlib import Path
import plistlib
import shutil
import subprocess
import sys
import unittest
import uuid

sys.dont_write_bytecode = True
SOURCE_ROOT = Path(__file__).resolve().parents[3]
LABELS = ["com.linggan-intelligence." + part for part in ("local-runtime", "patrol-worker", "media-worker", "ai-executor")]


class InstallFlow(unittest.TestCase):
    def setUp(self):
        self.root = SOURCE_ROOT / "artifacts/private/ai-gen1-pr1b" / ("install-proof." + str(uuid.uuid4()))
        self.root.mkdir(mode=0o700)
        self.source = self.root / "source"; self.runtime = self.root / "runtime & source"
        self.support = self.root / "support"; self.agents = self.root / "agents"; self.bin = self.root / "bin"
        for folder in (self.source, self.runtime, self.support, self.agents, self.bin, self.root / "tmp", self.root / "zsh"):
            folder.mkdir()
        for base in (self.source, self.runtime):
            folder = base / "scripts/runtime"; folder.mkdir(parents=True)
            for name in ("install.sh", "sync.sh", "launch.sh", "drain.zsh", "plist.zsh", "ai_guard.py", "ai_snapshot.py"):
                shutil.copy2(SOURCE_ROOT / "scripts/runtime" / name, folder / name)
            for name in ("prepare-web.sh", "prepare-pi-adapter.sh"):
                file = folder / name; file.write_text("#!/bin/sh\nexit 0\n"); file.chmod(0o700)
            resource = base / "crates/contracts/src/ai_limits.json"; resource.parent.mkdir(parents=True)
            shutil.copy2(SOURCE_ROOT / "crates/contracts/src/ai_limits.json", resource)
            migrations = base / "database/migrations"; migrations.mkdir(parents=True)
            for name in ("0131_ai_gen1_provider", "0140_ai_gen1_writing"): (migrations / (name + ".sql")).write_text("-- synthetic migration name only\n")
            (base / ".env").write_text("POSTGRES_USER=linggan_dev_admin\nPOSTGRES_PASSWORD=SYNTHETIC-NOT-A-SHARED-CREDENTIAL\nPOSTGRES_DB=synthetic_owned\nPOSTGRES_PORT=54321\nLINGGAN_LOCAL_PORT=3001\n")
            worker = base / "apps/worker/src/shutdown.rs"; worker.parent.mkdir(parents=True)
            worker.write_text("// synthetic marker: write_drain_ack\n")
        fixture = SOURCE_ROOT / "scripts/runtime/tests/fake_commands.py"
        for command in ("git", "launchctl", "docker", "cargo", "plutil", "pgrep", "lsof", "sleep"):
            file = self.bin / command; shutil.copy2(fixture, file); file.chmod(0o700)
        child = subprocess.Popen(["/usr/bin/true"]); pid = child.pid; child.wait()
        row = {"instanceRef": str(uuid.uuid4()), "generation": "7", "processId": pid, "sourceRevision": "a" * 40, "state": "running"}
        self.state = {"source": str(self.source), "runtime": str(self.runtime), "support": str(self.support),
                      "head": "a" * 40, "target": "b" * 40, "trace": str(self.root / "trace.jsonl"), "pid": pid,
                      "password": "SYNTHETIC-NOT-A-SHARED-CREDENTIAL", "loaded": {label: True for label in LABELS},
                      "disabled": {LABELS[1]: True, LABELS[2]: True, LABELS[3]: False},
                      "snapshot": {"contract": "ai-drain-snapshot.v1", "databaseKey": "synthetic-cluster:owned:123:public",
                                   "runningInvocations": 0, "runningItems": 0, "sessionLockHeld": True, "executor": row}}
        self.state_file = self.root / "state.json"
        self.environment = {"PATH": str(self.bin) + ":" + str(Path(sys.executable).parent) + ":/usr/bin:/bin:/usr/sbin",
                            "LANG": "en_US.UTF-8", "AI_INSTALL_PROOF_STATE": str(self.state_file),
                            "LINGGAN_RUNTIME_DIR": str(self.runtime), "LINGGAN_SUPPORT_DIR": str(self.support),
                            "LINGGAN_RUNTIME_SOURCE_DIR": str(self.source), "LINGGAN_LAUNCH_AGENTS_DIR": str(self.agents),
                            "TMPDIR": str(self.root / "tmp"), "ZDOTDIR": str(self.root / "zsh")}

    def tearDown(self):
        shutil.rmtree(self.root)

    def execute(self, mode=None, script="install.sh", extra_args=()):
        self.state_file.write_text(json.dumps(self.state))
        result = subprocess.run(["/bin/zsh", "-f", str(self.source / "scripts/runtime" / script), *([mode] if mode else []), *extra_args],
                                env=self.environment, text=True, capture_output=True, timeout=15)
        self.state = json.loads(self.state_file.read_text())
        trace = self.root / "trace.jsonl"
        self.trace = [json.loads(line) for line in trace.read_text().splitlines()] if trace.exists() else []
        return result

    def assert_no_switch(self):
        self.assertEqual(self.state["head"], "a" * 40)
        self.assertFalse(any(row[:2] == ["git", "reset"] or row[0] == "cargo" or row[:2] == ["launchctl", "bootstrap"] for row in self.trace))

    def test_two_disabled_workers_remain_off_and_ai_enabled_intent_is_preserved(self):
        result = self.execute(); self.assertEqual(result.returncode, 0, result.stderr + result.stdout)
        self.assertEqual(self.state["head"], "b" * 40)
        bootstrapped = [plistlib.loads(Path(row[-1]).read_bytes())["Label"] for row in self.trace if row[:2] == ["launchctl", "bootstrap"]]
        self.assertEqual(bootstrapped, [LABELS[0], LABELS[3]])
        for label in (LABELS[1], LABELS[3]):
            value = plistlib.loads((self.agents / (label + ".plist")).read_bytes())
            self.assertEqual(value["ExitTimeOut"], 3616)
            self.assertEqual(value["WorkingDirectory"], str(self.runtime))
        api = plistlib.loads((self.agents / (LABELS[0] + ".plist")).read_bytes())
        self.assertEqual(api["EnvironmentVariables"]["LINGGAN_LOCAL_PORT"], "3001")
        self.assertEqual(api["EnvironmentVariables"]["PATH"].split(":")[0], str(self.bin))
        self.assertFalse(any("-iTCP:3000" in row for row in self.trace if row[0] == "lsof"))
        permit = json.loads((self.support / "runtime-drain/worker-update-permit").read_text())
        self.assertEqual(permit["from"], "a" * 40); self.assertEqual(permit["target"], "b" * 40)
        self.assertEqual(permit["ai"]["state"], "drained")
        identity = json.loads((self.support / "runtime-identity.json").read_text()); self.assertEqual(identity["revision"], "b" * 40)
        reset = next(i for i, row in enumerate(self.trace) if row[:2] == ["git", "reset"])
        stops = [i for i, row in enumerate(self.trace) if row[:2] == ["launchctl", "bootout"]]
        self.assertTrue(all(i < reset for i in stops))
        self.assertTrue(any("linggan-ai-worker" in row for row in self.trace if row[0] == "cargo"))
        self.assertFalse(any(self.state["password"] in part for row in self.trace for part in row))

    def test_explicit_target_survives_main_advancing_during_drain_and_restart(self):
        self.state["advance_main_on_stop"] = "c" * 40
        result = self.execute(extra_args=("--target-sha", "b" * 40))
        self.assertEqual(result.returncode, 0, result.stderr + result.stdout)
        self.assertEqual(self.state["head"], "b" * 40)
        self.assertEqual(self.state["target"], "c" * 40)
        pin = self.support / "runtime-target-revision"
        self.assertEqual(pin.read_text().strip(), "b" * 40)
        self.assertEqual(json.loads((self.support / "runtime-identity.json").read_text())["revision"], "b" * 40)
        # A normal restart reads the installed pin, not the new origin/main.
        (self.root / "trace.jsonl").unlink()
        result = self.execute(script="sync.sh")
        self.assertEqual(result.returncode, 0, result.stderr + result.stdout)
        self.assertEqual(self.state["head"], "b" * 40)
        self.assertFalse(any(row[0] == "git" and "fetch" in row for row in self.trace))

    def test_invalid_explicit_target_never_stops_services(self):
        for target in ("", "main", "HEAD", "b" * 8, "g" * 40, "-" + "b" * 39):
            with self.subTest(target=target):
                (self.root / "trace.jsonl").unlink(missing_ok=True)
                result = self.execute(extra_args=("--target-sha", target))
                self.assertNotEqual(result.returncode, 0)
                self.assert_no_switch()
                self.assertFalse(any(row[:2] == ["launchctl", "bootout"] for row in self.trace))

    def test_target_must_exist_belong_to_main_and_match_clean_source(self):
        for fault in ("target_missing", "target_unmerged", "source_mismatch"):
            with self.subTest(fault=fault):
                self.state[fault] = True
                (self.root / "trace.jsonl").unlink(missing_ok=True)
                result = self.execute(extra_args=("--target-sha", "b" * 40))
                self.assertNotEqual(result.returncode, 0)
                self.assert_no_switch()
                self.assertFalse(any(row[:2] == ["launchctl", "bootout"] for row in self.trace))
                self.state.pop(fault)

    def test_default_target_is_also_frozen_once(self):
        self.state["advance_main_on_stop"] = "c" * 40
        result = self.execute()
        self.assertEqual(result.returncode, 0, result.stderr + result.stdout)
        self.assertEqual(self.state["head"], "b" * 40)
        self.assertEqual((self.support / "runtime-target-revision").read_text().strip(), "b" * 40)

    def test_invalid_installed_pin_is_rejected_without_switch_or_build(self):
        (self.support / "runtime-target-revision").write_text("main\n")
        result = self.execute(script="sync.sh")
        self.assertNotEqual(result.returncode, 0)
        self.assert_no_switch()

    def test_legacy_restart_without_pin_keeps_its_current_revision(self):
        result = self.execute(script="sync.sh")
        self.assertEqual(result.returncode, 0, result.stderr + result.stdout)
        self.assertEqual(self.state["head"], "a" * 40)
        self.assertFalse(any(row[0] == "git" and "fetch" in row for row in self.trace))

    def test_build_failure_does_not_replace_the_last_installed_pin(self):
        self.support.mkdir(exist_ok=True)
        pin = self.support / "runtime-target-revision"
        pin.write_text("a" * 40 + "\n")
        self.state["build_failure"] = True
        result = self.execute(extra_args=("--target-sha", "b" * 40))
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(pin.read_text().strip(), "a" * 40)
        self.assertFalse(any(row[:2] == ["launchctl", "bootstrap"] for row in self.trace))

    def test_missing_duplicate_targets_and_conflicting_modes_are_rejected(self):
        for args in [("--target-sha",), ("--target-sha", "b" * 40, "--target-sha", "b" * 40),
                     ("--prepare-migration", "--bootstrap-no-drain")]:
            with self.subTest(args=args):
                (self.root / "trace.jsonl").unlink(missing_ok=True)
                result = self.execute(extra_args=args)
                self.assertNotEqual(result.returncode, 0)
                self.assertFalse(any(row[:2] == ["launchctl", "bootout"] for row in self.trace))

    def test_real_git_target_survives_remote_and_source_advancing(self):
        real_git = shutil.which("git", path="/usr/bin:/bin")
        self.assertIsNotNone(real_git)
        git_env = {**os.environ, "GIT_CONFIG_NOSYSTEM": "1", "GIT_CONFIG_GLOBAL": os.devnull}

        def git(directory, *arguments):
            return subprocess.check_output([real_git, "-C", str(directory), *arguments],
                                           env=git_env, stderr=subprocess.PIPE, text=True).strip()

        remote = self.root / "origin.git"
        subprocess.run([real_git, "init", "--bare", str(remote)], env=git_env,
                       capture_output=True, check=True)
        git(self.source, "init", "-b", "main")
        git(self.source, "config", "user.email", "synthetic@example.invalid")
        git(self.source, "config", "user.name", "Owned proof")
        (self.source / ".gitignore").write_text(".env\n")
        marker = self.source / "release-marker"
        marker.write_text("previous")
        git(self.source, "add", ".")
        git(self.source, "commit", "-m", "synthetic previous")
        previous = git(self.source, "rev-parse", "HEAD")
        marker.write_text("selected")
        git(self.source, "commit", "-am", "synthetic selected")
        selected = git(self.source, "rev-parse", "HEAD")
        git(self.source, "remote", "add", "origin", str(remote))
        git(self.source, "push", "-u", "origin", "main")
        marker.write_text("later")
        # An advancing shared checkout also replaces the sync helper. It must never be used.
        (self.source / "scripts/runtime/sync.sh").write_text("#!/bin/zsh\nexit 98\n")
        git(self.source, "commit", "-am", "synthetic later release")
        later = git(self.source, "rev-parse", "HEAD")
        git(self.source, "reset", "--hard", selected)
        shutil.rmtree(self.runtime)
        subprocess.run([real_git, "clone", str(remote), str(self.runtime)], env=git_env,
                       capture_output=True, check=True)
        git(self.runtime, "checkout", "--detach", previous)
        shutil.copy2(self.source / ".env", self.runtime / ".env")
        self.environment.update({"GIT_CONFIG_NOSYSTEM": "1", "GIT_CONFIG_GLOBAL": os.devnull})
        self.state.update({"real_git": real_git, "real_git_advance": later, "head": previous,
                           "target": selected, "source_head": selected})
        self.state["snapshot"]["executor"]["sourceRevision"] = previous
        result = self.execute(extra_args=("--target-sha", selected))
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertEqual(git(self.source, "rev-parse", "HEAD"), later)
        self.assertEqual(git(remote, "rev-parse", "main"), later)
        self.assertEqual(git(self.runtime, "rev-parse", "HEAD"), selected)
        self.assertEqual((self.runtime / "release-marker").read_text(), "selected")
        self.assertEqual((self.support / "runtime-target-revision").read_text().strip(), selected)
        (self.root / "trace.jsonl").unlink()
        result = subprocess.run(["/bin/zsh", "-f", str(self.runtime / "scripts/runtime/sync.sh")],
                                env=self.environment, capture_output=True, text=True, timeout=15)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertEqual(git(self.runtime, "rev-parse", "HEAD"), selected)
        self.assertEqual(json.loads((self.support / "runtime-identity.json").read_text())["revision"], selected)
        trace = [json.loads(line) for line in (self.root / "trace.jsonl").read_text().splitlines()]
        self.assertFalse(any(row[0] == "git" and "fetch" in row for row in trace))

    def test_new_ai_service_is_written_disabled_and_not_bootstrapped(self):
        self.state["loaded"][LABELS[3]] = False; self.state["disabled"].pop(LABELS[3])
        self.state["snapshot"]["executor"] = None; self.state["snapshot"]["sessionLockHeld"] = False
        result = self.execute(); self.assertEqual(result.returncode, 0, result.stderr + result.stdout)
        self.assertTrue(self.state["disabled"][LABELS[3]])
        self.assertFalse(self.state["loaded"][LABELS[3]])
        self.assertTrue((self.agents / (LABELS[3] + ".plist")).is_file())

    def test_migration_preparation_stops_all_four_without_reset_build_bootstrap_or_permit(self):
        result = self.execute("--prepare-migration"); self.assertEqual(result.returncode, 0, result.stderr + result.stdout)
        self.assert_no_switch(); self.assertFalse(any(self.state["loaded"].values()))
        self.assertFalse((self.support / "runtime-drain/worker-update-permit").exists())

    def test_failed_stale_and_wrong_database_receipts_never_switch_code(self):
        for fault in ("ack_source", "ack_generation", "failed_ack", "inflight", "database"):
            with self.subTest(fault=fault):
                self.state["fault"] = fault; self.state["loaded"] = {label: True for label in LABELS}
                self.state["snapshot"]["executor"]["state"] = "running"
                self.state["snapshot"]["runningInvocations"] = 0; self.state["snapshot"]["sessionLockHeld"] = True
                (self.root / "trace.jsonl").unlink(missing_ok=True)
                result = self.execute(); self.assertNotEqual(result.returncode, 0); self.assert_no_switch()

    def test_unreadable_disabled_state_or_unknown_launchctl_state_is_not_no_worker(self):
        for key, value in (("disabled_unreadable", True), ("inspection_unreadable", LABELS[1])):
            with self.subTest(key=key):
                self.state[key] = value; (self.root / "trace.jsonl").unlink(missing_ok=True)
                result = self.execute(); self.assertNotEqual(result.returncode, 0); self.assert_no_switch()
                self.assertFalse(any(row[:2] == ["launchctl", "bootout"] for row in self.trace))
                self.state.pop(key)

    def test_unreadable_process_or_port_probe_cannot_release_a_permit(self):
        for key in ("process_probe_unreadable", "port_probe_unreadable"):
            with self.subTest(key=key):
                self.state[key] = True; self.state["loaded"] = {label: True for label in LABELS}
                self.state["snapshot"]["executor"]["state"] = "running"; self.state["snapshot"]["sessionLockHeld"] = True
                (self.root / "trace.jsonl").unlink(missing_ok=True)
                result = self.execute(); self.assertNotEqual(result.returncode, 0); self.assert_no_switch()
                self.assertFalse((self.support / "runtime-drain/worker-update-permit").exists()); self.state.pop(key)

    def test_sync_rejects_a_missing_permit_before_reset_or_build(self):
        self.environment["LINGGAN_RUNTIME_TARGET_REVISION"] = "b" * 40
        result = self.execute(script="sync.sh"); self.assertNotEqual(result.returncode, 0); self.assert_no_switch()

    def test_dirty_release_source_is_rejected_before_any_service_is_stopped(self):
        self.state["dirty"] = True
        result = self.execute(); self.assertNotEqual(result.returncode, 0); self.assert_no_switch()
        self.assertFalse(any(row[:2] == ["launchctl", "bootout"] for row in self.trace))


if __name__ == "__main__":
    unittest.main()
