#!/usr/bin/env python3
"""Retrieval-only LongMemEval runner for rust-mempalace."""

from __future__ import annotations

import argparse
import json
import os
import re
import subprocess
import sys
import tempfile
import time
from datetime import datetime, timezone
from pathlib import Path

RUNNER_VERSION = "j13-runner-v1"
DEFAULT_COMMAND_TIMEOUT_SECS = 120
REQUIRED_FIELDS = {
    "question_id",
    "question_type",
    "question",
    "haystack_session_ids",
    "haystack_dates",
    "haystack_sessions",
    "answer_session_ids",
}


class BrokenRun(Exception):
    pass


def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--dataset", required=True, type=Path)
    parser.add_argument("--out-dir", required=True, type=Path)
    parser.add_argument(
        "--mode", required=True, choices=("nightly", "weekly", "manual", "fixture")
    )
    parser.add_argument("--sample-size", type=int)
    parser.add_argument("--top-k", type=int, default=5)
    parser.add_argument("--command-timeout-secs", type=int, default=DEFAULT_COMMAND_TIMEOUT_SECS)
    parser.add_argument("--repo-root", type=Path, default=Path(__file__).resolve().parents[1])
    parser.add_argument("--include-abstention", action="store_true")
    return parser.parse_args(argv)


def load_dataset(path: Path) -> list[dict]:
    if not path.exists():
        raise BrokenRun(f"dataset not found: {path}")
    text = path.read_text(encoding="utf-8").strip()
    if not text:
        raise BrokenRun(f"dataset is empty: {path}")
    try:
        raw = json.loads(text)
    except json.JSONDecodeError:
        raw = [json.loads(line) for line in text.splitlines() if line.strip()]
    if isinstance(raw, dict):
        for key in ("data", "examples", "items"):
            if isinstance(raw.get(key), list):
                raw = raw[key]
                break
    if not isinstance(raw, list):
        raise BrokenRun("dataset must be a JSON array or JSONL records")
    for index, case in enumerate(raw):
        if not isinstance(case, dict):
            raise BrokenRun(f"case {index} is not an object")
        missing = sorted(REQUIRED_FIELDS.difference(case))
        if missing:
            qid = case.get("question_id", f"index:{index}")
            raise BrokenRun(f"case {qid} missing required fields: {', '.join(missing)}")
    return raw


def is_abstention(case: dict) -> bool:
    qid = str(case.get("question_id", ""))
    qtype = str(case.get("question_type", "")).lower()
    return qid.endswith("_abs") or "abstention" in qtype or qtype == "abs"


def select_cases(cases: list[dict], sample_size: int | None, include_abstention: bool) -> list[dict]:
    selected = [case for case in cases if include_abstention or not is_abstention(case)]
    selected.sort(key=lambda case: str(case["question_id"]))
    if sample_size is not None:
        if sample_size < 0:
            raise BrokenRun("--sample-size must be >= 0")
        if sample_size > 0:
            selected = selected[:sample_size]
    if not selected:
        raise BrokenRun("no benchmark cases after filtering")
    return selected


def safe_name(value: object) -> str:
    text = str(value)
    text = re.sub(r"[^A-Za-z0-9_.-]+", "_", text).strip("._")
    return text or "session"


def render_session(session_id: str, date: object, session: object) -> str:
    lines = [f"session_id: {session_id}", f"date: {date}", "", "turns:"]
    if isinstance(session, list):
        for index, turn in enumerate(session, 1):
            lines.append(f"\nturn {index}:")
            if isinstance(turn, dict):
                for key in sorted(turn):
                    lines.append(f"{key}: {turn[key]}")
            else:
                lines.append(str(turn))
    elif isinstance(session, dict):
        for key in sorted(session):
            lines.append(f"{key}: {session[key]}")
    else:
        lines.append(str(session))
    lines.append("")
    return "\n".join(lines)


