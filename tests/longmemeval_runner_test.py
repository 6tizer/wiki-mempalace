#!/usr/bin/env python3
import json
import os
import shutil
import subprocess
import sys
import tempfile
import textwrap
import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]
RUNNER = REPO_ROOT / "scripts" / "longmemeval_run.py"
FIXTURE = REPO_ROOT / "tests" / "fixtures" / "longmemeval_tiny.json"


class LongMemEvalRunnerTest(unittest.TestCase):
    def test_fixture_metrics_and_artifacts(self):
        with tempfile.TemporaryDirectory() as tmp:
            tmp_path = Path(tmp)
            fake_cli = tmp_path / "fake_mempalace.py"
            fake_cli.write_text(
                textwrap.dedent(
                    """\
                    #!/usr/bin/env python3
                    import json
                    import sys
                    from pathlib import Path

                    args = sys.argv[1:]
                    cmd = args[args.index("mine") if "mine" in args else args.index("search")]
                    if cmd == "mine":
                        print(json.dumps({"filed_drawers": 2}))
                        raise SystemExit(0)

                    query = args[args.index("search") + 1]
                    palace = Path(args[args.index("--palace") + 1])
                    case_dir = palace.parent
                    sessions = case_dir / "sessions"

                    def row(name, score):
                        return {
                            "id": int(score * 100),
                            "wing": "w",
                            "hall": "h",
                            "room": "r",
                            "bank_id": "b",
                            "source_path": str(sessions / name),
                            "snippet": name,
                            "score": score,
                            "explain": None,
                        }

                    if "Postgres" in query:
                        results = [row("s1.txt", 1.0), row("s2.txt", 0.5)]
                    elif "amber token" in query:
                        results = [row("s4.txt", 1.0), row("s3.txt", 0.5)]
                    else:
                        results = []
                    print(json.dumps({"results": results}))
                    """
                ),
                encoding="utf-8",
            )
            fake_cli.chmod(0o755)

            out_dir = tmp_path / "out"
            env = {**os.environ, "LONGMEMEVAL_MEMPALACE_BIN": str(fake_cli)}
            subprocess.run(
                [
                    sys.executable,
                    str(RUNNER),
                    "--dataset",
                    str(FIXTURE),
                    "--out-dir",
                    str(out_dir),
                    "--mode",
                    "fixture",
                    "--sample-size",
                    "3",
                    "--top-k",
                    "5",
                    "--repo-root",
                    str(REPO_ROOT),
                ],
                cwd=str(REPO_ROOT),
                env=env,
                check=True,
            )

            report_path = out_dir / "longmemeval-report.json"
            md_path = out_dir / "longmemeval-report.md"
            config_path = out_dir / "run-config.json"
            failed_path = out_dir / "failed-cases.jsonl"

            self.assertTrue(report_path.exists())
            self.assertTrue(md_path.exists())
            self.assertTrue(config_path.exists())
            self.assertTrue(failed_path.exists())

            report = json.loads(report_path.read_text(encoding="utf-8"))
            self.assertEqual(report["sample_count"], 3)
            self.assertAlmostEqual(report["metrics"]["r_at_1"], 1 / 3)
            self.assertAlmostEqual(report["metrics"]["r_at_5"], 2 / 3)
            self.assertAlmostEqual(report["metrics"]["mrr"], (1 + 0.5 + 0) / 3)
            self.assertEqual(report["failed_count"], 1)
            self.assertEqual(report["failed_cases"][0]["case_id"], "q3")
            self.assertEqual(report["runtime"]["timeout_count"], 0)
            self.assertIn("R@1: 0.3333", md_path.read_text(encoding="utf-8"))

            failed_lines = failed_path.read_text(encoding="utf-8").splitlines()
            self.assertEqual(len(failed_lines), 1)
            self.assertEqual(json.loads(failed_lines[0])["case_id"], "q3")

    def test_abstention_skipped_by_default(self):
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp) / "out"
            env = {**os.environ, "LONGMEMEVAL_MEMPALACE_BIN": shutil.which("true") or "/usr/bin/true"}
            proc = subprocess.run(
                [
                    sys.executable,
                    "-c",
                    (
                        "import importlib.util, pathlib; "
                        f"p=pathlib.Path({str(RUNNER)!r}); "
                        "s=importlib.util.spec_from_file_location('runner', p); "
                        "m=importlib.util.module_from_spec(s); s.loader.exec_module(m); "
                        f"cases=m.load_dataset(pathlib.Path({str(FIXTURE)!r})); "
                        "print([c['question_id'] for c in m.select_cases(cases, None, False)])"
                    ),
                ],
                cwd=str(REPO_ROOT),
                env=env,
                text=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                check=True,
            )
            self.assertNotIn("q4_abs", proc.stdout)

    def test_sample_size_zero_means_full_filtered_dataset(self):
        proc = subprocess.run(
            [
                sys.executable,
                "-c",
                (
                    "import importlib.util, pathlib; "
                    f"p=pathlib.Path({str(RUNNER)!r}); "
                    "s=importlib.util.spec_from_file_location('runner', p); "
                    "m=importlib.util.module_from_spec(s); s.loader.exec_module(m); "
                    f"cases=m.load_dataset(pathlib.Path({str(FIXTURE)!r})); "
                    "print(len(m.select_cases(cases, 0, False)))"
                ),
            ],
            cwd=str(REPO_ROOT),
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=True,
        )
        self.assertEqual(proc.stdout.strip(), "3")

    def test_real_cli_smoke_has_at_least_one_hit(self):
        binary = REPO_ROOT / "target" / "debug" / "rust-mempalace"
        if not binary.exists():
            subprocess.run(
                ["cargo", "build", "-p", "rust-mempalace"],
                cwd=str(REPO_ROOT),
                check=True,
            )
        with tempfile.TemporaryDirectory() as tmp:
            tmp_path = Path(tmp)
            out_dir = tmp_path / "out"
            env = {**os.environ, "LONGMEMEVAL_MEMPALACE_BIN": str(binary)}
            subprocess.run(
                [
                    sys.executable,
                    str(RUNNER),
                    "--dataset",
                    str(FIXTURE),
                    "--out-dir",
                    str(out_dir),
                    "--mode",
                    "fixture",
                    "--sample-size",
                    "3",
                    "--top-k",
                    "5",
                    "--repo-root",
                    str(REPO_ROOT),
                ],
                cwd=str(REPO_ROOT),
                env=env,
                check=True,
            )
            report = json.loads((out_dir / "longmemeval-report.json").read_text(encoding="utf-8"))
            self.assertGreaterEqual(report["metrics"]["r_at_5"], 1 / 3)


if __name__ == "__main__":
    unittest.main()
