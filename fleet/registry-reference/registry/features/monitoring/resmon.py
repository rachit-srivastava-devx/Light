#!/usr/bin/env python3
"""Persistent psutil sampler for fleet's process tree.

The shell report intentionally consumes the legacy six-column aggregate TSV. This
sampler also writes a detail TSV so each process's CPU, RSS, recursive children,
and open-file counts remain inspectable without changing that report contract.
"""

from __future__ import annotations

import argparse
import re
import sys
import time
from collections import OrderedDict
from pathlib import Path

import psutil


PATTERNS = OrderedDict(
    (
        ("console", re.compile(r"console/server/index\.mjs", re.I)),
        ("claude", re.compile(r"(^|/)claude(?:\s|$)", re.I)),
        ("codex", re.compile(r"(^|/)codex(?:-cli)?(?:\s|$)|codex exec", re.I)),
        ("cargo", re.compile(r"(^|/)cargo(?:-mutants|-deny)?(?:\s|$)", re.I)),
        ("node", re.compile(r"(^|/)node(?:\s|$)", re.I)),
        ("bats", re.compile(r"bats-(?:core|exec)|/bats(?:\s|$)", re.I)),
        ("python", re.compile(r"(^|/)python3?(?:\s|$)", re.I)),
    )
)


def command_line(proc: psutil.Process, info: dict) -> str:
    cmdline = info.get("cmdline") or []
    if isinstance(cmdline, list) and cmdline:
        return " ".join(str(part) for part in cmdline).replace("\t", " ").replace("\n", " ")
    return str(info.get("name") or proc.name())


def process_rows(cache: dict[int, psutil.Process]) -> dict[int, dict]:
    rows: dict[int, dict] = {}
    try:
        processes = psutil.process_iter(["pid", "ppid", "name", "cmdline"])
        process_list = list(processes)
    except (PermissionError, OSError) as exc:
        raise RuntimeError(f"psutil cannot enumerate the process table: {exc}") from exc
    self_pid = psutil.Process().pid
    for info in process_list:
        pid = int(info.info["pid"])
        if pid == self_pid:
            continue
        try:
            proc = cache.setdefault(pid, psutil.Process(pid))
            command = command_line(proc, info.info)
        except (psutil.AccessDenied, psutil.NoSuchProcess, psutil.ZombieProcess):
            continue
        if "resmon.py" in command:
            continue
        try:
            cpu = float(proc.cpu_percent(interval=None))
            rss = int(proc.memory_info().rss / (1024 * 1024))
            children = len(proc.children(recursive=True))
            open_files = len(proc.open_files())
        except (psutil.AccessDenied, psutil.NoSuchProcess, psutil.ZombieProcess):
            continue
        rows[pid] = {
            "pid": pid,
            "ppid": int(info.info.get("ppid") or 0),
            "command": command,
            "cpu": cpu,
            "rss": rss,
            "children": children,
            "open_files": open_files,
        }
    return rows


def descendants(rows: dict[int, dict], roots: set[int]) -> set[int]:
    result = set(roots)
    changed = True
    while changed:
        changed = False
        for pid, row in rows.items():
            if row["ppid"] in result and pid not in result:
                result.add(pid)
                changed = True
    return result


def sample(out: Path, detail: Path, cache: dict[int, psutil.Process]) -> None:
    rows = process_rows(cache)
    classes: dict[str, set[int]] = {name: set() for name in PATTERNS}
    for pid, row in rows.items():
        for name, pattern in PATTERNS.items():
            if pattern.search(row["command"]):
                classes[name].add(pid)
    for name in classes:
        classes[name] = descendants(rows, classes[name])
    timestamp = time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())
    loadavg = psutil.getloadavg()[0]
    out.parent.mkdir(parents=True, exist_ok=True)
    detail.parent.mkdir(parents=True, exist_ok=True)
    if not out.exists():
        out.write_text("# ts\tclass\tprocs\tcpu_pct\trss_mib\tloadavg\n", encoding="utf-8")
    if not detail.exists():
        detail.write_text("# ts\tpid\tppid\tclass\tcpu_pct\trss_mib\tchildren\topen_files\tcommand\n", encoding="utf-8")
    with out.open("a", encoding="utf-8") as aggregate, detail.open("a", encoding="utf-8") as per_process:
        for name, pids in classes.items():
            selected = [rows[pid] for pid in pids if pid in rows]
            aggregate.write(
                f"{timestamp}\t{name}\t{len(selected)}\t"
                f"{sum(row['cpu'] for row in selected):.1f}\t"
                f"{sum(row['rss'] for row in selected)}\t{loadavg:.2f}\n"
            )
            for row in selected:
                per_process.write(
                    f"{timestamp}\t{row['pid']}\t{row['ppid']}\t{name}\t{row['cpu']:.1f}\t"
                    f"{row['rss']}\t{row['children']}\t{row['open_files']}\t{row['command']}\n"
                )


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--out", required=True, type=Path)
    parser.add_argument("--detail", required=True, type=Path)
    parser.add_argument("--interval", required=True, type=float)
    args = parser.parse_args()
    cache: dict[int, psutil.Process] = {}
    try:
        while True:
            sample(args.out, args.detail, cache)
            time.sleep(max(args.interval, 0.1))
    except RuntimeError as exc:
        print(f"resmon: {exc}", file=sys.stderr)
        return 3


if __name__ == "__main__":
    raise SystemExit(main())