def write_sessions(case: dict, root: Path) -> dict[str, str]:
    ids = case["haystack_session_ids"]
    dates = case["haystack_dates"]
    sessions = case["haystack_sessions"]
    if not (isinstance(ids, list) and isinstance(dates, list) and isinstance(sessions, list)):
        raise BrokenRun(f"case {case['question_id']} haystack fields must be arrays")
    if not (len(ids) == len(dates) == len(sessions)):
        raise BrokenRun(f"case {case['question_id']} haystack arrays have different lengths")

    root.mkdir(parents=True, exist_ok=True)
    path_to_session: dict[str, str] = {}
    seen: dict[str, int] = {}
    for session_id, date, session in zip(ids, dates, sessions):
        sid = str(session_id)
        base = safe_name(sid)
        seen[base] = seen.get(base, 0) + 1
        suffix = "" if seen[base] == 1 else f"_{seen[base]}"
        path = root / f"{base}{suffix}.txt"
        path.write_text(render_session(sid, date, session), encoding="utf-8")
        path_to_session[str(path)] = sid
        path_to_session[path.name] = sid
    return path_to_session


def cli_prefix(repo_root: Path) -> list[str]:
    override = os.environ.get("LONGMEMEVAL_MEMPALACE_BIN")
    if override:
        return [override]
    return ["cargo", "run", "--quiet", "-p", "rust-mempalace", "--manifest-path", str(repo_root / "Cargo.toml"), "--"]


def run_cli(
    args: list[str], repo_root: Path, timeout_secs: int
) -> subprocess.CompletedProcess:
    cmd = cli_prefix(repo_root) + args
    try:
        return subprocess.run(
            cmd,
            cwd=str(repo_root),
            check=True,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            env={**os.environ, "NO_COLOR": "1"},
            timeout=timeout_secs,
        )
    except FileNotFoundError as exc:
        raise BrokenRun(f"CLI executable not found: {cmd[0]}") from exc
    except subprocess.TimeoutExpired as exc:
        raise TimeoutError(f"CLI timed out after {timeout_secs}s: {' '.join(cmd)}") from exc
    except subprocess.CalledProcessError as exc:
        stderr = (exc.stderr or "").strip()
        stdout = (exc.stdout or "").strip()
        raise BrokenRun(
            f"CLI failed: {' '.join(cmd)}\nstdout={stdout}\nstderr={stderr}"
        ) from exc


def mine_sessions(
    palace: Path, sessions_dir: Path, repo_root: Path, bank: str, timeout_secs: int
) -> None:
    run_cli(
        [
            "--palace",
            str(palace),
            "--quiet",
            "--output",
            "json",
            "mine",
            str(sessions_dir),
            "--bank",
            bank,
        ],
        repo_root,
        timeout_secs,
    )


def search(
    palace: Path, query: str, repo_root: Path, top_k: int, bank: str, timeout_secs: int
) -> tuple[list[dict], float]:
    started = time.perf_counter()
    out = run_cli(
        [
            "--palace",
            str(palace),
            "--quiet",
            "--output",
            "json",
            "search",
            query,
            "--limit",
            str(top_k),
            "--bank",
            bank,
        ],
        repo_root,
        timeout_secs,
    )
    elapsed_ms = (time.perf_counter() - started) * 1000.0
    try:
        data = json.loads(out.stdout.strip() or "{}")
    except json.JSONDecodeError as exc:
        raise BrokenRun(f"CLI search returned non-JSON stdout: {out.stdout}") from exc
    results = data.get("results")
    if not isinstance(results, list):
        raise BrokenRun("CLI search JSON missing results array")
    return results, elapsed_ms


def session_from_source(source_path: object, path_to_session: dict[str, str]) -> str | None:
    if not isinstance(source_path, str):
        return None
    if source_path in path_to_session:
        return path_to_session[source_path]
    clean = source_path.split("#", 1)[0]
    return path_to_session.get(clean) or path_to_session.get(Path(clean).name)


def score_case(case: dict, returned_ids: list[str]) -> tuple[int, int, float]:
    expected = {str(v) for v in case["answer_session_ids"]}
    rank = None
    for index, session_id in enumerate(returned_ids, 1):
        if session_id in expected:
            rank = index
            break
    if rank is None:
        return 0, 0, 0.0
    return int(rank == 1), int(rank <= 5), 1.0 / rank


