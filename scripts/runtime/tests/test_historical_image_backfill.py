"""Connection boundaries for the explicitly local historical image backfill."""

import importlib.util
import io
import os
from pathlib import Path
import sys
import unittest
from unittest.mock import patch


SCRIPT = Path(__file__).resolve().parents[2] / "backfill-historical-image-inventory.py"
spec = importlib.util.spec_from_file_location("historical_image_backfill", SCRIPT)
backfill = importlib.util.module_from_spec(spec)
spec.loader.exec_module(backfill)


class LocalTarget(unittest.TestCase):
    def test_explicit_loopback_and_plain_database(self):
        for host in ("127.0.0.1", "::1"):
            env = {"PGHOST": host, "PGPORT": "55432", "PGDATABASE": "linggan_proof"}
            self.assertEqual(
                backfill.local_psql_target(env),
                ["-h", host, "-p", "55432", "-d", "linggan_proof"],
            )

    def test_remote_implicit_or_connection_string_targets_are_rejected(self):
        good = {"PGHOST": "127.0.0.1", "PGPORT": "55432", "PGDATABASE": "linggan_proof"}
        changes = [
            {"PGHOST": ""}, {"PGHOST": "localhost"}, {"PGHOST": "192.0.2.1"},
            {"PGHOST": "127.0.0.1,192.0.2.1"}, {"PGPORT": ""}, {"PGPORT": "65536"},
            {"PGDATABASE": "postgresql://remote/db"}, {"PGDATABASE": "host=remote dbname=db"},
            {"PGSERVICE": "remote"}, {"PGSERVICEFILE": "/tmp/services"},
            {"PGHOSTADDR": "192.0.2.1"}, {"PGOPTIONS": "-c search_path=other"},
        ]
        for change in changes:
            with self.subTest(change=change):
                with self.assertRaises(ValueError):
                    backfill.local_psql_target({**good, **change})

    def test_apply_rejects_a_remote_target_before_opening_a_plan_or_psql(self):
        env = {"PGHOST": "192.0.2.1", "PGPORT": "5432", "PGDATABASE": "linggan_proof"}
        with patch.dict(os.environ, env, clear=True), \
             patch.object(sys, "argv", ["backfill", "/not/opened.json", "--all-vetted", "--apply"]), \
             patch("subprocess.run") as run, patch("sys.stderr", new_callable=io.StringIO):
            with self.assertRaises(SystemExit):
                backfill.main()
            run.assert_not_called()


if __name__ == "__main__":
    unittest.main()