def short_snippet(value: object, limit: int = 220) -> str:
    text = re.sub(r"\s+", " ", str(value or "")).strip()
    return text[:limit]


def timeout_case_row(case: dict, reason: str) -> dict:
    qid = str(case["question_id"])
    return {
        "case_id": qid,
        "question_type": case["question_type"],
        "query": case["question"],
        "expected_session_ids": [str(v) for v in case["answer_session_ids"]],
        "returned_session_ids": [],
        "returned_top": [],
        "hit_at_1": False,
        "hit_at_5": False,
        "reciprocal_rank": 0.0,
        "query_ms": 0.0,
        "timed_out": True,
        "failure_reason": reason,
    }


def run_case(
    case: dict, repo_root: Path, top_k: int, work_root: Path, timeout_secs: int
) -> tuple[dict, float, bool]:
    qid = str(case["question_id"])
    case_root = work_root / safe_name(qid)
    sessions_dir = case_root / "sessions"
    palace = case_root / "palace"
    path_to_session = write_sessions(case, sessions_dir)
    bank = f"longmemeval-{safe_name(qid)}"
    try:
        mine_sessions(palace, sessions_dir, repo_root, bank, timeout_secs)
        results, query_ms = search(
            palace, str(case["question"]), repo_root, top_k, bank, timeout_secs
        )
    except TimeoutError as exc:
        return timeout_case_row(case, str(exc)), 0.0, True
    returned_ids = [
        sid
        for sid in (session_from_source(row.get("source_path"), path_to_session) for row in results)
        if sid is not None
    ]
    r1, r5, mrr = score_case(case, returned_ids)
    row = {
        "case_id": qid,
        "question_type": case["question_type"],
        "query": case["question"],
        "expected_session_ids": [str(v) for v in case["answer_session_ids"]],
        "returned_session_ids": returned_ids,
        "returned_top": [
            {
                "session_id": session_from_source(result.get("source_path"), path_to_session),
                "source_path": result.get("source_path"),
                "score": result.get("score"),
                "snippet": short_snippet(result.get("snippet")),
            }
            for result in results[:top_k]
        ],
        "hit_at_1": bool(r1),
        "hit_at_5": bool(r5),
        "reciprocal_rank": mrr,
        "query_ms": query_ms,
        "timed_out": False,
    }
    return row, query_ms, False


def aggregate(
    case_rows: list[dict],
    query_latencies: list[float],
    total_runtime_sec: float,
    timeout_count: int,
    args: argparse.Namespace,
) -> dict:
    total = len(case_rows)
    failures = [row for row in case_rows if not row["hit_at_5"]]
    avg_query_ms = sum(query_latencies) / len(query_latencies) if query_latencies else 0.0
    return {
        "runner_version": RUNNER_VERSION,
        "generated_at": datetime.now(timezone.utc).isoformat(),
        "mode": args.mode,
        "sample_count": total,
        "top_k": args.top_k,
        "metrics": {
            "r_at_1": sum(1 for row in case_rows if row["hit_at_1"]) / total,
            "r_at_5": sum(1 for row in case_rows if row["hit_at_5"]) / total,
            "mrr": sum(row["reciprocal_rank"] for row in case_rows) / total,
        },
        "runtime": {
            "total_runtime_sec": total_runtime_sec,
            "avg_query_ms": avg_query_ms,
            "throughput_per_sec": total / total_runtime_sec if total_runtime_sec > 0 else 0.0,
            "timeout_count": timeout_count,
        },
        "failed_count": len(failures),
        "failed_cases": failures,
        "cases": case_rows,
    }


def write_artifacts(report: dict, args: argparse.Namespace, selected_count: int) -> None:
    args.out_dir.mkdir(parents=True, exist_ok=True)
    report_path = args.out_dir / "longmemeval-report.json"
    md_path = args.out_dir / "longmemeval-report.md"
    config_path = args.out_dir / "run-config.json"
    failed_path = args.out_dir / "failed-cases.jsonl"

    report_path.write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    config = {
        "runner_version": RUNNER_VERSION,
        "dataset": str(args.dataset),
        "out_dir": str(args.out_dir),
        "mode": args.mode,
        "sample_size": args.sample_size,
        "selected_count": selected_count,
        "top_k": args.top_k,
        "command_timeout_secs": args.command_timeout_secs,
        "repo_root": str(args.repo_root),
        "include_abstention": args.include_abstention,
    }
    config_path.write_text(json.dumps(config, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    with failed_path.open("w", encoding="utf-8") as fh:
        for row in report["failed_cases"]:
            failed = {
                "case_id": row["case_id"],
                "query": row["query"],
                "expected_session_ids": row["expected_session_ids"],
                "returned_session_ids": row["returned_session_ids"],
                "returned_top": row["returned_top"],
                "timed_out": row.get("timed_out", False),
            }
            if row.get("failure_reason"):
                failed["failure_reason"] = row["failure_reason"]
            fh.write(json.dumps(failed, ensure_ascii=False) + "\n")
    md_path.write_text(markdown_report(report), encoding="utf-8")


def markdown_report(report: dict) -> str:
    metrics = report["metrics"]
    runtime = report["runtime"]
    lines = [
        "# LongMemEval Report",
        "",
        f"- Mode: {report['mode']}",
        f"- Sample count: {report['sample_count']}",
        f"- R@1: {metrics['r_at_1']:.4f}",
        f"- R@5: {metrics['r_at_5']:.4f}",
        f"- MRR: {metrics['mrr']:.4f}",
        f"- Total runtime sec: {runtime['total_runtime_sec']:.3f}",
        f"- Avg query ms: {runtime['avg_query_ms']:.3f}",
        f"- Throughput/sec: {runtime['throughput_per_sec']:.3f}",
        f"- Timeout count: {runtime['timeout_count']}",
        f"- Failed cases: {report['failed_count']}",
        "",
        "## Failed Cases",
        "",
    ]
    if not report["failed_cases"]:
        lines.append("None.")
    else:
        for row in report["failed_cases"]:
            lines.extend(
                [
                    f"### {row['case_id']}",
                    "",
                    f"- Expected: {', '.join(row['expected_session_ids'])}",
                    f"- Returned: {', '.join(row['returned_session_ids'])}",
                    f"- Query: {row['query']}",
                    "",
                ]
            )
    lines.append("")
    return "\n".join(lines)


def main(argv: list[str]) -> int:
    args = parse_args(argv)
    if args.top_k < 1:
        raise BrokenRun("--top-k must be >= 1")
    if args.command_timeout_secs < 1:
        raise BrokenRun("--command-timeout-secs must be >= 1")
    cases = load_dataset(args.dataset)
    selected = select_cases(cases, args.sample_size, args.include_abstention)

    started = time.perf_counter()
    case_rows: list[dict] = []
    query_latencies: list[float] = []
    timeout_count = 0
    args.out_dir.parent.mkdir(parents=True, exist_ok=True)
    try:
        with tempfile.TemporaryDirectory(prefix="longmemeval-", dir=str(args.out_dir.parent)) as tmp:
            work_root = Path(tmp)
            for case in selected:
                row, query_ms, timed_out = run_case(
                    case, args.repo_root, args.top_k, work_root, args.command_timeout_secs
                )
                case_rows.append(row)
                if not timed_out:
                    query_latencies.append(query_ms)
                else:
                    timeout_count += 1
        total_runtime_sec = time.perf_counter() - started
        report = aggregate(case_rows, query_latencies, total_runtime_sec, timeout_count, args)
        write_artifacts(report, args, len(selected))
    except OSError as exc:
        raise BrokenRun(f"artifact or temp file operation failed: {exc}") from exc
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main(sys.argv[1:]))
    except BrokenRun as exc:
        print(f"error: {exc}", file=sys.stderr)
        raise SystemExit(1)
